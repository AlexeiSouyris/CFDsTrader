"""Calendario histórico oficial del FOMC (ADR 0016, #520), sin red.

La página de 2020 es un extracto real de federalreserve.gov (`fixtures/fed_fomchistorical2020.htm`).
La de calendarios también es un extracto real (`fixtures/fed_fomccalendars.htm`, #523).
Los comunicados y anuncios se arman acá con las líneas que traen las páginas reales ("For
release at 5:00 p.m. EDT", "For immediate release") y sus fechas reales.
"""

import json
import urllib.error
import urllib.request
from dataclasses import replace
from datetime import UTC, date, datetime, time
from email.message import Message
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.calendar.official import OfficialCalendarError, OfficialRelease, load_fed
from cfds_intel.calendar.official.fed import (
    known_since,
    parse_announcement,
    parse_calendars_page,
    parse_historical_page,
    parse_release_page,
)
from cfds_intel.calendar.official.model import Announcement, FedConfig, load_config
from cfds_intel.calendar.official.pages import MONTH_NAMES, PoliteFetcher, parse_days

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).parent / "fixtures"
CONFIG = load_config((ROOT / "config" / "official_calendar.toml").read_text())
SCHEMA = json.loads((ROOT / "contracts" / "official_release.v1.schema.json").read_text())
VALIDATOR = Draft202012Validator(SCHEMA)
SITE = "https://www.federalreserve.gov"
PREFIX = CONFIG.fed.release_url_prefix
PR = f"{SITE}/newsevents/pressreleases/monetary"


def z(text: str) -> datetime:
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def release_html(
    day: date, line: str, title: str = "Federal Reserve issues FOMC statement", body: str = ""
) -> str:
    """Página de un comunicado con la forma de las reales: fecha, línea de hora, título."""
    return (
        f"<html><head><title>Federal Reserve Board - {title}</title></head><body>"
        "<nav><a href='/monetarypolicy.htm'>Monetary Policy</a></nav><div id='article'>"
        f"<p class='article__time'>{MONTH_NAMES[day.month]} {day:%d}, {day.year}</p>"
        f"<p class='releaseTime'>{line}</p><h3 class='title'>{title}</h3>"
        f"<div>{body}</div></div><div id='lastUpdate'>Last Update: {MONTH_NAMES[day.month]}"
        f" {day.day}, {day.year}</div></body></html>"
    )


def announcement_html(day: date, line: str, title: str, entries: list[str]) -> str:
    body = "".join(f"<p>{e}</p>" for e in entries)
    return release_html(day, line, title, body)


def historical_html(year: int, blocks: list[tuple[str, str | None, bool]]) -> str:
    """Página por año: (encabezado, día del comunicado AAAAMMDD o None, conferencia)."""
    parts = [f"<html><body><div id='article'><h3>{year}</h3>"]
    for heading, stmt, presser in blocks:
        parts.append(f"<div class='panel'><h5 class='panel-heading'>{heading}</h5>")
        if stmt:
            parts.append(
                f"<p><a href='/newsevents/pressreleases/monetary{stmt}a.htm'>Statement</a></p>"
            )
        if presser:
            parts.append(
                f"<p><a href='/monetarypolicy/fomcpresconf{stmt}.htm'>Press Conference</a></p>"
            )
        parts.append("<p>Minutes (Released January 03, 2012)</p></div>")
    parts.append("</div></body></html>")
    return "".join(parts)


def getter(pages: dict[str, str]):  # type: ignore[no-untyped-def]
    def get(url: str) -> str:
        assert url in pages, f"pedido sin fixture (en los tests no hay red): {url}"
        return pages[url]

    return get


def fed(first: int, last: int, anns: list[tuple[str, tuple[int, ...]]]) -> FedConfig:
    return replace(
        CONFIG.fed,
        first_year=first,
        last_year=last,
        announcements=tuple(Announcement(f"{PR}{u}.htm", y) for u, y in anns),
    )


# --- 2020 y 2021: reuniones de emergencia, horas de la página y página de calendarios ---------

PAGES_2020_2021: dict[str, str] = {
    f"{SITE}/monetarypolicy/fomchistorical2020.htm": (
        FIXTURES / "fed_fomchistorical2020.htm"
    ).read_text(),
    f"{SITE}/monetarypolicy/fomccalendars.htm": (FIXTURES / "fed_fomccalendars.htm").read_text(),
    f"{PR}20190517a.htm": announcement_html(
        date(2019, 5, 17),
        "For release at 2:00 p.m. EDT",
        "Federal Open Market Committee announces its tentative meeting schedule for 2020",
        [
            "January 28-29 (Tuesday-Wednesday)",
            "March 17-18 (Tuesday-Wednesday)*",
            "April 28-29 (Tuesday-Wednesday)",
            "June 9-10 (Tuesday-Wednesday)*",
            "July 28-29 (Tuesday-Wednesday)",
            "September 15-16 (Tuesday-Wednesday)*",
            "November 4-5 (Wednesday-Thursday)",
            "December 15-16 (Tuesday-Wednesday)*",
            "January 26-27, 2021 (Tuesday-Wednesday)",
        ],
    ),
    f"{PR}20200702a.htm": announcement_html(
        date(2020, 7, 2),
        "For release at 2:00 p.m. EDT",
        "Federal Open Market Committee announces its tentative meeting schedule for 2021",
        [
            "January 26-27 (Tuesday-Wednesday)",
            "March 16-17 (Tuesday-Wednesday)*",
            "April 27-28 (Tuesday-Wednesday)",
            "June 15-16 (Tuesday-Wednesday)*",
            "July 27-28 (Tuesday-Wednesday)",
            "September 21-22 (Tuesday-Wednesday)*",
            "November 2-3 (Tuesday-Wednesday)",
            "December 14-15 (Tuesday-Wednesday)*",
            "January 25-26, 2022 (Tuesday-Wednesday)",
        ],
    ),
}
for _day, _line in {
    "20200129": "For release at 2:00 p.m. EST",
    "20200303": "For release at 10:00 a.m. EST",
    "20200315": "For release at 5:00 p.m. EDT",
    "20200323": "For release at 8:00 a.m. EDT",
    "20200429": "For release at 2:00 p.m. EDT",
    "20200610": "For release at 2:00 p.m. EDT",
    "20200729": "For release at 2:00 p.m. EDT",
    "20200916": "For release at 2:00 p.m. EDT",
    "20201105": "For release at 2:00 p.m. EST",
    "20201216": "For release at 2:00 p.m. EST",
    "20210127": "For release at 2:00 p.m. EST",
    "20210317": "For release at 2:00 p.m. EDT",
    "20210428": "For release at 2:00 p.m. EDT",
    "20210616": "For release at 2:00 p.m. EDT",
    "20210728": "For release at 2:00 p.m. EDT",
    "20210922": "For release at 2:00 p.m. EDT",
    "20211103": "For release at 2:00 p.m. EDT",
    "20211215": "For release at 2:00 p.m. EST",
}.items():
    PAGES_2020_2021[f"{PR}{_day}a.htm"] = release_html(
        datetime.strptime(_day, "%Y%m%d").date(), _line
    )

FED_2020_2021 = fed(2020, 2021, [("20190517a", (2020,)), ("20200702a", (2021,))])


def by_period(rows: list[OfficialRelease]) -> dict[str, OfficialRelease]:
    return {r.reference_period: r for r in rows}


def test_the_repository_config_is_honest_and_covers_every_year() -> None:
    assert "@" not in CONFIG.user_agent and "cfds-intel" in CONFIG.user_agent
    assert CONFIG.pause_secs >= 1
    assert (CONFIG.fed.first_year, CONFIG.fed.last_year) == (2011, 2025)
    assert str(CONFIG.fed.tz) == "America/New_York"
    # Exactamente una regla de hora para cada día y cada caso de conferencia.
    day = date(2011, 1, 1)
    while day.year <= 2025:
        for pc in (True, False):
            CONFIG.fed.rule_for(day, pc)
        day = date.fromordinal(day.toordinal() + 7)
    years = {y for a in CONFIG.fed.announcements for y in a.years}
    assert set(range(2011, 2026)) <= years


def test_the_real_2020_page_has_the_emergency_meetings_and_only_policy_statements() -> None:
    url = f"{SITE}/monetarypolicy/fomchistorical2020.htm"
    ms = parse_historical_page(PAGES_2020_2021[url], url, 2020, PREFIX)
    got = [(str(m.first_day), str(m.last_day), m.kind, m.statement_url is not None) for m in ms]
    assert got == [
        ("2020-01-28", "2020-01-29", "scheduled", True),
        ("2020-03-02", "2020-03-02", "unscheduled", True),
        ("2020-03-15", "2020-03-15", "unscheduled", True),
        ("2020-03-17", "2020-03-18", "cancelled", False),
        ("2020-03-19", "2020-03-19", "unscheduled", False),  # "Press Release", no comunicado
        ("2020-03-23", "2020-03-23", "unscheduled", True),
        ("2020-03-31", "2020-03-31", "unscheduled", False),
        ("2020-04-28", "2020-04-29", "scheduled", True),
        ("2020-06-09", "2020-06-10", "scheduled", True),
        ("2020-07-28", "2020-07-29", "scheduled", True),
        ("2020-08-27", "2020-08-27", "unscheduled", False),  # Statement on Longer-Run Goals
        ("2020-09-15", "2020-09-16", "scheduled", True),
        ("2020-11-04", "2020-11-05", "scheduled", True),
        ("2020-12-15", "2020-12-16", "scheduled", True),
    ]
    assert ms[1].statement_url == f"{PR}20200303a.htm" and ms[1].press_conference


def test_the_calendars_page_reads_split_months_asterisks_and_notation_votes() -> None:
    url = f"{SITE}/monetarypolicy/fomccalendars.htm"
    ms = parse_calendars_page(PAGES_2020_2021[url], url, PREFIX)
    jan_feb = next(m for m in ms if m.year == 2023)
    assert (jan_feb.first_day, jan_feb.last_day) == (date(2023, 1, 31), date(2023, 2, 1))
    assert jan_feb.statement_url == f"{PR}20230201a.htm"  # el "HTML", no el "PDF"
    vote = next(m for m in ms if m.year == 2025 and m.first_day.month == 8)
    assert vote.kind == "unscheduled" and vote.statement_url is None
    assert len([m for m in ms if m.year == 2021 and m.press_conference]) == 8
    # El año es un ancla (`<a id=...>2025 FOMC Meetings</a>`), no texto suelto (#523).
    for year in range(2021, 2026):
        assert len([m for m in ms if m.year == year and m.kind == "scheduled"]) == 8
        assert all(m.statement_url for m in ms if m.year == year and m.kind == "scheduled")


def test_emergency_meetings_use_their_own_time_and_are_known_only_at_release() -> None:
    result = load_fed(FED_2020_2021, getter(PAGES_2020_2021))
    rows = by_period(result.releases)
    assert len(result.releases) == 18 and result.excluded == []
    # 3-3-2020 (antes del cambio de hora del 8-3): 10:00 EST = 15:00 UTC.
    # 15-3-2020 (domingo, después): 5:00 p.m. EDT = 21:00 UTC. 23-3-2020: 8:00 a.m. EDT.
    for period, at in [
        ("2020-03-02", "2020-03-03T15:00:00Z"),
        ("2020-03-15", "2020-03-15T21:00:00Z"),
        ("2020-03-23", "2020-03-23T12:00:00Z"),
    ]:
        r = rows[period]
        assert (r.status, r.scheduled_at, r.published_at) == ("unscheduled", z(at), z(at))
        assert r.time_rule == "release_page"
    assert rows["2020-03-02"].source_url == f"{PR}20200303a.htm"
    assert "2020-03-18" not in rows  # cancelada: no hay comunicado
    assert any("2020-03-17" in s for s in result.skipped)


def test_scheduled_meetings_are_known_since_the_tentative_calendar() -> None:
    rows = by_period(load_fed(FED_2020_2021, getter(PAGES_2020_2021)).releases)
    jan = rows["2020-01-29"]
    assert jan.scheduled_at == z("2020-01-29T19:00:00Z")  # 2:00 p.m. EST
    assert jan.published_at == z("2019-05-17T18:00:00Z")  # anuncio, 2:00 p.m. EDT
    assert rows["2020-06-10"].scheduled_at == z("2020-06-10T18:00:00Z")  # EDT
    # Enero de 2021: ya figuraba (con su año) en el anuncio de 2019.
    assert rows["2021-01-27"].published_at == z("2019-05-17T18:00:00Z")
    assert rows["2021-03-17"].published_at == z("2020-07-02T18:00:00Z")
    assert all(r.with_press_conference for r in rows.values() if r.status == "scheduled")
    assert rows["2020-03-23"].with_press_conference is False  # voto por escrito


def test_every_row_follows_the_contract_and_never_looks_ahead() -> None:
    for r in load_fed(FED_2020_2021, getter(PAGES_2020_2021)).releases:
        doc = r.to_contract()
        assert list(VALIDATOR.iter_errors(doc)) == [], doc
        assert r.scheduled_at is not None and r.published_at <= r.scheduled_at


# --- 2011 a 2013: las épocas de la regla de hora ----------------------------------------------

PAGES_2011: dict[str, str] = {
    f"{SITE}/monetarypolicy/fomchistorical2011.htm": historical_html(
        2011,
        [
            ("January 25-26 Meeting - 2011", "20110126", False),
            ("March 15 Meeting - 2011", "20110315", False),
            ("April 26-27 Meeting - 2011", "20110427", True),
            ("June 21-22 Meeting - 2011", "20110622", True),
            ("August 1 Conference Call - 2011", None, False),
            ("August 9 Meeting - 2011", "20110809", False),
            ("September 20-21 Meeting - 2011", "20110921", False),
            ("November 1-2 Meeting - 2011", "20111102", True),
            ("November 28 Conference Call - 2011", None, False),
            ("December 13 Meeting - 2011", "20111213", False),
        ],
    ),
    f"{PR}20100528b.htm": announcement_html(
        date(2010, 5, 28),
        "For immediate release",
        "FOMC announces tentative 2011 meeting schedule",
        [
            "January 25-26 (Tuesday-Wednesday)",
            "March 15 (Tuesday)",
            "April 26-27 (Tuesday-Wednesday)",
            "June 21-22 (Tuesday-Wednesday)",
            "August 9 (Tuesday)",
            "September 20 (Tuesday)",
            "November 1-2 (Tuesday-Wednesday)",
            "December 13 (Tuesday)",
            "January 24-25, 2012 (Tuesday-Wednesday)",
        ],
    ),
}
for _day in ["20110126", "20110315", "20110427", "20110622", "20110809", "20110921"]:
    PAGES_2011[f"{PR}{_day}a.htm"] = release_html(
        datetime.strptime(_day, "%Y%m%d").date(), "For immediate release"
    )
for _day in ["20111102", "20111213"]:
    PAGES_2011[f"{PR}{_day}a.htm"] = release_html(
        datetime.strptime(_day, "%Y%m%d").date(), "For immediate release"
    )

FED_2011 = fed(2011, 2011, [("20100528b", (2011,))])


def test_2011_times_follow_the_rule_and_daylight_saving() -> None:
    result = load_fed(FED_2011, getter(PAGES_2011))
    rows = by_period(result.releases)
    expected = {
        # 2:15 p.m. EST antes del 13-3-2011 y EDT después (la misma hora local, otro UTC).
        "2011-01-26": ("2011-01-26T19:15:00Z", "fomc_1415_until_2011_03"),
        "2011-03-15": ("2011-03-15T18:15:00Z", "fomc_1415_until_2011_03"),
        # Desde abril de 2011: ~12:30 con conferencia, 2:15 sin.
        "2011-04-27": ("2011-04-27T16:30:00Z", "fomc_1230_press_conference_2011_2013"),
        "2011-06-22": ("2011-06-22T16:30:00Z", "fomc_1230_press_conference_2011_2013"),
        "2011-08-09": ("2011-08-09T18:15:00Z", "fomc_1415_no_press_conference_2011_2013"),
        # 2-11-2011 todavía en EDT (el horario de verano terminó el 6-11).
        "2011-11-02": ("2011-11-02T16:30:00Z", "fomc_1230_press_conference_2011_2013"),
        "2011-12-13": ("2011-12-13T19:15:00Z", "fomc_1415_no_press_conference_2011_2013"),
    }
    assert {p: (r.scheduled_at, r.time_rule) for p, r in rows.items()} == {
        p: (z(at), rule) for p, (at, rule) in expected.items()
    }
    # El anuncio dice "For immediate release": vale desde el final de ese día (23:59:59 EDT).
    assert {r.published_at for r in rows.values()} == {z("2010-05-29T03:59:59Z")}
    # Septiembre: el calendario decía "September 20" y la reunión fue el 20-21. No hay anuncio
    # documentado del cambio: se marca y queda fuera (ADR 0016 §6).
    assert len(result.excluded) == 1 and "2011-09-20" in result.excluded[0]
    assert len([s for s in result.skipped if "unscheduled" in s]) == 2  # las conferencias


PAGES_2013: dict[str, str] = {
    f"{SITE}/monetarypolicy/fomchistorical2013.htm": historical_html(
        2013,
        [
            ("January 29-30 Meeting - 2013", "20130130", False),
            ("March 19-20 Meeting - 2013", "20130320", True),
            ("April/May 30-1 Meeting - 2013", "20130501", False),
            ("June 18-19 Meeting - 2013", "20130619", True),
            ("July 30-31 Meeting - 2013", "20130731", False),
            ("September 17-18 Meeting - 2013", "20130918", True),
            ("October 16 (unscheduled) - 2013", None, False),
            ("October 29-30 Meeting - 2013", "20131030", False),
            ("December 17-18 Meeting - 2013", "20131218", True),
        ],
    ),
    # Dos calendarios en un comunicado ("for 2012:" y "for 2013:") más enero de 2014.
    f"{PR}20120516b.htm": announcement_html(
        date(2012, 5, 16),
        "For immediate release",
        "FOMC announces revised tentative 2012 meeting schedule and tentative 2013 meeting"
        " schedule",
        [
            "Revised tentative meeting schedule for 2012:",
            "June 19-20 (Tuesday-Wednesday)*",
            "July 31-August 1 (Tuesday-Wednesday)",
            "December 11-12 (Tuesday-Wednesday)*",
            "Tentative meeting schedule for 2013:",
            "January 29-30 (Tuesday-Wednesday)",
            "March 19-20 (Tuesday-Wednesday)*",
            "April 30-May 1 (Tuesday-Wednesday)",
            "June 18-19 (Tuesday-Wednesday)*",
            "July 30-31 (Tuesday-Wednesday)",
            "September 17-18 (Tuesday-Wednesday)*",
            "October 29-30 (Tuesday-Wednesday)",
            "December 17-18 (Tuesday-Wednesday)*",
            "January 28-29, 2014 (Tuesday-Wednesday)",
        ],
    ),
}
for _day in ["20130130", "20130320", "20130501", "20130619", "20130731", "20130918"]:
    PAGES_2013[f"{PR}{_day}a.htm"] = release_html(
        datetime.strptime(_day, "%Y%m%d").date(), "For immediate release"
    )
for _day in ["20131030", "20131218"]:
    PAGES_2013[f"{PR}{_day}a.htm"] = release_html(
        datetime.strptime(_day, "%Y%m%d").date(), "For immediate release"
    )

FED_2013 = fed(2013, 2013, [("20120516b", (2012, 2013))])


def test_2013_switches_to_two_pm_from_the_march_meeting() -> None:
    result = load_fed(FED_2013, getter(PAGES_2013))
    rows = by_period(result.releases)
    assert result.excluded == [] and len(rows) == 8
    # Enero de 2013: todavía 2:15 p.m. EST (sin conferencia).
    assert rows["2013-01-30"].scheduled_at == z("2013-01-30T19:15:00Z")
    # Desde el 20-3-2013: 2:00 p.m., ya en EDT.
    assert rows["2013-03-20"].scheduled_at == z("2013-03-20T18:00:00Z")
    assert rows["2013-03-20"].time_rule == "fomc_1400_from_2013_03"
    assert rows["2013-05-01"].scheduled_at == z("2013-05-01T18:00:00Z")
    assert rows["2013-12-18"].scheduled_at == z("2013-12-18T19:00:00Z")  # EST
    assert {r.published_at for r in rows.values()} == {z("2012-05-17T03:59:59Z")}
    assert any("2013-10-16" in s for s in result.skipped)


def test_a_january_meeting_is_known_since_the_first_calendar_that_lists_it_with_its_year() -> None:
    old = parse_announcement(
        PAGES_2013[f"{PR}20120516b.htm"],
        Announcement(f"{PR}20120516b.htm", (2012, 2013)),
        CONFIG.fed.tz,
    )
    new_url = f"{PR}20130405a.htm"
    new = parse_announcement(
        announcement_html(
            date(2013, 4, 5),
            "For immediate release",
            "FOMC announces tentative 2014 meeting schedule",
            ["January 28-29 (Tuesday-Wednesday)", "March 18-19 (Tuesday-Wednesday)*"],
        ),
        Announcement(new_url, (2014,)),
        CONFIG.fed.tz,
    )
    at = z("2014-01-29T19:00:00Z")
    assert known_since([old, new], date(2014, 1, 28), date(2014, 1, 29), at) is old
    # Marzo de 2014 no está en el de 2012, que sí habla de 2014: corta ahí, vale el de 2013.
    march = z("2014-03-19T18:00:00Z")
    assert known_since([old, new], date(2014, 3, 18), date(2014, 3, 19), march) is new
    # Si el último anuncio lo contradice (otra fecha), no se sabe desde cuándo: None.
    assert known_since([old, new], date(2014, 3, 25), date(2014, 3, 26), march) is None
    # La sección de 2012 no se confunde con la de 2013.
    assert not old.lists(date(2013, 6, 19), date(2013, 6, 20))
    assert old.lists(date(2012, 6, 19), date(2012, 6, 20))


def test_the_long_form_of_2024_announcements_is_recognized() -> None:
    page = announcement_html(
        date(2024, 8, 9),
        "For release at 1:30 p.m. EDT",
        "Federal Open Market Committee announces its tentative meeting schedule for 2025 and 2026",
        [
            "For 2025:",
            "Tuesday, January 28, and Wednesday, January 29",
            "Tuesday, March 18, and Wednesday, March 19",
            "For 2026:",
            "Tuesday, January 27, and Wednesday, January 28",
            "Tuesday, January 26, and Wednesday, January 27, 2027",
        ],
    )
    a = parse_announcement(page, Announcement(f"{PR}20240809a.htm", (2025, 2026)), CONFIG.fed.tz)
    assert a.published_at == z("2024-08-09T17:30:00Z")
    assert a.lists(date(2025, 1, 28), date(2025, 1, 29))
    assert a.lists(date(2026, 1, 27), date(2026, 1, 28))
    assert not a.lists(date(2025, 1, 27), date(2025, 1, 28))  # es la de 2026
    assert a.lists(date(2027, 1, 26), date(2027, 1, 27))
    # Un día suelto no se confunde con el primer día de una reunión de dos.
    assert not a.lists(date(2025, 1, 28), date(2025, 1, 28))


# --- Lo que tiene que fallar ------------------------------------------------------------------


def test_release_pages_need_their_date_and_a_release_line() -> None:
    url = f"{PR}20200315a.htm"
    page = parse_release_page(release_html(date(2020, 3, 15), "For release at 5:00 p.m. EDT"), url)
    assert page.at == time(17, 0)
    assert (
        parse_release_page(release_html(date(2020, 3, 15), "For immediate release"), url).at is None
    )
    with pytest.raises(OfficialCalendarError, match="no trae su fecha"):
        parse_release_page(release_html(date(2020, 3, 16), "For immediate release"), url)
    with pytest.raises(OfficialCalendarError, match="For release at"):
        parse_release_page(release_html(date(2020, 3, 15), "Embargoed"), url)


def test_an_unscheduled_statement_without_a_time_fails_instead_of_inventing_one() -> None:
    pages = dict(PAGES_2020_2021)
    pages[f"{PR}20200315a.htm"] = release_html(date(2020, 3, 15), "For immediate release")
    with pytest.raises(OfficialCalendarError, match="no se inventa una hora"):
        load_fed(FED_2020_2021, getter(pages))


def test_a_page_time_that_contradicts_the_rule_fails() -> None:
    pages = dict(PAGES_2020_2021)
    pages[f"{PR}20200429a.htm"] = release_html(date(2020, 4, 29), "For release at 2:15 p.m. EDT")
    with pytest.raises(OfficialCalendarError, match="no se elige en silencio"):
        load_fed(FED_2020_2021, getter(pages))


def test_a_year_with_the_wrong_number_of_meetings_fails() -> None:
    pages = dict(PAGES_2011)
    url = f"{SITE}/monetarypolicy/fomchistorical2011.htm"
    pages[url] = pages[url].replace("December 13 Meeting - 2011", "December 13 Notes")
    with pytest.raises(OfficialCalendarError, match="2011: 7 reuniones programadas"):
        load_fed(FED_2011, getter(pages))


def test_an_announcement_for_another_year_fails() -> None:
    cfg = fed(2011, 2011, [("20100528b", (2012,))])
    with pytest.raises(OfficialCalendarError, match="no nombra 2012"):
        load_fed(cfg, getter(PAGES_2011))


def test_days_are_parsed_in_every_form_the_pages_use() -> None:
    assert parse_days("July 31-August 1", 2012) == (date(2012, 7, 31), date(2012, 8, 1))
    assert parse_days("Apr/May 30-1", 2024) == (date(2024, 4, 30), date(2024, 5, 1))
    assert parse_days("March 15", 2011) == (date(2011, 3, 15), date(2011, 3, 15))
    assert parse_days("Released March 3", 2020) is None
    assert parse_days("February 30", 2020) is None


def test_the_fetcher_pauses_sends_the_honest_agent_and_stops_on_a_block() -> None:
    sent: list[urllib.request.Request] = []
    waits: list[float] = []
    clock = iter([0.0, 0.0, 0.2, 0.2, 5.0, 5.0])

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        sent.append(req)
        if req.full_url.endswith("blocked.htm"):
            raise urllib.error.HTTPError(req.full_url, 403, "Forbidden", Message(), None)
        return b"<html></html>"

    f = PoliteFetcher(CONFIG.user_agent, 30, 1.5, (PR,), opener, waits.append, lambda: next(clock))
    f(f"{PR}20200315a.htm")
    f(f"{PR}20200315a.htm")  # ya leída: no se pide otra vez
    f(f"{PR}20200323a.htm")
    assert waits == [pytest.approx(1.5)]
    assert sent[0].get_header("User-agent") == CONFIG.user_agent
    with pytest.raises(OfficialCalendarError, match="403"):
        f(f"{PR}blocked.htm")
    with pytest.raises(OfficialCalendarError, match="fuera de las documentadas"):
        f("https://www.bls.gov/schedule/2013/home.htm")
    assert f.requests == 3


@pytest.mark.parametrize(
    "over",
    [
        {"status": "cancelled"},  # cancelada con hora
        {"published_at": datetime(2020, 3, 16, tzinfo=UTC)},  # se supo después
        {"status": "unscheduled", "published_at": datetime(2020, 3, 1, tzinfo=UTC)},
        {"with_press_conference": None},  # el FOMC siempre dice si hubo conferencia
        {"reference_period": "2020-03"},  # el FOMC usa el día
        {"series": "cpi"},
        {"source_url": "http://www.federalreserve.gov/x.htm"},
    ],
)
def test_rows_that_break_the_contract_are_rejected(over: dict[str, object]) -> None:
    ok = OfficialRelease(
        source="fed_fomc",
        series="fomc_statement",
        reference_period="2020-03-15",
        status="scheduled",
        scheduled_at=datetime(2020, 3, 15, 21, tzinfo=UTC),
        published_at=datetime(2019, 5, 17, 18, tzinfo=UTC),
        time_rule="release_page",
        with_press_conference=True,
        source_url=f"{PR}20200315a.htm",
    )
    assert list(VALIDATOR.iter_errors(ok.to_contract())) == []
    with pytest.raises(OfficialCalendarError):
        replace(ok, **over)  # type: ignore[arg-type]


@pytest.mark.parametrize(
    "over",
    [
        {"status": "cancelled"},
        {"scheduled_at": None},
        {"with_press_conference": None},
        {"reference_period": "2020-03"},
        {"series": "cpi"},
        {"source": "bls"},
        {"time_rule": "Release Page"},
        {"extra": 1},
    ],
)
def test_the_json_contract_rejects_the_same(over: dict[str, object]) -> None:
    ok = {
        "source": "fed_fomc",
        "series": "fomc_statement",
        "reference_period": "2020-03-15",
        "status": "scheduled",
        "scheduled_at": "2020-03-15T21:00:00Z",
        "published_at": "2019-05-17T18:00:00Z",
        "time_rule": "release_page",
        "with_press_conference": True,
        "source_url": f"{PR}20200315a.htm",
    }
    assert list(VALIDATOR.iter_errors(ok)) == []
    assert list(VALIDATOR.iter_errors(ok | over)) != []
    bls = ok | {
        "source": "bls",
        "series": "cpi",
        "reference_period": "2013-09",
        "with_press_conference": None,
        "source_url": "https://www.bls.gov/news.release/archives/cpi_10302013.htm",
    }
    assert list(VALIDATOR.iter_errors(bls)) == []  # la forma del BLS ya entra


def test_bad_config_is_rejected() -> None:
    text = (ROOT / "config" / "official_calendar.toml").read_text()
    for old, new in [
        ("pause_secs = 1.5", "pause_secs = 0.2"),
        ('timezone = "America/New_York"', 'timezone = "Mars/Olympus"'),
        ('time = "14:00"', 'time = "2pm"'),
        (f"{PR}20100528b.htm", "https://example.com/evil.htm"),
        ('name = "fomc_1400_from_2013_03"', 'name = "release_page"'),
    ]:
        assert old in text
        with pytest.raises(OfficialCalendarError):
            load_config(text.replace(old, new, 1))


def test_an_unscheduled_statement_may_come_days_later_but_not_weeks() -> None:
    # Como la conferencia del 4-10-2019, cuyo comunicado salió el 11-10 (#523).
    url = f"{SITE}/monetarypolicy/fomchistorical2020.htm"
    pages = dict(PAGES_2020_2021)
    pages[url] = pages[url].replace("March 2 (unscheduled)", "February 25 (unscheduled)")
    rows = by_period(load_fed(FED_2020_2021, getter(pages)).releases)
    assert rows["2020-02-25"].scheduled_at == z("2020-03-03T15:00:00Z")
    pages[url] = pages[url].replace("February 25 (unscheduled)", "February 10 (unscheduled)")
    with pytest.raises(OfficialCalendarError, match="fuera de fecha"):
        load_fed(FED_2020_2021, getter(pages))
