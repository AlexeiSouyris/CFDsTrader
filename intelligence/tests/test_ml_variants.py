"""Variantes de ML v2 (#432), con datos sintéticos y sin base: contexto a la hora de la propuesta,
sin mirar el futuro, sin rellenar, y la regla de comparaciones múltiples pre-registrada."""

import math
import random
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest
from test_analogs import SMALL, bars
from test_ml_pipeline import FIRST, LAST, Synthetic, proposals, small_cfg

from cfds_intel.models.config import MlConfigError, load_config, load_default
from cfds_intel.models.context import AnalogFeature, Published, PublishedSeries
from cfds_intel.models.dataset import build
from cfds_intel.models.experiment import CONFIG_FILES
from cfds_intel.models.variants import Variant, augment, base_of, run_variant, treasury_variant

ROOT = Path(__file__).resolve().parents[2]
CFG = load_default()
TEXTS = {n: (ROOT / "config" / n).read_text() for n in CONFIG_FILES}


def at(d: date, hour: int) -> datetime:
    return datetime(d.year, d.month, d.day, hour, tzinfo=UTC)


D1, D2 = date(2024, 3, 4), date(2024, 3, 5)


# ---------- config: la regla pre-registrada ----------


def test_the_family_and_its_bonferroni_confidence_are_the_preregistered_ones() -> None:
    v = CFG.variants
    assert v.family == ("tasas", "vix", "analogos")
    assert v.family_error == 0.05
    assert v.confidence == pytest.approx(1 - 0.05 / 3)
    assert v.treasury_tenors == ("3M", "2Y", "10Y")
    base = (ROOT / "config" / "ml.toml").read_text()
    for old, new in [
        ('family = ["tasas", "vix", "analogos"]', "family = []"),
        ('family = ["tasas", "vix", "analogos"]', 'family = ["tasas", "tasas"]'),
        ('family = ["tasas", "vix", "analogos"]', 'family = ["sentimiento"]'),
        ('family_error = "0.05"', 'family_error = "1.5"'),
        ('treasury_tenors = ["3M", "2Y", "10Y"]', 'treasury_tenors = ["15Y"]'),
        ("max_age_days = 7", "max_age_days = 0"),
    ]:
        with pytest.raises(MlConfigError):
            load_config(base.replace(old, new))


# ---------- datos publicados: solo lo publicado hasta t ----------


def test_a_value_exists_only_from_its_publication() -> None:
    s = PublishedSeries([Published(D1, at(D2, 13), Decimal("4.1"))])
    assert s.at(at(D2, 12), 7) is None, "antes de publicarse no existe"
    assert s.at(at(D2, 13), 7) == Decimal("4.1"), "a la hora exacta de publicación, sí"


def test_a_correction_replaces_its_day_but_never_a_newer_day() -> None:
    s = PublishedSeries(
        [
            Published(D1, at(D2, 13), Decimal("4.1")),
            Published(D2, at(D2 + timedelta(days=1), 13), Decimal("4.3")),
            # Corrección de D1 publicada después del dato de D2: no lo pisa.
            Published(D1, at(D2 + timedelta(days=2), 13), Decimal("4.0")),
            # Corrección de D2: sí lo reemplaza desde su publicación.
            Published(D2, at(D2 + timedelta(days=3), 13), Decimal("4.4")),
        ]
    )
    assert s.at(at(D2, 20), 7) == Decimal("4.1")
    assert s.at(at(D2 + timedelta(days=2), 20), 7) == Decimal("4.3")
    assert s.at(at(D2 + timedelta(days=3), 20), 7) == Decimal("4.4")


def test_a_stale_value_is_missing_never_carried_forward() -> None:
    s = PublishedSeries([Published(D1, at(D2, 13), Decimal("4.1"))])
    assert s.at(at(D1 + timedelta(days=7), 20), 7) == Decimal("4.1")
    assert s.at(at(D1 + timedelta(days=8), 20), 7) is None


class FakeConn:
    """Solo `execute(...).fetchall()`, con filas por tenor."""

    def __init__(self, rows: dict[str, list[tuple[Any, ...]]]) -> None:
        self.rows = rows

    def execute(self, _sql: str, params: tuple[str, ...]) -> "FakeConn":
        self._last = self.rows[params[0]]
        return self

    def fetchall(self) -> list[tuple[Any, ...]]:
        return self._last


def test_a_zero_yield_is_data_not_missing() -> None:
    # 2020-2021: el 3M estuvo en 0,00 %. Un 0 no es un faltante.
    pub = at(D2, 13)
    conn = FakeConn(
        {
            "3M": [(D1, pub, Decimal("0.00"))],
            "2Y": [(D1, pub, Decimal("0.12"))],
            "10Y": [(D1, pub, Decimal("0.90"))],
        }
    )
    v = treasury_variant(conn, CFG)  # type: ignore[arg-type]
    assert v.features == ("yield_3M", "yield_2Y", "yield_10Y")
    assert v.lookup(at(D2, 15), True) == (0.0, 0.12, 0.9)
    assert v.lookup(at(D2, 12), True) is None


# ---------- análogos: la última diaria cerrada, sin mirar el futuro ----------


def series(n: int, seed: int) -> dict[str, list[Any]]:
    rnd = random.Random(seed)  # noqa: S311 (datos de prueba)
    closes, c = [], 100.0
    for _ in range(n):
        c += rnd.gauss(0, 1)
        closes.append(f"{c:.2f}")
    return {"US500.FUT": bars(closes)}


def test_the_analog_feature_uses_only_closed_dailies() -> None:
    s = series(60, 1)
    f = AnalogFeature(s, "US500.FUT", SMALL)
    day = s["US500.FUT"][40]
    before_close = day.end - timedelta(minutes=1)
    # Antes del cierre de la diaria 40 se usa la 39; al cierre, la 40.
    assert f.at(before_close, True) == AnalogFeature(s, "US500.FUT", SMALL).at(
        s["US500.FUT"][39].end, True
    )
    # Cambiar todas las diarias posteriores no cambia nada (§8.1).
    future = {"US500.FUT": s["US500.FUT"][:41] + series(60, 9)["US500.FUT"][41:]}
    assert AnalogFeature(future, "US500.FUT", SMALL).at(day.end, True) == f.at(day.end, True)
    assert AnalogFeature(future, "US500.FUT", SMALL).at(day.end, False) == f.at(day.end, False)


def test_the_analog_feature_is_per_side_and_missing_without_neighbors() -> None:
    s = series(60, 2)
    f = AnalogFeature(s, "US500.FUT", SMALL)
    t = s["US500.FUT"][50].end
    long, short = f.at(t, True), f.at(t, False)
    assert long is not None and short is not None
    assert 0 <= long <= 1 and 0 <= short <= 1 and long + short <= 1
    # Al principio no hay ventana ni vecinos: falta.
    assert f.at(s["US500.FUT"][1].end, True) is None
    assert f.at(s["US500.FUT"][0].end - timedelta(days=1), True) is None


# ---------- variantes: sin rellenar, misma base, regla corregida ----------


def base_dataset(seed: int = 1) -> Any:
    return build(proposals(seed), CFG.dataset, ".FUT")


def test_augment_appends_columns_and_counts_missing_apart() -> None:
    base = base_dataset()
    odd = Variant(
        "x", ("a", "b"), lambda t, long: None if t.day % 2 else (1.0, 2.0 if long else 3.0)
    )
    d = augment(base, odd)
    assert d.feature_names == (*base.feature_names, "a", "b")
    assert len(d.examples) + d.without_context == len(base.examples)
    assert d.without_context > 0 and all(e.signal_at.day % 2 == 0 for e in d.examples)
    side = base.feature_names.index("side_long")
    assert all(e.x[-1] == (2.0 if e.x[side] == 1.0 else 3.0) for e in d.examples)
    ref = base_of(d, base)
    assert ref.feature_names == base.feature_names
    assert [e.proposal_id for e in ref.examples] == [e.proposal_id for e in d.examples]


def informative(seed: int) -> Variant:
    """Una variable que de verdad explica `y` (la etiqueta sintética depende de tp1_r)."""
    rows = {r.signal_at: r for r in proposals(seed)}

    def lookup(t: datetime, long: bool) -> tuple[float, ...] | None:
        r = rows[t]
        return (float(abs(r.tp1 - r.reference) / abs(r.reference - r.stop)),)

    return Variant("informativa", ("leak_free_tp1_r",), lookup)


def noisy() -> Variant:
    """Una variable de ruido, la misma en cada llamada (semilla fija)."""
    rnd = random.Random(5)  # noqa: S311 (datos de prueba)
    return Variant("ruido", ("noise",), lambda t, long: (rnd.gauss(0, 1),))


def test_a_variant_runs_with_the_corrected_confidence_and_is_recorded_as_such() -> None:
    cfg = small_cfg()
    base = base_dataset()
    variant = noisy()
    src = Synthetic(tuple(proposals(1)))
    r = run_variant(base, variant, (FIRST, LAST), cfg, TEXTS, src)
    assert r.doc["hyperparameters"]["confidence"] == pytest.approx(1 - 0.05 / 3)
    assert r.doc["features"][-1] == "noise"
    assert r.comparison is not None
    # Con el intervalo corregido, más ancho que el de 95 %.
    plain = run_variant(
        base,
        noisy(),
        (FIRST, LAST),
        replace(cfg, variants=replace(cfg.variants, family=("tasas",))),
        TEXTS,
        src,
    )
    assert plain.comparison is not None
    assert plain.comparison.difference == pytest.approx(r.comparison.difference)
    assert (r.comparison.high - r.comparison.low) > (plain.comparison.high - plain.comparison.low)
    # La referencia (M1 sin la variante) usa exactamente las mismas propuestas.
    ref_val = [e.proposal_id for w in r.reference.windows for e in w.validation]
    var_val = [e.proposal_id for w in r.evaluation.windows for e in w.validation]
    assert ref_val == var_val


def test_the_variant_pipeline_is_reproducible() -> None:
    cfg = small_cfg()
    src = Synthetic(tuple(proposals(1)))
    a = run_variant(base_dataset(), informative(1), (FIRST, LAST), cfg, TEXTS, src)
    b = run_variant(base_dataset(), informative(1), (FIRST, LAST), cfg, TEXTS, src)
    assert a.evaluation == b.evaluation and a.doc["result"] == b.doc["result"]
    assert not math.isnan(a.evaluation.pooled["M1"].log_loss)
