"""`forecast` y `previous` de Forex Factory como instantáneas (ADR 0018 §1, #696), sin red."""

import json
from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.calendar import CalendarError, build_estimate, load_config, parse_feed, parse_value

ROOT = Path(__file__).resolve().parents[2]
CFG = load_config((ROOT / "config" / "calendar.toml").read_text())
FIXTURE = (Path(__file__).parent / "fixtures" / "ff_calendar_synthetic.json").read_bytes()
NOW = datetime(2026, 11, 23, 10, 0, 7, 123456, tzinfo=UTC)
AT = datetime(2026, 12, 10, 13, 30, tzinfo=UTC)


def schema() -> Draft202012Validator:
    doc = json.loads((ROOT / "contracts" / "calendar_estimate.v1.schema.json").read_text())
    return Draft202012Validator(doc)


def feed(*events: dict[str, object]) -> bytes:
    return json.dumps(list(events)).encode()


def ev(**over: object) -> dict[str, object]:
    base: dict[str, object] = {
        "title": "CPI m/m",
        "country": "USD",
        "date": "2026-12-10T08:30:00-05:00",
        "impact": "High",
        "forecast": "0.3%",
        "previous": "0.2%",
    }
    return {**base, **over}


@pytest.mark.parametrize(
    ("raw", "want"),
    [
        ("0.3%", (Decimal("0.3"), "percent")),
        ("-0.1%", (Decimal("-0.1"), "percent")),
        ("215K", (Decimal("215"), "thousands")),
        ("-1.2B", (Decimal("-1.2"), "billions")),
        ("3.5M", (Decimal("3.5"), "millions")),
        ("1.01T", (Decimal("1.01"), "trillions")),
        ("52.3", (Decimal("52.3"), "level")),
        ("4.50%", (Decimal("4.50"), "percent")),  # conserva la escala
        (None, None),
        ("<0.10%", None),
        ("1.2|1.3", None),
        ("0,3%", None),
        ("+0.3%", None),
        ("0.3 %", None),
        ("215k", None),
        (".3%", None),
        ("%", None),
    ],
)
def test_only_unambiguous_numbers_are_read(raw: str | None, want: object) -> None:
    assert parse_value(raw) == want


def test_one_unit_for_both_values_or_none() -> None:
    def est(f: str | None, p: str | None) -> tuple[object, object, object]:
        e = build_estimate("forex_factory", "X", "US", AT, f, p, NOW)
        return (e.forecast_value, e.previous_value, e.unit)

    assert est("215K", "140K") == (Decimal("215"), Decimal("140"), "thousands")
    assert est("215K", None) == (Decimal("215"), None, "thousands")
    assert est(None, "4.1%") == (None, Decimal("4.1"), "percent")
    assert est("<0.1%", "0.2%") == (None, Decimal("0.2"), "percent")
    # Unidades distintas: no se sabe cuál vale para los dos; texto sí, números no.
    assert est("215K", "1.2M") == (None, None, None)
    assert est(None, None) == (None, None, None)


def test_estimates_follow_the_contract_and_come_from_the_same_read() -> None:
    p = parse_feed(FIXTURE, CFG, NOW)
    docs = [e.to_contract() for e in p.estimates]
    v = schema()
    for d in docs:
        assert not list(v.iter_errors(d)), d
    # Uno por evento guardado: ni el feriado, ni la moneda desconocida, ni el repetido.
    assert len(docs) == len(p.events) == 3
    assert docs[0] == {
        "source": "forex_factory",
        "event": "Evento de prueba de empleo",
        "country": "US",
        "scheduled_at": "2026-11-27T13:30:00Z",
        "forecast_raw": "150K",
        "previous_raw": "140K",
        "forecast_value": "150",
        "previous_value": "140",
        "unit": "thousands",
        "seen_at": "2026-11-23T10:00:07Z",  # la lectura, no la hora del evento
    }
    assert docs[1]["forecast_raw"] is None and docs[1]["unit"] is None  # "" es null
    assert p.ambiguous_estimates == 0


def test_dst_the_same_0830_et_is_a_different_utc_hour_and_a_different_event() -> None:
    # 2026-03-08: EE. UU. pasa a horario de verano. Antes, 08:30 ET = 13:30Z; después, 12:30Z.
    body = feed(
        ev(title="Antes", date="2026-03-06T08:30:00-05:00"),
        ev(title="Después", date="2026-03-11T08:30:00-04:00"),
    )
    got = {e.event: e.scheduled_at for e in parse_feed(body, CFG, NOW).estimates}
    assert got == {
        "Antes": datetime(2026, 3, 6, 13, 30, tzinfo=UTC),
        "Después": datetime(2026, 3, 11, 12, 30, tzinfo=UTC),
    }


def test_raw_text_is_kept_trimmed_and_missing_fields_are_null() -> None:
    item = ev(forecast=" 0.3% ")
    del item["previous"]
    (e,) = parse_feed(feed(item), CFG, NOW).estimates
    assert (e.forecast_raw, e.previous_raw, e.forecast_value) == ("0.3%", None, Decimal("0.3"))


def test_a_repeated_event_with_two_values_keeps_the_event_but_no_estimate() -> None:
    p = parse_feed(feed(ev(), ev(forecast="0.4%")), CFG, NOW)
    assert len(p.events) == 1
    assert p.estimates == [] and p.ambiguous_estimates == 1


@pytest.mark.parametrize("bad", [{"forecast": 0.3}, {"previous": ["1"]}, {"forecast": "9" * 51}])
def test_a_bad_forecast_or_previous_fails_the_whole_feed(bad: dict[str, object]) -> None:
    with pytest.raises(CalendarError):
        parse_feed(feed(ev(), ev(title="Otro", **bad)), CFG, NOW)
