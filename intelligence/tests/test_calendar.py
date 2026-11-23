"""Calendario de Forex Factory por su feed JSON oficial (ADR 0010, #239), con respuestas
grabadas sintéticas y sin red."""

import json
import urllib.request
from datetime import UTC, datetime
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.calendar import CalendarError, fetch, load_config, parse_feed

ROOT = Path(__file__).resolve().parents[2]
CONFIG_TEXT = (ROOT / "config" / "calendar.toml").read_text()
CFG = load_config(CONFIG_TEXT)
FIXTURE = (Path(__file__).parent / "fixtures" / "ff_calendar_synthetic.json").read_bytes()
NOW = datetime(2026, 11, 23, 10, 0, 7, 123456, tzinfo=UTC)


def schema() -> Draft202012Validator:
    doc = json.loads((ROOT / "contracts" / "economic_event.v1.schema.json").read_text())
    return Draft202012Validator(doc)


def feed(*events: dict[str, object]) -> bytes:
    return json.dumps(list(events)).encode()


def ev(**over: object) -> dict[str, object]:
    base: dict[str, object] = {
        "title": "Evento",
        "country": "USD",
        "date": "2026-11-27T08:30:00-05:00",
        "impact": "High",
        "forecast": "",
        "previous": "",
    }
    return {**base, **over}


def test_the_repository_config_is_honest_and_maps_currencies_to_countries() -> None:
    assert CFG.source == "forex_factory"
    assert CFG.url == "https://nfs.faireconomy.media/ff_calendar_thisweek.json"
    assert "Mozilla" not in CFG.user_agent  # no se hace pasar por un navegador
    assert CFG.currencies["USD"] == "US"
    assert CFG.currencies["EUR"] == "EU"
    assert CFG.currencies["GBP"] == "GB"
    assert CFG.currencies["JPY"] == "JP"


def test_events_follow_the_contract_in_utc_and_published_when_first_read() -> None:
    p = parse_feed(FIXTURE, CFG, NOW)
    docs = [e.to_contract() for e in p.events]
    v = schema()
    for d in docs:
        assert not list(v.iter_errors(d)), d
    nfp = docs[0]
    assert nfp == {
        "source": "forex_factory",
        "event": "Evento de prueba de empleo",
        "country": "US",
        "instrument_id": None,
        "impact": "high",
        "scheduled_at": "2026-11-27T13:30:00Z",  # 08:30 -05:00 en UTC
        "published_at": "2026-11-23T10:00:07Z",  # cuando lo leímos, no la hora del evento
    }
    assert [d["impact"] for d in docs] == ["high", "medium", "low"]
    assert [d["country"] for d in docs] == ["US", "EU", "GB"]


def test_holidays_unknown_currencies_and_repeats_are_not_stored() -> None:
    p = parse_feed(FIXTURE, CFG, NOW)
    assert len(p.events) == 3  # el repetido cuenta una vez
    assert p.holidays == 1
    assert p.unknown_currencies == {"All": 1}


@pytest.mark.parametrize(
    "bad",
    [
        ev(date="2026-11-27T08:30:00"),  # sin zona horaria: no se supone
        ev(date="27/11/2026 08:30"),
        ev(date=None),
        ev(impact="Extreme"),
        ev(impact="high"),
        ev(title=""),
        ev(title="x" * 201),
        ev(country=None),
        "no es un objeto",
    ],
)
def test_any_bad_event_fails_the_whole_feed(bad: object) -> None:
    with pytest.raises(CalendarError):
        parse_feed(json.dumps([ev(), bad]).encode(), CFG, NOW)


def test_bad_bodies_and_times_are_errors() -> None:
    for body in [b"<html>Rate Limited</html>", b'{"title": "x"}', b"\xff"]:
        with pytest.raises(CalendarError):
            parse_feed(body, CFG, NOW)
    with pytest.raises(CalendarError):
        parse_feed(feed(ev()), CFG, datetime(2026, 11, 23, 10, 0))  # sin zona


def test_bad_config_is_rejected() -> None:
    for bad in [
        CONFIG_TEXT.replace('url = "https://', 'url = "http://'),
        CONFIG_TEXT.replace('name = "forex_factory"', 'name = "Forex Factory"'),
        CONFIG_TEXT.replace('USD = "US"', 'USD = "USA"'),
        CONFIG_TEXT.replace("timeout_secs = 30", "timeout_secs = 0"),
        CONFIG_TEXT + "\n[otra]\nx = 1\n",
    ]:
        with pytest.raises(CalendarError):
            load_config(bad)


def test_fetch_sends_the_honest_user_agent_to_the_configured_url() -> None:
    seen: list[urllib.request.Request] = []

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        seen.append(req)
        assert timeout == CFG.timeout_secs
        return FIXTURE

    p = fetch(CFG, opener)
    assert len(p.events) == 3
    assert seen[0].full_url == CFG.url
    assert seen[0].get_header("User-agent") == CFG.user_agent
