"""Contratos del pronóstico y del dato real del calendario (ADR 0018, #695)."""

import json
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]


def validator(name: str) -> Draft202012Validator:
    doc = json.loads((ROOT / "contracts" / f"{name}.v1.schema.json").read_text())
    Draft202012Validator.check_schema(doc)
    return Draft202012Validator(doc)


ESTIMATE: dict[str, Any] = {
    "source": "forex_factory",
    "event": "CPI m/m",
    "country": "US",
    "scheduled_at": "2026-12-10T13:30:00Z",
    "forecast_raw": "0.3%",
    "previous_raw": "0.2%",
    "forecast_value": "0.3",
    "previous_value": "0.2",
    "unit": "percent",
    "seen_at": "2026-12-07T10:00:07Z",
}

ACTUAL: dict[str, Any] = {
    "source": "bls",
    "series_id": "CUSR0000SA0",
    "reference_period": "2026-11",
    "value": "337.912",
    "unit": "level",
    "vintage": "first",
    "released_at": "2026-12-10T13:30:00Z",
    "retrieved_at": "2026-12-11T14:05:00Z",
    "source_url": "https://api.bls.gov/publicAPI/v1/timeseries/data/",
}


def test_valid_examples_pass() -> None:
    validator("calendar_estimate").validate(ESTIMATE)
    validator("release_actual").validate(ACTUAL)
    empty = {
        **ESTIMATE,
        **dict.fromkeys(("forecast_raw", "forecast_value", "previous_value", "unit")),
        "previous_raw": "n/a",
    }
    validator("calendar_estimate").validate(empty)


@pytest.mark.parametrize(
    "over",
    [
        {"forecast_raw": None},  # número sin texto
        {"forecast_raw": ""},  # vacío va como null
        {"forecast_value": 0.3},  # nunca float
        {"unit": None},  # hay números: hace falta unidad
        {"forecast_value": None, "previous_value": None},  # sin números: sin unidad
        {"unit": "percent_points"},
        {"country": None},
        {"seen_at": "2026-12-07T10:00:07+00:00"},
        {"actual": "0.4%"},  # el dato real no va acá
    ],
)
def test_bad_estimates_are_rejected(over: dict[str, Any]) -> None:
    assert not validator("calendar_estimate").is_valid({**ESTIMATE, **over})


@pytest.mark.parametrize(
    "over",
    [
        {"source": "fred"},
        {"series_id": "cusr0000sa0"},
        {"reference_period": "2026-13"},
        {"reference_period": "2026-11-01"},
        {"value": 337.912},
        {"value": "-"},
        {"vintage": "latest"},
        {"source_url": "http://api.bls.gov/"},
        {"released_at": None},
    ],
)
def test_bad_actuals_are_rejected(over: dict[str, Any]) -> None:
    assert not validator("release_actual").is_valid({**ACTUAL, **over})
