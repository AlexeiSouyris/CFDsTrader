"""Tasas en USD de la NY Fed (#467): configuración, lectura del JSON y hora de publicación.

Sin red: las respuestas son sintéticas, con el mismo formato que `search.json` de la API.
"""

import json
import urllib.request
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.rates import nyfed
from cfds_intel.rates.ecb import RatesError, to_rates

ROOT = Path(__file__).resolve().parents[2]
TEXT = (ROOT / "config" / "reference_rates.toml").read_text()
CFG = nyfed.load_config(TEXT)
SOFR = CFG.series["SOFR"]
EFFR = CFG.series["EFFR"]


def body(kind: str, *rows: tuple[str, object]) -> str:
    # Como la API: de más nuevo a más viejo, con campos que no se usan.
    return json.dumps(
        {
            "refRates": [
                {
                    "effectiveDate": day,
                    "type": kind,
                    "percentRate": value,
                    "volumeInBillions": 3141,
                    "revisionIndicator": "",
                }
                for day, value in rows
            ]
        }
    )


def utc(y: int, m: int, d: int, h: int = 0) -> datetime:
    return datetime(y, m, d, h, tzinfo=UTC)


# Jueves 19 y viernes 20, lunes 23 de noviembre de 2026 (horario de invierno, ET = UTC-5).
NOV = body("SOFR", ("2026-11-23", 3.93), ("2026-11-20", 3.92), ("2026-11-19", 3.91))
LATE = utc(2026, 11, 24, 12)


def test_real_config_loads() -> None:
    assert CFG.base_url == "https://markets.newyorkfed.org/api/rates"
    assert (SOFR.path, SOFR.type, SOFR.start) == ("secured/sofr", "SOFR", date(2018, 4, 2))
    assert (EFFR.path, EFFR.type) == ("unsecured/effr", "EFFR")
    assert EFFR.start <= date(2011, 1, 1)


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('base_url = "https://markets', 'base_url = "http://markets'),
        ('path = "secured/sofr"', 'path = "../sofr"'),
        ('type = "SOFR"', 'type = "sofr"'),
        ('start = "2018-04-02"', 'start = "2018-04-31"'),
        ('publication_timezone = "America/New_York"', 'publication_timezone = "Mars/Olympus"'),
        ("[nyfed]", "[fed]"),
    ],
)
def test_bad_config_is_rejected(old: str, new: str) -> None:
    assert old in TEXT
    with pytest.raises(RatesError):
        nyfed.load_config(TEXT.replace(old, new))


def test_parse_orders_dates_and_keeps_exact_decimals() -> None:
    assert nyfed.parse_json(NOV, SOFR) == [
        (date(2026, 11, 19), Decimal("3.91")),
        (date(2026, 11, 20), Decimal("3.92")),
        (date(2026, 11, 23), Decimal("3.93")),
    ]
    # Un decimal con más cifras no pasa por float.
    assert nyfed.parse_json(
        '{"refRates": [{"effectiveDate": "2011-01-03", "type": "SOFR", '
        '"percentRate": 0.123456789012345678}]}',
        SOFR,
    ) == [(date(2011, 1, 3), Decimal("0.123456789012345678"))]


@pytest.mark.parametrize(
    "bad",
    [
        "no es json",
        "{}",
        '{"refRates": {}}',
        body("EFFR", ("2026-11-19", 3.9)),
        body("SOFR", ("2026-13-19", 3.9)),
        body("SOFR", ("2026-11-19", "3.9")),
        body("SOFR", ("2026-11-19", None)),
        body("SOFR", ("2026-11-19", 3.9), ("2026-11-19", 3.9)),
        '{"refRates": [1]}',
    ],
)
def test_anything_odd_rejects_the_whole_response(bad: str) -> None:
    with pytest.raises(RatesError):
        nyfed.parse_json(bad, SOFR)


def test_published_the_next_business_day_after_the_revision_window() -> None:
    rates = to_rates(nyfed.parse_json(NOV, SOFR), SOFR, LATE)
    # El del jueves se publica el viernes; el del viernes, el lunes. A las 15:00 ET = 20:00 UTC.
    assert rates[0].published_at == utc(2026, 11, 20, 20)
    assert rates[1].published_at == utc(2026, 11, 23, 20)
    assert rates[-1].published_at == LATE


def test_summer_time_uses_edt() -> None:
    july = body("EFFR", ("2026-07-10", 4.33), ("2026-07-09", 4.33))
    rates = to_rates(nyfed.parse_json(july, EFFR), EFFR, LATE)
    assert rates[0].published_at == utc(2026, 7, 10, 19)


def test_rates_match_the_contract() -> None:
    schema = json.loads((ROOT / "contracts" / "reference_rate.v1.schema.json").read_text())
    v = Draft202012Validator(schema)
    for r in to_rates(nyfed.parse_json(NOV, SOFR), SOFR, LATE):
        assert list(v.iter_errors(r.to_contract())) == []
    assert to_rates(nyfed.parse_json(NOV, SOFR), SOFR, LATE)[0].to_contract() == {
        "series": "SOFR",
        "ref_date": "2026-11-19",
        "published_at": "2026-11-20T20:00:00Z",
        "rate_pct": "3.91",
    }


def test_fetch_asks_the_configured_series_and_dates() -> None:
    seen: dict[str, object] = {}

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        seen["url"] = req.full_url
        seen["accept"] = req.get_header("Accept")
        seen["timeout"] = timeout
        return NOV.encode()

    got = nyfed.fetch_json(CFG, SOFR, date(2026, 11, 19), date(2026, 11, 24), opener)
    assert got == NOV
    assert seen["url"] == (
        "https://markets.newyorkfed.org/api/rates/secured/sofr/search.json"
        "?startDate=2026-11-19&endDate=2026-11-24"
    )
    assert seen["accept"] == "application/json"
    assert seen["timeout"] == CFG.timeout_secs
