"""Índice de sentimiento v1 (spec §9.1, #213): funciones puras, sin base ni red."""

import json
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.sentiment.index import (
    COT_LONG,
    COT_OPEN_INTEREST,
    COT_SHORT,
    IndexConfig,
    NotEmitted,
    Point,
    SentimentConfigError,
    SentimentIndex,
    cot_points,
    index_at,
    index_series,
    load_config,
)

ROOT = Path(__file__).resolve().parents[2]
CONFIG = (ROOT / "config" / "sentiment.toml").read_text()
T0 = datetime(2020, 1, 1, 23, tzinfo=UTC)


def cfg(min_components: int = 3) -> IndexConfig:
    return load_config(CONFIG.replace("min_components = 3", f"min_components = {min_components}"))


def daily(values: list[int], start: datetime = T0, step: int = 1) -> list[Point]:
    """Un dato por día: corresponde a ese día y se publica a las 23:00 UTC."""
    return [
        Point(
            (start + timedelta(days=i * step)).replace(hour=0),
            start + timedelta(days=i * step),
            Decimal(v),
        )
        for i, v in enumerate(values)
    ]


def history(n: int, last: int) -> list[Point]:
    """`n` días con valores 0..n-1 y un último dato `last`, más de 3 años después del primero."""
    return daily([*range(n - 1), last], step=1100 // (n - 1) + 1)


def test_the_real_config_loads() -> None:
    c = load_config(CONFIG)
    assert c.instruments == ("US500", "US100", "US30")
    assert (c.warmup_years, c.min_components) == (3, 3)
    names = {x.name: x for x in c.components}
    assert names["realized_vol"].inverted and not names["drawdown"].inverted
    assert names["cot"].expiry == timedelta(days=21)
    assert names["breadth"].pending and names["breadth"].applies_to("US100")
    assert not names["breadth"].applies_to("US500")


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ("min_components = 3", "min_components = 4"),  # más de los que tienen datos
        ("warmup_years = 3", "warmup_years = 0"),
        ('weight = "1"', 'weight = "-1"'),
        ("expiry_days = 21", "expiry_days = 0"),
        ('instruments = ["US500", "US100", "US30"]', 'instruments = ["us500"]'),
        ("inverted = true", "inverted = true\ntypo = 1"),
    ],
)
def test_a_bad_config_is_rejected(old: str, new: str) -> None:
    with pytest.raises(SentimentConfigError):
        load_config(CONFIG.replace(old, new, 1))


def test_percentile_counts_ties_as_half_and_excludes_the_current_value() -> None:
    # Ventana: 0, 1, 2, 2 (el dato actual, 2, no entra). Menores: 2, iguales: 2 → (2 + 1) / 4.
    pts = daily([0, 1, 2, 2, 2], step=274)  # el primero, justo 3 años antes del último
    data = {"cot": pts, "realized_vol": pts, "drawdown": pts}
    r = index_at("US500", data, cfg(), pts[-1].published_at)
    assert isinstance(r, SentimentIndex)
    cot = r.components[0]
    assert cot.percentile == Decimal("0.75")
    assert cot.component == Decimal("0.5")
    vol = next(c for c in r.components if c.name == "realized_vol")
    assert vol.component == Decimal("-0.5")  # invertido: alta volatilidad = miedo
    assert r.value == Decimal("0.5") / 3  # (0,5 - 0,5 + 0,5) / 3


def test_the_window_only_has_the_last_three_years() -> None:
    # 10 y 20, viejos (más de 3 años antes del actual), no cuentan: solo 5, 6 y 7.
    old = daily([10, 20], start=T0)
    recent = daily([5, 6, 7, 6], start=T0 + timedelta(days=1200), step=30)
    r = index_at("US500", {"cot": old + recent}, cfg(1), recent[-1].published_at)
    assert isinstance(r, SentimentIndex)
    assert r.components[0].percentile == Decimal("0.5")  # (1 + 0,5) / 3


def test_warmup_needs_three_years_of_published_history() -> None:
    pts = daily([1, 2, 3], step=500)  # el último, a 1000 días del primero
    r = index_at("US500", {"cot": pts}, cfg(1), pts[-1].published_at)
    assert isinstance(r, NotEmitted)
    assert r.components[0].status == "warming_up"
    longer = daily([1, 2, 3], step=550)  # 1100 días
    assert isinstance(
        index_at("US500", {"cot": longer}, cfg(1), longer[-1].published_at), SentimentIndex
    )


def test_each_component_expires_on_its_own() -> None:
    pts = history(40, 5)
    at = pts[-1].observed_for
    data = {n: pts for n in ("cot", "realized_vol", "drawdown")}
    ok = index_at("US500", data, cfg(), at + timedelta(days=4, hours=23))
    assert isinstance(ok, SentimentIndex)
    # A los 5 días vencen las medidas diarias, no el COT (21 días).
    late = index_at("US500", data, cfg(1), at + timedelta(days=5))
    assert isinstance(late, SentimentIndex)
    states = {c.name: c.status for c in late.components}
    assert states == {
        "cot": "valid",
        "realized_vol": "expired",
        "drawdown": "expired",
        "new_highs_lows": "pending_data",
    }
    assert late.value == ok.components[0].component


def test_below_the_minimum_the_index_is_not_emitted_and_says_why() -> None:
    pts = history(40, 5)
    r = index_at("US100", {"cot": pts, "realized_vol": pts}, cfg(), pts[-1].published_at)
    assert isinstance(r, NotEmitted)
    assert r.valid == 2
    why = r.reason()
    assert "2 componentes válidos de 3" in why
    assert "drawdown: missing" in why and "breadth: pending_data" in why


def test_nothing_published_later_is_used() -> None:
    pts = history(40, 5)
    later = Point(
        pts[-1].observed_for + timedelta(days=1),
        pts[-1].published_at + timedelta(days=1),
        Decimal(-9),
    )
    data = {n: [*pts, later] for n in ("cot", "realized_vol", "drawdown")}
    r = index_at("US500", data, cfg(), later.published_at - timedelta(seconds=1))
    assert isinstance(r, SentimentIndex)
    assert r.published_at == pts[-1].published_at
    assert all(c.current == pts[-1] for c in r.components if c.current)


def test_the_series_never_repaints() -> None:
    cot = history(60, 50)
    vol = history(70, 3)
    dd = history(80, 40)
    data = {"cot": cot, "realized_vol": vol, "drawdown": dd}
    full, _ = index_series("US500", data, cfg())
    assert full
    # Cortando los datos en cualquier momento, lo ya emitido es idéntico.
    for cut in (full[0].published_at, full[len(full) // 2].published_at):
        partial = {k: [p for p in v if p.published_at <= cut] for k, v in data.items()}
        got, _ = index_series("US500", partial, cfg())
        assert got == [r for r in full if r.published_at <= cut]


def test_a_correction_is_a_new_point_and_does_not_repaint() -> None:
    pts = history(40, 5)
    fix = Point(pts[-1].observed_for, pts[-1].published_at + timedelta(days=1), Decimal(100))
    data = {"cot": [*pts, fix], "realized_vol": pts, "drawdown": pts}
    series, _ = index_series("US500", data, cfg())
    first, corrected = series[-2], series[-1]
    assert first.published_at == pts[-1].published_at
    assert corrected.published_at == fix.published_at
    assert corrected.components[0].current == fix
    assert first.value != corrected.value


def test_cot_raw_value_and_its_corrections() -> None:
    week = datetime(2026, 11, 10, tzinfo=UTC)
    pub = week + timedelta(days=7)
    rows = [
        (COT_LONG, week, pub, Decimal(300)),
        (COT_SHORT, week, pub, Decimal(100)),
        (COT_OPEN_INTEREST, week, pub, Decimal(1000)),
        ("tff_dealer_long", week, pub, Decimal(5)),  # otra serie: no cuenta
        (COT_SHORT, week, pub + timedelta(days=2), Decimal(200)),  # corrección
        (COT_LONG, week + timedelta(days=7), pub, Decimal(1)),  # informe incompleto
    ]
    assert cot_points(rows) == [
        Point(week, pub, Decimal("0.2")),
        Point(week, pub + timedelta(days=2), Decimal("0.1")),
    ]
    zero_oi = [(s, week, pub, Decimal(0)) for s in (COT_LONG, COT_SHORT, COT_OPEN_INTEREST)]
    assert cot_points(zero_oi) == []


def test_the_record_validates_against_the_contract() -> None:
    schema = json.loads((ROOT / "contracts" / "sentiment.v1.schema.json").read_text())
    pts = history(40, 5)
    r = index_at(
        "US100", {n: pts for n in ("cot", "realized_vol", "drawdown")}, cfg(), pts[-1].published_at
    )
    assert isinstance(r, SentimentIndex)
    doc = r.to_contract()
    Draft202012Validator(schema).validate(doc)
    assert doc["published_at"].endswith("Z")
    assert -1 <= Decimal(doc["value"]) <= 1
    names = [c["name"] for c in doc["components"]]
    assert names == ["cot", "realized_vol", "drawdown", "breadth", "new_highs_lows"]
    bad = {**doc, "value": 0.5}
    assert not Draft202012Validator(schema).is_valid(bad)
