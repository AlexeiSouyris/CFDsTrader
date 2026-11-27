"""Tipos de referencia del BCE (#79): configuración, lectura del CSV y hora de publicación.

Sin red: el CSV es sintético, con el mismo formato que `csvdata` de la API del BCE.
"""

import json
import urllib.request
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.rates import ecb
from cfds_intel.rates.ecb import RatesError, load_config, parse_csv, to_rates

ROOT = Path(__file__).resolve().parents[2]
CFG = load_config((ROOT / "config" / "reference_rates.toml").read_text())
ESTR = CFG.series["ESTR"]
HEADER = "KEY,FREQ,BENCHMARK_ITEM,DATA_TYPE_EST,TIME_PERIOD,OBS_VALUE,OBS_STATUS,TITLE"


def csv_of(*rows: tuple[str, str]) -> str:
    lines = [HEADER]
    for day, value in rows:
        lines.append(
            f"EST.B.EU000A2X2A25.WT,B,EU000A2X2A25,WT,{day},{value},A,Euro short-term rate"
        )
    return "\n".join(lines) + "\n"


def utc(y: int, m: int, d: int, h: int = 0, mi: int = 0) -> datetime:
    return datetime(y, m, d, h, mi, tzinfo=UTC)


# Jueves, viernes y lunes de noviembre (horario de invierno, CET = UTC+1).
NOV = csv_of(("2026-11-12", "2.443"), ("2026-11-13", "2.437"), ("2026-11-16", "2.44"))
LATE = utc(2026, 11, 22, 12)


def test_real_config_loads() -> None:
    assert ESTR.flow == "EST"
    assert ESTR.key == "B.EU000A2X2A25.WT"
    assert ESTR.start == date(2019, 10, 1)
    assert CFG.base_url.startswith("https://")


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('base_url = "https://', 'base_url = "http://'),
        ('flow = "EST"', 'flow = "est"'),
        ('key = "B.EU000A2X2A25.WT"', 'key = "B..WT"'),
        ('publication_timezone = "Europe/Berlin"', 'publication_timezone = "Mars/Olympus"'),
        ("timeout_secs = 60", "timeout_secs = 0"),
    ],
)
def test_bad_config_is_rejected(old: str, new: str) -> None:
    text = (ROOT / "config" / "reference_rates.toml").read_text()
    assert old in text
    with pytest.raises(RatesError):
        load_config(text.replace(old, new))


def test_parse_reads_dates_and_exact_decimals() -> None:
    assert parse_csv(NOV, ESTR) == [
        (date(2026, 11, 12), Decimal("2.443")),
        (date(2026, 11, 13), Decimal("2.437")),
        (date(2026, 11, 16), Decimal("2.44")),
    ]


def test_negative_rates_are_valid() -> None:
    assert parse_csv(csv_of(("2021-03-01", "-0.567")), ESTR) == [
        (date(2021, 3, 1), Decimal("-0.567"))
    ]


@pytest.mark.parametrize(
    "bad",
    [
        NOV.replace("EST.B.EU000A2X2A25.WT", "EST.B.OTHER.WT", 1),
        csv_of(("2026-11-12", "abc")),
        csv_of(("2026-11-12", "NaN")),
        csv_of(("2026-13-12", "2.4")),
        csv_of(("2026-11-13", "2.4"), ("2026-11-12", "2.4")),
        csv_of(("2026-11-12", "2.4"), ("2026-11-12", "2.4")),
        "KEY,TIME_PERIOD\nEST.B.EU000A2X2A25.WT,2026-11-12\n",
    ],
)
def test_anything_odd_rejects_the_whole_response(bad: str) -> None:
    with pytest.raises(RatesError):
        parse_csv(bad, ESTR)


def test_published_the_next_business_day_at_nine_cet() -> None:
    rates = to_rates(parse_csv(NOV, ESTR), ESTR, LATE)
    # El del jueves se publica el viernes; el del viernes, el lunes (no el sábado).
    assert rates[0].published_at == utc(2026, 11, 13, 8)
    assert rates[1].published_at == utc(2026, 11, 16, 8)


def test_last_value_is_available_from_when_it_was_read() -> None:
    rates = to_rates(parse_csv(NOV, ESTR), ESTR, LATE)
    assert rates[-1].published_at == LATE


def test_summer_time_uses_cest() -> None:
    july = csv_of(("2026-07-09", "1.9"), ("2026-07-10", "1.91"))
    rates = to_rates(parse_csv(july, ESTR), ESTR, LATE)
    assert rates[0].published_at == utc(2026, 7, 10, 7)


def test_read_before_the_day_ended_is_an_error() -> None:
    with pytest.raises(RatesError):
        to_rates(parse_csv(NOV, ESTR)[:1], ESTR, utc(2026, 11, 12, 15))


def test_retrieved_at_needs_a_timezone() -> None:
    with pytest.raises(RatesError):
        to_rates(parse_csv(NOV, ESTR), ESTR, datetime(2026, 11, 22, 12))


def test_rates_match_the_contract() -> None:
    schema = json.loads((ROOT / "contracts" / "reference_rate.v1.schema.json").read_text())
    v = Draft202012Validator(schema)
    rates = to_rates(parse_csv(NOV, ESTR), ESTR, LATE)
    for r in rates:
        assert list(v.iter_errors(r.to_contract())) == []
    assert rates[0].to_contract() == {
        "series": "ESTR",
        "ref_date": "2026-11-12",
        "published_at": "2026-11-13T08:00:00Z",
        "rate_pct": "2.443",
    }


def test_fetch_asks_the_configured_series_in_csv() -> None:
    seen: dict[str, object] = {}

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        seen["url"] = req.full_url
        seen["accept"] = req.get_header("Accept")
        seen["timeout"] = timeout
        return NOV.encode()

    assert ecb.fetch_csv(CFG, ESTR, date(2026, 11, 12), opener) == NOV
    assert seen["url"] == (
        "https://data-api.ecb.europa.eu/service/data/EST/B.EU000A2X2A25.WT"
        "?startPeriod=2026-11-12&format=csvdata"
    )
    assert seen["accept"] == "text/csv"
    assert seen["timeout"] == CFG.timeout_secs


def test_estr_skips_holidays_without_empty_rows() -> None:
    # CSV real del BCE (#447), del 27-03 al 03-04-2024: el €STR (frecuencia B, días hábiles
    # TARGET) no trae fila para Viernes Santo ni Lunes de Pascua. El del jueves 28-03 rige
    # hasta que se publica el siguiente, el martes 02-04 a las 9:00 CEST.
    real = Path(__file__).resolve().parent / "fixtures" / "ecb_estr_2024-03-27_04-03.csv"
    values = parse_csv(real.read_text(), ESTR)
    assert [d for d, _ in values] == [
        date(2024, 3, 27),
        date(2024, 3, 28),
        date(2024, 4, 2),
        date(2024, 4, 3),
    ]
    rates = to_rates(values, ESTR, LATE)
    assert rates[1].published_at == utc(2024, 4, 2, 7)
    # Si el BCE mandara la fila vacía, como en el EUR/USD, el resultado sería el mismo.
    with_empty = csv_of(("2024-03-28", "3.899"), ("2024-03-29", ""), ("2024-04-02", "3.906"))
    assert to_rates(parse_csv(with_empty, ESTR), ESTR, LATE)[0].published_at == utc(2024, 4, 2, 7)
