"""VIX diario (ADR 0012 §2) con respuestas sintéticas: los números son inventados.

Los datos reales de Cboe nunca van al repo.
"""

import io
import json
import urllib.error
import urllib.request
from datetime import UTC, date, datetime
from decimal import Decimal
from email.message import Message
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.daily_publication import PublicationError, published_times
from cfds_intel.vix import cboe

ROOT = Path(__file__).resolve().parents[2]
CFG = cboe.load_config((ROOT / "config" / "vix.toml").read_text())
START = date(2019, 1, 1)

CSV = (
    "DATE,OPEN,HIGH,LOW,CLOSE\n"
    "02/11/1992,9.0,8.0,7.0,6.0\n"  # antes de start: incoherente a propósito, se ignora
    "11/19/2025,10.00,12.00,9.50,11.00\n"
    "11/20/2025,11.00,13.00,10.50,12.50\n"
    "11/21/2025,12.50,12.75,10.00,10.25\n"
)


def test_repo_config_is_valid() -> None:
    assert CFG.csv_url.startswith("https://cdn.cboe.com/")
    assert CFG.start == START
    assert str(CFG.publication_tz) == "America/New_York"


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ("https://cdn.cboe.com/", "http://cdn.cboe.com/"),
        ("https://cdn.cboe.com/", "https://espejo.example/"),
        ('time = "09:30"', 'time = "luego"'),
        ('start = "2019-01-01"', 'start = "ayer"'),
    ],
)
def test_bad_config_is_rejected(old: str, new: str) -> None:
    text = (ROOT / "config" / "vix.toml").read_text()
    assert old in text
    with pytest.raises(cboe.VixError):
        cboe.load_config(text.replace(old, new, 1))


def test_parse_keeps_rows_from_start_in_order() -> None:
    rows = cboe.parse_csv(CSV, START)
    assert [r[0] for r in rows] == [date(2025, 11, 19), date(2025, 11, 20), date(2025, 11, 21)]
    assert rows[0][1:] == (Decimal("10.00"), Decimal("12.00"), Decimal("9.50"), Decimal("11.00"))


@pytest.mark.parametrize(
    ("text", "match"),
    [
        ("<html>Just a moment...</html>", "anti-bots"),
        (CSV.replace("DATE,OPEN", "Date,Open"), "encabezado"),
        (CSV.replace("11/19/2025", "2025-11-19"), "fecha inválida"),
        (CSV.replace("10.00,12.00", "diez,12.00"), "no es un número"),
        (CSV.replace("10.00,12.00,9.50", "0,12.00,9.50"), "nivel"),
        (CSV.replace("10.00,12.00,9.50,11.00", "10.00,9.00,9.50,11.00"), "coherentes"),
        (CSV.replace("10.00,12.00,9.50,11.00", "10.00,12.00,9.50"), "celdas"),
    ],
)
def test_odd_responses_are_rejected(text: str, match: str) -> None:
    with pytest.raises(cboe.VixError, match=match):
        cboe.parse_csv(text, START)


def test_published_at_is_next_business_day_0930_et_or_read_time() -> None:
    rows = cboe.parse_csv(CSV, START)
    read = datetime(2025, 11, 21, 22, 0, tzinfo=UTC)
    days = cboe.to_days(rows, CFG, read)
    assert days[0].published_at == datetime(2025, 11, 20, 14, 30, tzinfo=UTC)
    assert days[1].published_at == datetime(2025, 11, 21, 14, 30, tzinfo=UTC)
    assert days[2].published_at == read


def test_never_before_the_close() -> None:
    rows = cboe.parse_csv(CSV, START)
    before_close = datetime(2025, 11, 21, 20, 0, tzinfo=UTC)  # 15:00 ET del último día
    with pytest.raises(cboe.VixError, match="antes"):
        cboe.to_days(rows, CFG, before_close)


def test_shared_publication_rule_rejects_unordered_days() -> None:
    ny = CFG.publication_tz
    with pytest.raises(PublicationError, match="desordenadas"):
        published_times(
            [date(2025, 1, 3), date(2025, 1, 2)],
            CFG.publication_time,
            ny,
            datetime(2025, 1, 4, tzinfo=UTC),
            cboe.CLOSE_AT,
            ny,
        )


def test_days_match_the_contract() -> None:
    schema = json.loads((ROOT / "contracts" / "vix_daily.v1.schema.json").read_text())
    v = Draft202012Validator(schema)
    days = cboe.to_days(cboe.parse_csv(CSV, START), CFG, datetime(2025, 11, 22, tzinfo=UTC))
    for d in days:
        assert not list(v.iter_errors(d.to_contract())), d


def test_fetch_asks_once_with_our_identity() -> None:
    seen: list[tuple[str, str | None, int]] = []

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        seen.append((req.full_url, req.get_header("User-agent"), timeout))
        return CSV.encode()

    assert cboe.fetch_csv(CFG, opener) == CSV
    assert seen == [(CFG.csv_url, CFG.user_agent, CFG.timeout_secs)]


def test_a_403_stops_without_retrying() -> None:
    calls = 0

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        nonlocal calls
        calls += 1
        raise urllib.error.HTTPError(req.full_url, 403, "Forbidden", Message(), io.BytesIO())

    with pytest.raises(cboe.VixError, match="403"):
        cboe.fetch_csv(CFG, opener)
    assert calls == 1
