"""Modelos, métricas y mínimo de datos de ML v1 (docs/spec/ml-v1.md §4, §6 y §8.4-5), con datos
sintéticos: todavía no hay velas de 15m para entrenar con datos reales."""

import math
import random
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from cfds_intel.models.baselines import B0, B1, BaselineError, load_probability_config
from cfds_intel.models.config import MlConfigError, load_config, load_default
from cfds_intel.models.dataset import Example
from cfds_intel.models.logistic import M1, fit_with_inner_tuning
from cfds_intel.models.metrics import auc, brier, calibration, log_loss, wilson
from cfds_intel.models.splits import Fold, Period, Window
from cfds_intel.models.train import evaluate, minimum_reason

ROOT = Path(__file__).resolve().parents[2]
CFG = load_default()
TRAIN = CFG.training
PROB = load_probability_config((ROOT / "config" / "probability.toml").read_text())
T0 = datetime(2024, 1, 1, 15, 0, tzinfo=UTC)


def synthetic(n: int, seed: int, start: datetime = T0) -> list[Example]:
    """y depende de la primera variable (señal) y no de la segunda (ruido)."""
    rnd = random.Random(seed)  # noqa: S311 (datos de prueba, no criptografía)
    out = []
    for i in range(n):
        signal, noise = rnd.gauss(0, 1), rnd.gauss(0, 1)
        y = 1 if rnd.random() < 1 / (1 + math.exp(-2 * signal)) else 0
        at = start + timedelta(hours=i)
        trig = ("sweep",) if i % 2 else ("candle_pattern",)
        out.append(
            Example(
                f"p{seed}-{i}",
                at,
                at + timedelta(minutes=90),
                (signal, noise, 1.0),
                y,
                trig,
                5 + i % 5,
            )
        )
    return out


def test_the_training_config_is_the_spec() -> None:
    assert (TRAIN.min_examples, TRAIN.min_per_class) == (300, 50)
    assert TRAIN.inner_tune_fraction == 0.2
    base = (ROOT / "config" / "ml.toml").read_text()
    for old, new in [
        ("min_examples = 300", "min_examples = 0"),
        ('inner_tune_fraction = "0.2"', 'inner_tune_fraction = "1.5"'),
        ('inner_tune_fraction = "0.2"', "inner_tune_fraction = 0.2"),
        ('l2_grid = ["0.0001"', 'l2_grid = ["-1"'),
    ]:
        with pytest.raises(MlConfigError):
            load_config(base.replace(old, new))


def test_metrics_match_hand_calculations() -> None:
    assert log_loss([1, 0], [0.8, 0.2]) == pytest.approx(-math.log(0.8))
    assert brier([1, 0], [0.8, 0.2]) == pytest.approx(0.04)
    assert auc([1, 1, 0, 0], [0.9, 0.4, 0.4, 0.1]) == pytest.approx(0.875)
    assert auc([1, 1], [0.5, 0.6]) is None
    assert math.isfinite(log_loss([1], [0.0])), "una predicción 0 no da pérdida infinita"
    low, high = wilson(5, 10)
    assert (round(low, 4), round(high, 4)) == (0.2366, 0.7634)
    bins = calibration([0, 1] * 50, [i / 100 for i in range(100)])
    assert len(bins) == 10 and all(b.n == 10 and b.low <= b.observed <= b.high for b in bins)
    assert (bins[0].p_min, bins[0].p_max, bins[-1].p_max) == (0.0, 0.09, 0.99)
    assert all(b.p_min <= b.mean_predicted <= b.p_max for b in bins)
    with pytest.raises(ValueError):
        log_loss([2], [0.5])


def test_b1_is_the_section_13_table_with_its_minimum() -> None:
    def e(i: int, trig: tuple[str, ...], score: int, y: int) -> Example:
        return Example(f"e{i}", T0, T0, (0.0,), y, trig, score)

    # 30 casos sweep con puntaje 6 (rango 6), 20 aciertos; 10 double_break: no llegan a 30.
    train = [e(i, ("sweep", "candle_pattern"), 6, int(i < 20)) for i in range(30)]
    train += [e(100 + i, ("double_break",), 7, 1) for i in range(10)]
    b0, b1 = B0.fit(train), B1.fit(train, PROB)
    assert b0.p == 30 / 40
    # sweep tiene preferencia sobre candle_pattern (trigger_priority).
    p = b1.predict(
        [
            e(1, ("candle_pattern", "sweep"), 6, 0),
            e(2, ("double_break",), 8, 0),
            e(3, ("sweep",), 4, 0),
        ]
    )
    assert p == [20 / 30, b0.p, b0.p], "grupo con 30; sin mínimo → B0; bajo el primer rango → B0"
    with pytest.raises(BaselineError):
        B0.fit([])


def test_standardization_uses_only_the_training_data() -> None:
    # §8.4: la media y el desvío salen solo del entrenamiento.
    train = synthetic(400, 1)
    m = M1.fit(train, 0.01, TRAIN.seed, TRAIN.max_iterations)
    assert m.mean[0].item() == pytest.approx(sum(e.x[0] for e in train) / len(train))
    shifted = [
        Example(
            e.proposal_id,
            e.signal_at,
            e.known_at,
            (e.x[0] + 100, *e.x[1:]),
            e.y,
            e.triggers,
            e.score,
        )
        for e in synthetic(50, 2)
    ]
    again = M1.fit(train, 0.01, TRAIN.seed, TRAIN.max_iterations)
    assert again.mean.tolist() == m.mean.tolist(), "predecir otra cosa no cambia el modelo"
    assert all(0 <= q <= 1 for q in m.predict(shifted))
    assert m.std[2].item() == 1.0, "una columna constante no divide por cero"


def test_m1_is_reproducible_and_learns_the_signal() -> None:
    # §8.5: misma semilla y mismos datos → mismas probabilidades.
    train, val = synthetic(600, 3), synthetic(300, 4)
    a = fit_with_inner_tuning(
        train, TRAIN.l2_grid, TRAIN.inner_tune_fraction, TRAIN.seed, TRAIN.max_iterations
    )
    b = fit_with_inner_tuning(
        train, TRAIN.l2_grid, TRAIN.inner_tune_fraction, TRAIN.seed, TRAIN.max_iterations
    )
    assert a.predict(val) == b.predict(val)
    assert a.l2 in TRAIN.l2_grid
    assert a.weights[0] > abs(a.weights[1]), "pesa más la señal que el ruido"
    y = [e.y for e in val]
    assert log_loss(y, a.predict(val)) < log_loss(y, B0.fit(train).predict(val))


def test_a_window_below_the_minimum_does_not_train() -> None:
    assert minimum_reason(synthetic(299, 5), TRAIN) is not None
    few_positives = [Example(f"n{i}", T0, T0, (0.0,), int(i < 49), (), 5) for i in range(400)]
    reason = minimum_reason(few_positives, TRAIN)
    assert reason is not None and "49 con TP1" in reason
    assert minimum_reason(synthetic(400, 6), TRAIN) is None


def window(start: datetime, days: int) -> Window:
    mid = start + timedelta(days=days)
    return Window(Period(start, mid), Period(mid, mid + timedelta(days=30)))


def test_evaluate_pools_the_windows_that_trained_and_reports_the_rest() -> None:
    w1, w2 = window(T0, 60), window(T0 + timedelta(days=90), 60)
    folds = [
        Fold(w1, tuple(synthetic(500, 7)), tuple(synthetic(120, 8)), 0, 0),
        Fold(w2, tuple(synthetic(100, 9)), tuple(synthetic(50, 10)), 0, 0),
    ]
    ev = evaluate(folds, TRAIN, PROB)
    assert not ev.suspended
    assert ev.windows[0].skipped is None and ev.windows[1].skipped is not None
    assert set(ev.pooled) == {"B0", "B1", "M1"}
    assert ev.pooled["M1"].log_loss < ev.pooled["B0"].log_loss
    # Ninguna ventana llega: ML v1 en suspenso (§6), sin métricas inventadas.
    none = evaluate(folds[1:], TRAIN, PROB)
    assert none.suspended and none.pooled == {}
