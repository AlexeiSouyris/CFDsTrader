"""Dato real del BLS por su API pública v1 (ADR 0018 §2, #697), con una respuesta real grabada
(25-11-2026, recortada; dominio público, fuente: U.S. Bureau of Labor Statistics) y sin red."""

import json
import urllib.request
from dataclasses import replace
from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path
from typing import cast

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.calendar.actuals import (
    ActualsError,
    Release,
    ReleaseRow,
    build_actuals,
    due_releases,
    effective_releases,
    load_config,
    parse_response,
    query,
    request_body,
    vintage,
)

ROOT = Path(__file__).resolve().parents[2]
CONFIG_TEXT = (ROOT / "config" / "calendar_surprise.toml").read_text()
CFG = load_config(CONFIG_TEXT)
FIXTURE = (Path(__file__).parent / "fixtures" / "bls_api_v1_cpi_empleo_2026-10.json").read_bytes()


def utc(y: int, mo: int, d: int, h: int = 0, mi: int = 0, s: int = 0) -> datetime:
    return datetime(y, mo, d, h, mi, s, tzinfo=UTC)


CPI_09 = Release("cpi", "2026-09", utc(2026, 10, 14, 12, 30), utc(2026, 11, 10, 13, 30))
CPI_10 = Release("cpi", "2026-10", utc(2026, 11, 10, 13, 30), utc(2026, 12, 10, 13, 30))


def test_the_repository_config_is_honest_and_complete() -> None:
    assert CFG.api_url == "https://api.bls.gov/publicAPI/v1/timeseries/data/"
    assert "@" not in CFG.user_agent and "Mozilla" not in CFG.user_agent  # sin datos personales
    assert [s.series_id for s in CFG.series_of("cpi")] == ["CUSR0000SA0", "CUSR0000SA0L1E"]
    assert [s.series_id for s in CFG.series_of("employment_situation")] == [
        "CES0000000001",
        "LNS14000000",
    ]


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('user_agent = "cfds-intel/0.1', 'user_agent = "bot (yo@example.com)'),
        ("publicAPI/v1/", "publicAPI/v2/"),
        ('release = "cpi"', 'release = "gdp"'),
        ('unit = "thousands"', 'unit = "people"'),
        ('transform = "change"', 'transform = "ratio"'),
        ('series_id = "LNS14000000"\ntransform', 'series_id = "XYZ"\ntransform'),
        ("lookback_days = 10", "lookback_days = 0"),
        ("\n[bls]\n", "\n[otra]\n[bls]\n"),
    ],
)
def test_bad_config_is_rejected(old: str, new: str) -> None:
    assert old in CONFIG_TEXT
    with pytest.raises(ActualsError):
        load_config(CONFIG_TEXT.replace(old, new, 1))


def row(period: str, status: str, at: datetime | None, pub: datetime) -> ReleaseRow:
    return ReleaseRow("cpi", period, status, at, pub)


def test_effective_releases_never_look_ahead_and_follow_reschedules() -> None:
    rows = [
        row("2026-01", "scheduled", utc(2026, 2, 11, 13, 30), utc(2026, 1, 13, 13, 30)),
        row("2026-01", "rescheduled", utc(2026, 2, 13, 13, 30), utc(2026, 2, 13, 4, 59, 59)),
        row("2026-02", "scheduled", utc(2026, 3, 11, 12, 30), utc(2026, 2, 13, 13, 30)),
        row("2025-10", "cancelled", None, utc(2025, 11, 1)),
    ]
    # Antes del aviso: vale la fecha original.
    before = effective_releases(rows, utc(2026, 2, 12))
    assert [(r.reference_period, r.released_at) for r in before] == [
        ("2026-01", utc(2026, 2, 11, 13, 30))
    ]
    assert before[0].next_released_at is None  # la de febrero todavía no se conocía
    after = effective_releases(rows, utc(2026, 3, 1))
    assert [(r.reference_period, r.released_at, r.next_released_at) for r in after] == [
        ("2026-01", utc(2026, 2, 13, 13, 30), utc(2026, 3, 11, 12, 30)),
        ("2026-02", utc(2026, 3, 11, 12, 30), None),
    ]  # la cancelada no existe


def test_due_the_business_day_after_in_new_york_time() -> None:
    # Viernes 6-11-2026 08:30 ET (13:30Z): toca el lunes 9, no el sábado ni el domingo.
    nfp = Release("employment_situation", "2026-10", utc(2026, 11, 6, 13, 30), None)
    assert due_releases([nfp], utc(2026, 11, 6, 20), 10, set()) == []
    assert due_releases([nfp], utc(2026, 11, 8, 20), 10, set()) == []
    assert due_releases([nfp], utc(2026, 11, 9, 15), 10, set()) == [nfp]
    # Jueves 5-11 08:30 ET; el viernes 6 a las 03:00Z todavía es jueves en Nueva York.
    thu = replace(nfp, released_at=utc(2026, 11, 5, 13, 30))
    assert due_releases([thu], utc(2026, 11, 6, 3), 10, set()) == []
    assert due_releases([thu], utc(2026, 11, 6, 5), 10, set()) == [thu]
    # Ya con su primer dato, o demasiado vieja (no se rellena el pasado): no se consulta.
    assert due_releases([nfp], utc(2026, 11, 9, 15), 10, {(nfp.series, "2026-10")}) == []
    assert due_releases([nfp], utc(2026, 11, 20), 10, set()) == []


def test_dst_the_0830_et_release_is_1330z_in_winter_and_1230z_in_summer() -> None:
    # 8-3-2026: EE. UU. pasa a horario de verano. Los dos son 08:30 ET y el día hábil siguiente
    # se cuenta en hora del Este.
    winter = Release("cpi", "2026-01", utc(2026, 3, 6, 13, 30), None)  # viernes
    summer = Release("cpi", "2026-02", utc(2026, 3, 11, 12, 30), None)  # miércoles
    assert due_releases([winter, summer], utc(2026, 3, 9, 3, 30), 10, set()) == []  # dom 23:30 EDT
    assert due_releases([winter, summer], utc(2026, 3, 9, 4), 10, set()) == [winter]  # lun 00:00
    assert due_releases([summer], utc(2026, 3, 12, 3, 59), 10, set()) == []  # mié 23:59 EDT
    assert due_releases([summer], utc(2026, 3, 12, 4), 10, set()) == [summer]


def test_the_recorded_response_is_read_exactly_and_missing_values_are_skipped() -> None:
    v = parse_response(FIXTURE)
    assert v[("CUSR0000SA0", "2026-10")] == Decimal("337.476")
    assert v[("CUSR0000SA0L1E", "2026-10")] == Decimal("339.900")  # conserva la escala
    assert v[("CES0000000001", "2026-10")] == Decimal("159293")
    assert v[("LNS14000000", "2026-10")] == Decimal("4.1")
    assert ("CUSR0000SA0", "2025-10") not in v  # "-": el BLS no lo tiene (corte de 2025)
    assert ("CES0000000001", "2025-10") in v


@pytest.mark.parametrize(
    "body",
    [
        b"no json",
        b'{"status": "REQUEST_NOT_PROCESSED", "message": ["daily threshold"]}',
        b'{"status": "REQUEST_SUCCEEDED", "Results": {}}',
        json.dumps(
            {
                "status": "REQUEST_SUCCEEDED",
                "Results": {
                    "series": [
                        {
                            "seriesID": "X",
                            "data": [{"year": "2026", "period": "M10", "value": "1,2"}],
                        }
                    ]
                },
            }
        ).encode(),
    ],
)
def test_bad_responses_are_errors(body: bytes) -> None:
    with pytest.raises(ActualsError):
        parse_response(body)


def test_vintage_is_first_only_before_the_next_release() -> None:
    assert vintage(CPI_10, utc(2026, 11, 11, 15), 20) == "first"
    assert vintage(CPI_10, utc(2026, 12, 10, 13, 29), 20) == "first"
    assert vintage(CPI_10, utc(2026, 12, 10, 13, 30), 20) == "revised"  # ya salió la siguiente
    unknown = replace(CPI_10, next_released_at=None)
    assert vintage(unknown, utc(2026, 11, 30, 13, 30), 20) == "first"
    assert vintage(unknown, utc(2026, 11, 30, 13, 31), 20) == "revised"
    with pytest.raises(ActualsError):
        vintage(CPI_10, utc(2026, 11, 10, 13, 29), 20)  # antes de publicarse: imposible


def test_rows_follow_the_contract_with_official_release_time() -> None:
    schema = Draft202012Validator(
        json.loads((ROOT / "contracts" / "release_actual.v1.schema.json").read_text())
    )
    rows = build_actuals(CPI_10, CPI_09, CFG, parse_response(FIXTURE), utc(2026, 11, 11, 15, 0, 1))
    docs = [r.to_contract() for r in rows]
    for d in docs:
        assert not list(schema.iter_errors(d)), d
    assert [(d["series_id"], d["reference_period"], d["vintage"], d["value"]) for d in docs] == [
        ("CUSR0000SA0", "2026-10", "first", "337.476"),
        ("CUSR0000SA0", "2026-09", "revised", "336.298"),
        ("CUSR0000SA0L1E", "2026-10", "first", "339.900"),
        ("CUSR0000SA0L1E", "2026-09", "revised", "338.896"),
    ]
    assert docs[0]["released_at"] == "2026-11-10T13:30:00Z"  # la hora oficial, no la lectura
    assert docs[1]["released_at"] == "2026-10-14T12:30:00Z"  # la del mes anterior
    assert {d["retrieved_at"] for d in docs} == {"2026-11-11T15:00:01Z"}
    assert all(d["unit"] == "level" for d in docs)


def test_without_the_previous_release_only_the_month_is_stored() -> None:
    rows = build_actuals(CPI_10, None, CFG, parse_response(FIXTURE), utc(2026, 11, 11, 15))
    assert {r.reference_period for r in rows} == {"2026-10"}


def test_january_asks_from_the_previous_year() -> None:
    assert json.loads(request_body(["A"], "2027-01")) == {
        "seriesid": ["A"],
        "startyear": "2026",
        "endyear": "2027",
    }


def test_one_post_with_all_series_and_the_honest_user_agent() -> None:
    seen: list[urllib.request.Request] = []

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        seen.append(req)
        return FIXTURE

    nfp = Release("employment_situation", "2026-10", utc(2026, 11, 6, 13, 30), None)
    values = query(CFG, nfp, opener)
    (req,) = seen
    assert req.get_method() == "POST" and req.full_url == CFG.api_url
    assert req.get_header("User-agent") == CFG.user_agent
    assert json.loads(cast(bytes, req.data))["seriesid"] == ["CES0000000001", "LNS14000000"]
    assert values[("LNS14000000", "2026-10")] == Decimal("4.1")
