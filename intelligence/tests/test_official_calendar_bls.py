"""CPI y empleo del BLS desde páginas guardadas a mano (ADR 0016, #608).

Los fixtures son HTML chicos hechos a mano que imitan la forma de las páginas del BLS
(`https://www.bls.gov/schedule/{AAAA}/home.htm` y los avisos de fechas revisadas). Las fechas de
2013 son las del aviso real de ese cierre. Fuente: U.S. Bureau of Labor Statistics (dominio
público). Ningún test pide bls.gov.
"""

import json
import os
import random
import uuid
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from pathlib import Path
from zoneinfo import ZoneInfo

import jsonschema
import psycopg
import pytest

from cfds_intel.calendar.official import OfficialCalendarError, OfficialRelease, build_bls
from cfds_intel.calendar.official.bls import load_bls, parse_day, read_bls_dir
from cfds_intel.calendar.official.model import BlsConfig, Lapse, load_config
from cfds_intel.calendar.official.pages import MONTH_NAMES
from cfds_intel.calendar.official.store import insert_releases

ROOT = Path(__file__).resolve().parents[2]
REPO_CONFIG = load_config((ROOT / "config" / "official_calendar.toml").read_text())
NOTICE_2013 = "https://www.bls.gov/bls/updated_release_schedule.htm"
NOTICE_2025 = "https://www.bls.gov/bls/2025-lapse-revised-release-dates.htm"
NOW = datetime(2026, 11, 25, tzinfo=UTC)
CPI, EMP = "Consumer Price Index", "Employment Situation"
SCHEMA = json.loads((ROOT / "contracts" / "official_release.v1.schema.json").read_text())


def cfg(first: int, last: int) -> BlsConfig:
    assert REPO_CONFIG.bls is not None
    return replace(REPO_CONFIG.bls, first_year=first, last_year=last)


def z(text: str) -> datetime:
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def long_day(d: date) -> str:
    return d.strftime("%A, %B %d, %Y")


def weekday_on_or_after(d: date, weekday: int | None = None) -> date:
    while d.weekday() >= 5 or (weekday is not None and d.weekday() != weekday):
        d += timedelta(days=1)
    return d


def default_day(name: str, ref: tuple[int, int]) -> date:
    y, m = ref[0] + ref[1] // 12, ref[1] % 12 + 1
    if name == EMP:
        return weekday_on_or_after(date(y, m, 1), 4)  # primer viernes
    return weekday_on_or_after(date(y, m, 15))


def year_html(
    year: int,
    overrides: dict[tuple[str, str], date | None] | None = None,
    time_text: str = "08:30 AM",
) -> str:
    """Página por año: 12 meses con su `h1`, CPI y empleo de cada mes de referencia.
    `overrides[(nombre, "AAAA-MM")]` cambia la fecha; `None` la quita (cancelada)."""
    over = overrides or {}
    rows: dict[int, list[tuple[date, str, str]]] = {m: [] for m in range(1, 13)}
    for name in (CPI, EMP):
        for ref in [(year - 1, 12)] + [(year, m) for m in range(1, 12)]:
            key = (name, f"{ref[0]:04d}-{ref[1]:02d}")
            day = over[key] if key in over else default_day(name, ref)
            if day is None:
                continue
            desc = f"<strong>{name}</strong> for {MONTH_NAMES[ref[1]]} {ref[0]}"
            rows[day.month].append((day, time_text, desc))
    # Otras publicaciones que se ignoran (con nombres parecidos).
    rows[3].append(
        (date(year, 3, 20), "10:00 AM", f"<strong>{EMP} of Veterans</strong> for Annual")
    )
    rows[1].append((date(year, 1, 16), "08:30 AM", "<strong>Real Earnings</strong> for December"))
    body = []
    for m in range(1, 13):
        trs = "".join(
            f'<tr><td class="date-cell"><p>{long_day(d)}</p></td>'
            f'<td class="time-cell"><p>{t}</p></td><td class="desc-cell"><p>{desc}</p></td></tr>'
            for d, t, desc in sorted(rows[m])
        )
        body.append(
            f'<h1 style="float:left;">{MONTH_NAMES[m]}, {year}</h1><table class="release-list">'
            "<thead><tr><th><p>Date</p></th><th><p>Time</p></th><th><p>Release</p></th></tr>"
            f"</thead><tbody>{trs}</tbody></table>"
        )
    return (
        f"<html><head><title>Schedule of Releases for {year}</title>"
        "<script>var x = '<h1>no</h1>';</script></head><body>"
        f'<a href="https://data.bls.gov/cgi-bin/print.pl/schedule/{year}/home.htm">PRINT</a>'
        f"{''.join(body)}</body></html>"
    )


def notice_html(url: str, last_modified: str, entries: list[tuple[str, str, str, str, str]]) -> str:
    trs = "".join(
        "<tr>" + "".join(f"<td>{c}</td>" for c in e) + "</tr>"
        for e in [("Real Earnings", "September 2013", "Wednesday, October 16, 2013",
                   "Wednesday, October 30, 2013", "8:30 a.m."), *entries]
    )  # fmt: skip
    return (
        f'<html><head><title>Revised</title><link rel="canonical" href="{url}"></head><body>'
        "<table><thead><tr><th>Release</th><th>Reference Period</th>"
        "<th>Previously Scheduled Release Date</th><th>Revised Release Date</th>"
        f"<th>Release Time</th></tr></thead><tbody>{trs}</tbody></table>"
        f"<p><strong>Last modified date:</strong> {last_modified}</p></body></html>"
    )


# El cierre de 2013, con las fechas del aviso real.
OVERRIDES_2013: dict[tuple[str, str], date | None] = {
    (EMP, "2013-08"): date(2013, 9, 6),
    (CPI, "2013-08"): date(2013, 9, 17),
    (EMP, "2013-09"): date(2013, 10, 22),
    (CPI, "2013-09"): date(2013, 10, 30),
    (EMP, "2013-10"): date(2013, 11, 8),
    (CPI, "2013-10"): date(2013, 11, 20),
    (EMP, "2013-11"): date(2013, 12, 6),
    (CPI, "2013-11"): date(2013, 12, 17),
}
ENTRIES_2013 = [
    (EMP, "September 2013", "Friday, October 4, 2013", "Tuesday, October 22, 2013", "8:30 a.m."),
    (CPI, "September 2013", "Wednesday, October 16, 2013", "Wednesday, October 30, 2013",
     "8:30 a.m."),
    (EMP, "October 2013", "Friday, November 1, 2013", "Friday, November 8, 2013", "8:30 a.m."),
    (CPI, "October 2013", "Friday, November 15, 2013", "Wednesday, November 20, 2013",
     "8:30 a.m."),
]  # fmt: skip


def notices_ok() -> list[str]:
    """Los dos avisos de la config: el de 2013 de verdad y uno de 2025 sin CPI ni empleo."""
    return [
        notice_html(NOTICE_2013, "December 6, 2013", ENTRIES_2013),
        notice_html(NOTICE_2025, "February 12, 2026", []),
    ]


def rows_of(rows: list[OfficialRelease], series: str, ref: str) -> dict[str, OfficialRelease]:
    found = [r for r in rows if r.series == series and r.reference_period == ref]
    by_status = {r.status: r for r in found}
    assert len(by_status) == len(found)
    return by_status


def build_2012_2013() -> list[OfficialRelease]:
    years = [year_html(2012), year_html(2013, OVERRIDES_2013)]
    return build_bls(cfg(2012, 2013), years, notices_ok(), NOW).releases


def test_the_repository_config_has_the_bls_section() -> None:
    bls = REPO_CONFIG.bls
    assert bls is not None
    assert (bls.first_year, bls.last_year) == (2011, 2026)
    assert bls.release_time.isoformat() == "08:30:00"
    assert bls.tz == ZoneInfo("America/New_York")
    assert {n.url for n in bls.notices} == {NOTICE_2013, NOTICE_2025}
    assert Lapse(date(2013, 10, 1), date(2013, 10, 16)) in bls.lapses
    assert Lapse(date(2025, 10, 1), date(2025, 11, 12)) in bls.lapses


def test_eight_thirty_eastern_follows_daylight_saving() -> None:
    # 2013: horario de verano del 10-3 al 3-11. Viernes 8-3 en EST, viernes 15-3 en EDT.
    over = dict(OVERRIDES_2013)
    over[(EMP, "2013-02")] = date(2013, 3, 8)
    over[(CPI, "2013-02")] = date(2013, 3, 15)
    years = [year_html(2012), year_html(2013, over)]
    rows = build_bls(cfg(2012, 2013), years, notices_ok(), NOW).releases
    assert rows_of(rows, "employment_situation", "2013-02")["scheduled"].scheduled_at == z(
        "2013-03-08T13:30:00Z"
    )
    assert rows_of(rows, "cpi", "2013-02")["scheduled"].scheduled_at == z("2013-03-15T12:30:00Z")
    # Fin del horario de verano: el 8-11-2013 ya es EST.
    emp_oct = rows_of(rows, "employment_situation", "2013-10")["rescheduled"]
    assert emp_oct.scheduled_at == z("2013-11-08T13:30:00Z")
    for r in rows:
        assert r.scheduled_at is None or r.scheduled_at.tzinfo is not None
        assert r.published_at.tzinfo is not None


def test_scheduled_rows_are_known_since_the_previous_report_of_the_series() -> None:
    rows = build_2012_2013()
    feb = rows_of(rows, "cpi", "2013-01")["scheduled"]
    jan = rows_of(rows, "cpi", "2012-12")["scheduled"]
    assert feb.published_at == jan.scheduled_at
    assert feb.source_url == "https://www.bls.gov/schedule/2013/home.htm"
    assert feb.time_rule == "bls_calendar_0830_et"
    assert feb.with_press_conference is None


def test_the_first_release_of_each_series_has_no_previous_report_and_stays_out() -> None:
    result = build_bls(cfg(2012, 2013), [year_html(2012), year_html(2013, OVERRIDES_2013)],
                       notices_ok(), NOW)  # fmt: skip
    assert not rows_of(result.releases, "cpi", "2011-12")
    assert not rows_of(result.releases, "employment_situation", "2011-12")
    assert sum("sin informe anterior" in e for e in result.excluded) == 2


def test_2013_reschedules_keep_the_original_and_mark_the_new_date() -> None:
    rows = build_2012_2013()
    # Empleo de septiembre: la fecha original (4-10) la anunció el informe del 6-9.
    sep = rows_of(rows, "employment_situation", "2013-09")
    assert sep["scheduled"].scheduled_at == z("2013-10-04T12:30:00Z")
    assert sep["scheduled"].published_at == z("2013-09-06T12:30:00Z")
    assert sep["scheduled"].source_url == NOTICE_2013
    # La fecha nueva (22-10) no tiene aviso documentado antes: el aviso es del 6-12-2013.
    assert "rescheduled" not in sep
    # Empleo de octubre: el informe del 22-10 (después del corte) anuncia el 8-11 (ADR 0016).
    oct_ = rows_of(rows, "employment_situation", "2013-10")
    assert set(oct_) == {"rescheduled"}
    assert oct_["rescheduled"].published_at == z("2013-10-22T12:30:00Z")
    # El de noviembre se conoce desde el informe reprogramado del 8-11.
    nov = rows_of(rows, "employment_situation", "2013-11")["scheduled"]
    assert nov.published_at == z("2013-11-08T13:30:00Z")
    cpi_oct = rows_of(rows, "cpi", "2013-10")
    assert set(cpi_oct) == {"rescheduled"}
    assert cpi_oct["rescheduled"].published_at == z("2013-10-30T12:30:00Z")


def test_2025_cancellations_are_rows_known_since_the_notice() -> None:
    over: dict[tuple[str, str], date | None] = {
        (EMP, "2025-08"): date(2025, 9, 5),
        (CPI, "2025-08"): date(2025, 9, 11),
        (EMP, "2025-09"): date(2025, 11, 20),
        (CPI, "2025-09"): date(2025, 10, 24),
        (EMP, "2025-10"): None,
        (CPI, "2025-10"): None,
        (EMP, "2025-11"): date(2025, 12, 16),
        (CPI, "2025-11"): date(2025, 12, 18),
    }
    entries = [
        (EMP, "September 2025", "Friday, October 3, 2025", "Thursday, November 20, 2025",
         "8:30 AM ET"),
        (EMP, "October 2025", "Friday, November 7, 2025",
         "Canceled (see Employment Situation note)", ""),
        (EMP, "November 2025", "Friday, December 5, 2025", "Tuesday, December 16, 2025",
         "8:30 AM ET"),
        (CPI, "October 2025", "Thursday, November 13, 2025", "Canceled (See CPI note)", ""),
        (CPI, "November 2025", "Wednesday, December 10, 2025", "Thursday, December 18, 2025",
         "8:30 AM ET"),
    ]  # fmt: skip
    notices = [
        notice_html(NOTICE_2013, "December 6, 2013", []),
        notice_html(NOTICE_2025, "February 12, 2026", entries),
    ]
    result = build_bls(cfg(2025, 2025), [year_html(2025, over)], notices, NOW)
    assert result.counts[("cpi", 2025)] == 11
    assert result.counts[("employment_situation", 2025)] == 11
    rows = result.releases
    for series in ("cpi", "employment_situation"):
        cancelled = rows_of(rows, series, "2025-10")
        assert set(cancelled) == {"cancelled"}
        assert cancelled["cancelled"].scheduled_at is None
        # Final del 12-2-2026 en ET (EST): 04:59:59Z del 13.
        assert cancelled["cancelled"].published_at == z("2026-02-13T04:59:59Z")
        assert cancelled["cancelled"].time_rule == "bls_notice_last_modified"
    # CPI de septiembre (24-10): un corte cae entre el informe anterior y esta fecha, y ningún
    # aviso la documenta: queda fuera.
    assert not rows_of(rows, "cpi", "2025-09")
    assert any("cpi 2025-09" in e and "corte" in e for e in result.excluded)
    # Empleo de noviembre: el informe del 20-11 salió después del corte y anuncia el 16-12.
    nov = rows_of(rows, "employment_situation", "2025-11")
    assert set(nov) == {"rescheduled"}
    assert nov["rescheduled"].published_at == z("2025-11-20T13:30:00Z")
    # CPI de noviembre: el informe anterior (24-10) es del corte; el aviso es posterior: fuera.
    assert not rows_of(rows, "cpi", "2025-11")


def test_every_row_follows_the_contract_and_never_looks_ahead() -> None:
    for r in build_2012_2013():
        jsonschema.validate(r.to_contract(), SCHEMA)
        assert r.source == "bls"
        assert r.scheduled_at is None or r.published_at <= r.scheduled_at


def test_reloading_gives_exactly_the_same_rows() -> None:
    assert build_2012_2013() == build_2012_2013()


def test_rows_not_yet_known_are_left_out() -> None:
    years = [year_html(2012), year_html(2013, OVERRIDES_2013)]
    result = build_bls(cfg(2012, 2013), years, notices_ok(), datetime(2013, 1, 1, tzinfo=UTC))
    assert all(r.published_at <= datetime(2013, 1, 1, tzinfo=UTC) for r in result.releases)
    assert any("todavía no se conoce" in e for e in result.excluded)


def test_the_year_comes_from_the_content_and_unknown_files_fail(tmp_path: Path) -> None:
    (tmp_path / "whatever name.html").write_text(year_html(2012))
    (tmp_path / "Schedule of Releases for 1999.html").write_text(year_html(2013, OVERRIDES_2013))
    for i, n in enumerate(notices_ok()):
        (tmp_path / f"n{i}.html").write_text(n)
    rows = load_bls(cfg(2012, 2013), tmp_path, NOW).releases
    assert rows == build_2012_2013()
    (tmp_path / "other.html").write_text("<html><title>Something else</title></html>")
    with pytest.raises(OfficialCalendarError, match="no es un calendario ni un aviso"):
        read_bls_dir(tmp_path, cfg(2012, 2013))


@pytest.mark.parametrize(
    ("years", "notices", "message"),
    [
        # Falta un año.
        (lambda: [year_html(2013, OVERRIDES_2013)], notices_ok, "faltan \\[2012\\]"),
        # Año repetido.
        (lambda: [year_html(2012), year_html(2012), year_html(2013, OVERRIDES_2013)],
         notices_ok, "repetido"),
        # Otra hora.
        (lambda: [year_html(2012, time_text="10:00 AM"), year_html(2013, OVERRIDES_2013)],
         notices_ok, "no '08:30 AM'"),
        # Una publicación de menos (sin aviso de cancelación).
        (lambda: [year_html(2012, {(CPI, "2012-05"): None}), year_html(2013, OVERRIDES_2013)],
         notices_ok, "11 publicaciones, se esperaban 12"),
        # El calendario no coincide con la fecha revisada del aviso.
        (lambda: [year_html(2012), year_html(2013, {**OVERRIDES_2013,
                                                     (EMP, "2013-09"): date(2013, 10, 23)})],
         notices_ok, "el aviso no coincide"),
        # Falta un aviso.
        (lambda: [year_html(2012), year_html(2013, OVERRIDES_2013)],
         lambda: notices_ok()[:1], "faltan avisos"),
    ],
)  # fmt: skip
def test_unexpected_structure_fails_loudly(years, notices, message) -> None:  # type: ignore[no-untyped-def]
    with pytest.raises(OfficialCalendarError, match=message):
        build_bls(cfg(2012, 2013), years(), notices(), NOW)


def test_a_row_with_an_unexpected_shape_fails() -> None:
    html = year_html(2012).replace(
        "<strong>Consumer Price Index</strong> for March 2012",
        "<strong>Consumer Price Index</strong> for Mar. 2012 (revised)",
    )
    with pytest.raises(OfficialCalendarError, match="forma inesperada"):
        build_bls(cfg(2012, 2013), [html, year_html(2013, OVERRIDES_2013)], notices_ok(), NOW)


def test_days_need_the_right_weekday() -> None:
    assert parse_day("Friday, March 08, 2013", "t") == date(2013, 3, 8)
    assert parse_day("Friday, March 8, 2013", "t") == date(2013, 3, 8)
    with pytest.raises(OfficialCalendarError, match="día de la semana"):
        parse_day("Monday, March 8, 2013", "t")


def test_bad_bls_config_is_rejected() -> None:
    text = (ROOT / "config" / "official_calendar.toml").read_text()
    for old, new in [
        ('release_time = "08:30"', 'release_time = "8:30"'),
        ('year_url = "https://www.bls.gov/schedule/{year}/home.htm"', 'year_url = "https://x/"'),
        ('start = "2013-10-01"', 'start = "2013-11-01"'),
        ('time_rule = "bls_calendar_0830_et"', 'time_rule = "release_page"'),
    ]:
        assert old in text
        with pytest.raises(OfficialCalendarError):
            load_config(text.replace(old, new, 1))


# En el Mac mini: la carpeta del checkout principal (o CFDS_BLS_RAW_DIR, para una worktree).
REAL_DIR = Path(os.environ.get("CFDS_BLS_RAW_DIR", ROOT / "data" / "raw" / "bls"))


@pytest.mark.skipif(not REAL_DIR.is_dir(), reason="páginas del BLS guardadas solo en el Mac mini")
def test_the_pages_saved_by_the_owner() -> None:
    assert REPO_CONFIG.bls is not None
    result = load_bls(REPO_CONFIG.bls, REAL_DIR, NOW)
    for (_series, year), n in result.counts.items():
        assert n == (11 if year == 2025 else 12)
    rows = result.releases
    oct13 = rows_of(rows, "employment_situation", "2013-10")["rescheduled"]
    assert oct13.published_at == z("2013-10-22T12:30:00Z")
    assert set(rows_of(rows, "cpi", "2025-10")) == {"cancelled"}


@pytest.mark.pg
def test_loading_bls_rows_twice_adds_nothing() -> None:
    conn = psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)
    year = random.randint(2100, 2999)  # noqa: S311
    at = datetime(year, 3, 15, 12, 30, tzinfo=UTC)
    url = f"https://www.bls.gov/test/{uuid.uuid4().hex}.htm"
    rows = [
        OfficialRelease("bls", "cpi", f"{year}-02", "scheduled", at, at - timedelta(days=30),
                        "bls_calendar_0830_et", None, url),
        OfficialRelease("bls", "cpi", f"{year}-02", "rescheduled", at + timedelta(days=5),
                        at - timedelta(days=1), "bls_calendar_0830_et", None, url),
        OfficialRelease("bls", "employment_situation", f"{year}-02", "cancelled", None,
                        at, "bls_notice_last_modified", None, url),
    ]  # fmt: skip
    first = insert_releases(conn, rows)
    second = insert_releases(conn, rows)
    assert (first.inserted, first.unchanged) == (3, 0)
    assert (second.inserted, second.unchanged) == (0, 3)
