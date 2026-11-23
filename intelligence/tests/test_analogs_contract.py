"""El informe de análogos como `analog_report.v1` (#294). Precios sintéticos."""

import json
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.analogs.config import load_config
from cfds_intel.analogs.contract import _dec, to_contract
from cfds_intel.analogs.search import Bar, analyze

ROOT = Path(__file__).resolve().parents[2]
CFG = load_config((ROOT / "config" / "analogs.toml").read_text())
SMALL = replace(
    CFG, window=3, dtw_band=1, neighbors=2, min_neighbors=2, horizons=(1, 2), r_horizon=2
)
SCHEMA = json.loads((ROOT / "contracts" / "analog_report.v1.schema.json").read_text())


def bars(closes: list[str]) -> list[Bar]:
    out = []
    for i, c in enumerate(closes):
        d = date(2024, 1, 1) + timedelta(days=i)
        close = Decimal(c)
        out.append(
            Bar(
                day=d,
                end=datetime(d.year, d.month, d.day, 22, tzinfo=UTC),
                high=close + Decimal("0.5"),
                low=close - Decimal("0.5"),
                close=close,
                atr=Decimal("1"),
                vol_tercile="mid",
            )
        )
    return out


def report_doc(source: str = "futures") -> dict[str, Any]:
    series = {
        "US500.FUT": bars([str(100 + (i * 7) % 11) for i in range(40)]),
        "US100.FUT": bars([str(200 + (i * 5) % 13) for i in range(40)]),
    }
    r = analyze(series, {}, "US500.FUT", 39, SMALL)
    return to_contract(r, series, source, SMALL, datetime(2026, 11, 23, 12, tzinfo=UTC))  # type: ignore[arg-type]


def test_the_report_follows_the_contract() -> None:
    doc = report_doc()
    errors = list(Draft202012Validator(SCHEMA).iter_errors(doc))
    assert not errors, errors
    assert doc["window_end_day"] == "2024-02-09"
    assert doc["as_of"] == "2024-02-09T22:00:00Z"
    assert doc["context"] == {"vol": "mid", "rates": None}
    assert [g["group"] for g in doc["groups"]] == ["all", "same_context"]
    assert len(doc["neighbors"]) == 2
    first = doc["groups"][0]["horizons"][0]
    assert [p["p"] for p in first["percentiles"]] == sorted(CFG.percentiles)


def test_the_schema_rejects_a_forecast_shaped_document() -> None:
    doc = report_doc()
    bad = [
        {**doc, "source": "mezcla"},
        {**doc, "prediction": "sube"},
        {**doc, "groups": doc["groups"][:1]},
        {**doc, "as_of": "2024-02-09T22:00:00+01:00"},
    ]
    v = Draft202012Validator(SCHEMA)
    for d in bad:
        assert list(v.iter_errors(d)), d


@pytest.mark.parametrize(
    ("value", "text"),
    [(Decimal("1E-5"), "0.00001"), (Decimal("-0.0123"), "-0.0123"), (Decimal("-0"), "0")],
)
def test_decimals_never_use_exponents(value: Decimal, text: str) -> None:
    assert _dec(value) == text


def test_naive_times_are_rejected() -> None:
    series = {"Q.FUT": bars([str(100 + (i * 7) % 11) for i in range(30)])}
    r = analyze(series, {}, "Q.FUT", 29, SMALL)
    with pytest.raises(ValueError):
        to_contract(r, series, "futures", SMALL, datetime(2026, 11, 23, 12))
