"""Regla de "mejora" e intervalos de ML v1 (docs/spec/ml-v1.md §6), con datos sintéticos."""

import math
import random
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest

from cfds_intel.models.baselines import load_probability_config
from cfds_intel.models.compare import compare, percentile, week_of, weekly_blocks
from cfds_intel.models.config import MlConfigError, load_config, load_default
from cfds_intel.models.dataset import Example
from cfds_intel.models.intervals import predict_with_intervals
from cfds_intel.models.logistic import M1

ROOT = Path(__file__).resolve().parents[2]
CFG = load_default()
PROB = load_probability_config((ROOT / "config" / "probability.toml").read_text())
T0 = datetime(2024, 1, 1, 15, 0, tzinfo=UTC)  # lunes


def examples(
    n: int, seed: int, trig: tuple[str, ...] = ("sweep",), score: int = 6
) -> list[Example]:
    rnd = random.Random(seed)  # noqa: S311
    out = []
    for i in range(n):
        s = rnd.gauss(0, 1)
        y = 1 if rnd.random() < 1 / (1 + math.exp(-2 * s)) else 0
        at = T0 + timedelta(hours=7 * i)
        out.append(Example(f"x{seed}-{i}", at, at + timedelta(hours=2), (s, 1.0), y, trig, score))
    return out


def test_the_evaluation_config_is_the_spec() -> None:
    assert (CFG.evaluation.bootstrap_resamples, CFG.evaluation.confidence) == (1000, 0.95)
    assert (CFG.interval.models, CFG.interval.low_percentile, CFG.interval.high_percentile) == (
        200,
        5,
        95,
    )
    base = (ROOT / "config" / "ml.toml").read_text()
    with pytest.raises(MlConfigError):
        load_config(base.replace("high_percentile = 95", "high_percentile = 4"))
    with pytest.raises(MlConfigError):
        load_config(base.replace("[evaluation]", "[otra]"))


def test_weeks_are_iso_weeks_in_utc() -> None:
    assert week_of(T0) == (2024, 1)
    assert week_of(T0 + timedelta(days=6)) == (2024, 1)
    assert week_of(T0 + timedelta(days=7)) == (2024, 2)
    # Una propuesta cada 7 h desde el lunes a las 15:00: 22, 24 y las 2 que quedan.
    ex = examples(48, 1)
    blocks = weekly_blocks(ex)
    assert [len(b) for b in blocks] == [22, 24, 2]
    assert all(len({week_of(ex[i].signal_at) for i in b}) == 1 for b in blocks)
    assert percentile([1.0, 2.0, 3.0, 4.0], 50) == 2.0
    assert percentile([1.0, 2.0, 3.0, 4.0], 100) == 4.0


def test_m1_improves_only_if_the_whole_interval_is_below_zero() -> None:
    ex = examples(400, 2)
    better = compare(ex, [0.5] * 400, [0.7] * 400, 1000, 0.95, 42)
    assert better.difference == pytest.approx(-0.2) and better.improves
    rnd = random.Random(3)  # noqa: S311
    noisy_m1 = [0.6 + rnd.gauss(0, 0.5) for _ in ex]
    noisy_b1 = [0.6 + rnd.gauss(0, 0.5) for _ in ex]
    tie = compare(ex, noisy_m1, noisy_b1, 1000, 0.95, 42)
    assert tie.low < 0 < tie.high and not tie.improves, "un empate no es mejora: se queda B1"
    worse = compare(ex, [0.7] * 400, [0.5] * 400, 1000, 0.95, 42)
    assert not worse.improves
    assert compare(ex, noisy_m1, noisy_b1, 1000, 0.95, 42) == tie, "reproducible"
    with pytest.raises(ValueError):
        compare(ex, [0.5], [0.5], 10, 0.95, 42)


def test_each_prediction_has_its_interval_or_says_insufficient_data() -> None:
    train = examples(300, 4)
    train += examples(10, 5, trig=("double_break",), score=7)
    cfg = CFG.interval.__class__(models=20, low_percentile=5, high_percentile=95)
    model = M1.fit(train, 0.01, CFG.training.seed, CFG.training.max_iterations)
    new = [examples(1, 6)[0], examples(1, 7, trig=("double_break",), score=7)[0]]
    a = predict_with_intervals(model, train, new, PROB, cfg, CFG.training)
    first = a[0]
    assert first is not None and first.low <= first.probability <= first.high
    assert first.low < first.high
    assert a[1] is None, "10 parecidas, el mínimo es 30: datos insuficientes"
    assert predict_with_intervals(model, train, new, PROB, cfg, CFG.training) == a
