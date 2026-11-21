"""COT de la CFTC (#113) con filas sintéticas y sin red."""

import json
import urllib.parse
import urllib.request
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.sentiment.cot import (
    CotError,
    available_from,
    fetch_rows,
    load_config,
    to_observations,
)

ROOT = Path(__file__).resolve().parents[2]
REAL = (ROOT / "config" / "cot.toml").read_text()
CFG = load_config(REAL)


def row(
    code: str = "20974+", report: str = "2026-11-17T00:00:00.000", **over: object
) -> dict[str, object]:
    # Valores inventados, con la forma de la API (enteros como texto).
    r: dict[str, object] = {
        "cftc_contract_market_code": code,
        "report_date_as_yyyy_mm_dd": report,
    }
    r.update({col: str(1000 + i) for i, col in enumerate(CFG.series)})
    r.update(over)
    return r


LATE = datetime(2026, 12, 1, tzinfo=UTC)


def test_the_tuesday_report_counts_from_the_next_tuesday_at_1530_new_york_time() -> None:
    # Invierno (EST, UTC-5) y verano (EDT, UTC-4).
    assert available_from(date(2026, 11, 17), CFG) == datetime(2026, 11, 24, 20, 30, tzinfo=UTC)
    assert available_from(date(2026, 7, 14), CFG) == datetime(2026, 7, 21, 19, 30, tzinfo=UTC)


def test_rows_become_contract_observations() -> None:
    obs = to_observations([row(), row("13874+")], CFG, LATE)
    assert len(obs) == 2 * len(CFG.series)
    schema = json.loads((ROOT / "contracts" / "sentiment_observation.v1.schema.json").read_text())
    v = Draft202012Validator(schema)
    for o in obs:
        assert list(v.iter_errors(o.to_contract())) == []
    first = obs[0].to_contract()
    assert first == {
        "source": "cftc_cot",
        "series": "tff_open_interest",
        "instrument_id": "US100",
        "observed_for": "2026-11-17T00:00:00Z",
        "published_at": "2026-11-24T20:30:00Z",
        "value": "1000",
    }
    assert {o.instrument_id for o in obs} == {"US100", "US500"}


def test_reading_it_earlier_proves_it_was_already_published() -> None:
    # En vivo: leído el viernes a la noche, vale desde que se leyó (antes del martes siguiente).
    friday = datetime(2026, 11, 20, 21, 0, tzinfo=UTC)
    obs = to_observations([row()], CFG, friday)
    assert {o.published_at for o in obs} == {friday}


@pytest.mark.parametrize(
    "bad",
    [
        row("999999"),  # mercado no pedido
        row(open_interest_all=None),
        row(open_interest_all="12.5"),
        row(open_interest_all="-3"),
        row(open_interest_all="n/a"),
        row(report="2026-11-17T15:30:00.000"),
        row(report="17/11/2026"),
    ],
)
def test_a_bad_row_rejects_everything(bad: dict[str, object]) -> None:
    with pytest.raises(CotError):
        to_observations([row(), bad], CFG, LATE)


def test_retrieval_needs_a_time_zone_and_cannot_precede_the_report() -> None:
    with pytest.raises(CotError):
        to_observations([row()], CFG, datetime(2026, 12, 1))
    with pytest.raises(CotError):
        to_observations([row()], CFG, datetime(2026, 11, 16, tzinfo=UTC))


def test_repository_config() -> None:
    assert CFG.markets == {"20974+": "US100", "13874+": "US500", "124603": "US30"}
    assert len(CFG.series) == 11
    assert CFG.base_url.startswith("https://publicreporting.cftc.gov/")
    assert "@" not in CFG.user_agent, "sin datos personales"


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ("lag_days = 7", "lag_days = 1"),
        ('time = "15:30"', 'time = "tarde"'),
        ('timezone = "America/New_York"', 'timezone = "Nowhere/City"'),
        ('"20974+" = "US100"', '"20974+" = "us100"'),
        ('open_interest_all = "tff_open_interest"', 'open_interest_all = "tff_dealer_long"'),
        ("https://publicreporting", "http://publicreporting"),
    ],
)
def test_invalid_config_is_rejected(old: str, new: str) -> None:
    assert old in REAL
    with pytest.raises(CotError):
        load_config(REAL.replace(old, new, 1))


def test_fetch_pages_through_the_api_with_the_configured_identity() -> None:
    cfg = load_config(REAL.replace("page_size = 1000", "page_size = 2"))
    pages = [[row(), row("13874+")], [row("124603")]]
    seen: list[tuple[dict[str, list[str]], str | None]] = []

    def fake(req: urllib.request.Request, timeout: int) -> bytes:
        q = urllib.parse.parse_qs(urllib.parse.urlsplit(req.full_url).query)
        seen.append((q, req.get_header("User-agent")))
        return json.dumps(pages[len(seen) - 1]).encode()

    rows = list(fetch_rows(cfg, date(2010, 6, 15), opener=fake))
    assert len(rows) == 3
    assert [q["$offset"] for q, _ in seen] == [["0"], ["2"]]
    assert all(ua == cfg.user_agent for _, ua in seen)
    where = seen[0][0]["$where"][0]
    assert "'124603', '13874+', '20974+'" in where and "2010-06-15" in where


def test_value_is_exact() -> None:
    obs = to_observations([row(open_interest_all="314318")], CFG, LATE)
    assert obs[0].value == Decimal("314318")
