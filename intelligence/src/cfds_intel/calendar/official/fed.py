"""Comunicados del FOMC 2011-2025 desde federalreserve.gov (ADR 0016, #520).

De dónde sale cada dato:
- **Reuniones:** la página de cada año (`fomchistorical{AAAA}.htm`, hasta 2020) y la de
  calendarios vigente (`fomccalendars.htm`, desde 2021). Cada reunión trae su tipo (programada,
  `unscheduled`, `notation vote`, `Conference Call`, `cancelled`), su link "Statement" y si tuvo
  "Press Conference". Solo entran las reuniones con un link "Statement" (ADR 0016 §2): un "Press
  Release" o el "Statement on Longer-Run Goals" no son el comunicado de política.
- **Hora (`scheduled_at`):** la de la página del comunicado ("For release at 2:00 p.m. EST")
  si la trae; si no, y la reunión era programada, la regla de `config/official_calendar.toml`
  (2:15, ~12:30 o 2:00 p.m.). Si las dos están y no coinciden, falla. Una no programada sin hora
  en su página falla: **nunca se inventa una hora**. La hora del Este pasa a UTC con
  `America/New_York` (horario de verano incluido), nunca con un desfase fijo.
- **Desde cuándo se sabía (`published_at`, ADR 0016 §4):** una programada, desde el comunicado
  del calendario tentativo (o su revisión) que la anunció con sus fechas exactas, a su hora o,
  si no la trae, al final de ese día (23:59:59 ET). Si el último anuncio anterior a la reunión
  no la trae con esas fechas, la reunión se **marca y queda fuera** (no se supone que se
  conocía). Una no programada, desde su propio comunicado.
"""

import re
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, date, datetime, time
from urllib.parse import urljoin
from zoneinfo import ZoneInfo

from cfds_intel.calendar.official.model import (
    RELEASE_PAGE,
    Announcement,
    FedConfig,
    OfficialCalendarError,
    OfficialRelease,
)
from cfds_intel.calendar.official.pages import (
    MONTH_NAMES,
    MONTHS,
    Link,
    Text,
    parse_days,
    read_page,
)

SOURCE = "fed_fomc"
SERIES = "fomc_statement"
SITE = "https://www.federalreserve.gov/"
STATEMENT = "Statement"
PRESS_CONFERENCE = "Press Conference"
_RELEASE_URL = re.compile(r"monetary(?P<day>\d{8})[a-z]\.htm$")
_HEADING = re.compile(
    r"^(?P<days>.+?)(?: \((?P<tag>[^)]+)\))?"
    r"(?: (?P<kind>Meeting|Conference Call))?-(?P<year>\d{4})$"
)
_YEAR_HEADING = re.compile(r"^(?P<year>\d{4}) FOMC Meetings$")
_MONTH = re.compile(r"^[A-Z][a-z]{2,8}(?:/[A-Z][a-z]{2,8})?$")
_DAYS = re.compile(r"^(?P<days>\d{1,2}(?:-\d{1,2})?)\*?(?: ?\((?P<tag>[^)]+)\))?$")
_STATEMENT_LABEL = re.compile(r"^Statement ?:$")
_TAG = re.compile(r"^\((?P<tag>[^)]+)\)$")
_FOR_RELEASE = re.compile(
    r"For release at (?P<h>\d{1,2}):(?P<m>\d{2}) ?(?P<ap>[ap])\.? ?m\.?", re.IGNORECASE
)
_IMMEDIATE = re.compile(r"For immediate release", re.IGNORECASE)
_SECTION = re.compile(r"\bfor (?P<year>\d{4}):", re.IGNORECASE)
_TAGS = {
    None: "scheduled",
    "unscheduled": "unscheduled",
    "notation vote": "unscheduled",
    "cancelled": "cancelled",
    "canceled": "cancelled",
}


@dataclass
class Meeting:
    """Una reunión tal como la lista la página de su año."""

    year: int
    first_day: date
    last_day: date
    kind: str  # scheduled | unscheduled | cancelled
    page_url: str
    statement_url: str | None = None
    press_conference: bool = False

    def label(self) -> str:
        return f"reunión {self.first_day}…{self.last_day} ({self.kind}, {self.page_url})"


def _kind(tag: str | None, conference_call: bool, where: str) -> str:
    key = None if tag is None else tag.strip().lower()
    if key not in _TAGS:
        raise OfficialCalendarError(f"{where}: tipo de reunión desconocido {tag!r}")
    kind = _TAGS[key]
    return "unscheduled" if conference_call and kind == "scheduled" else kind


def _statement_url(href: str, prefix: str, where: str) -> str:
    url = urljoin(SITE, href)
    if not (url.startswith(prefix) and _RELEASE_URL.search(url)):
        raise OfficialCalendarError(f"{where}: link de comunicado con forma inesperada {href!r}")
    return url


def _attach(meeting: Meeting | None, link: Link, prefix: str, after_label: bool = False) -> None:
    """Asocia un link a la reunión en curso. El comunicado es el link "Statement" (páginas por
    año) o el link "HTML" después del rótulo "Statement:" (página de calendarios)."""
    if meeting is None:
        return
    if link.text == STATEMENT or (after_label and link.text == "HTML"):
        if meeting.statement_url is not None:
            raise OfficialCalendarError(f"{meeting.label()}: dos links 'Statement'")
        meeting.statement_url = _statement_url(link.href, prefix, meeting.label())
    elif link.text == PRESS_CONFERENCE:
        meeting.press_conference = True


def parse_historical_page(body: str, url: str, year: int, prefix: str) -> list[Meeting]:
    """Reuniones de `fomchistorical{AAAA}.htm`: un encabezado por reunión, como
    "January 28-29 Meeting - 2020" o "March 15 (unscheduled) Meeting - 2020"."""
    meetings: list[Meeting] = []
    current: Meeting | None = None
    for tok in read_page(body).tokens:
        if isinstance(tok, Text):
            m = _HEADING.match(tok.text)
            days = m and parse_days(m["days"], int(m["year"]))
            if m is None or not days:
                continue
            if int(m["year"]) != year:
                raise OfficialCalendarError(f"{url}: reunión de otro año: {tok.text!r}")
            kind = _kind(m["tag"], m["kind"] == "Conference Call", f"{url} {tok.text!r}")
            current = Meeting(year, days[0], days[1], kind, url)
            meetings.append(current)
        else:
            _attach(current, tok, prefix)
    return meetings


def parse_calendars_page(body: str, url: str, prefix: str) -> list[Meeting]:
    """Reuniones de `fomccalendars.htm`: por año ("2025 FOMC Meetings"), el mes ("January",
    "Apr/May") seguido de los días ("28-29", "18-19*", "22 (notation vote)"). El comunicado es
    el link "HTML" que sigue al rótulo "Statement:" (al lado va el "PDF")."""
    meetings: list[Meeting] = []
    current: Meeting | None = None
    year: int | None = None
    month: str | None = None
    after_label = False  # se vio el rótulo "Statement:" y todavía no otro texto
    just_dated = False  # el token anterior fueron los días de una reunión
    for tok in read_page(body).tokens:
        # El encabezado del año es un ancla: `<h4><a id="...">2025 FOMC Meetings</a></h4>`.
        if y := _YEAR_HEADING.match(tok.text):
            year, month, current, after_label, just_dated = int(y["year"]), None, None, False, False
            continue
        if isinstance(tok, Link):
            _attach(current, tok, prefix, after_label)
            just_dated = False
            continue
        text = tok.text
        if text in ("|", "/"):  # separador entre "PDF" y "HTML"
            continue
        after_label = _STATEMENT_LABEL.match(text) is not None
        if year is None:
            continue
        if month is not None and (d := _DAYS.match(text)):
            days = parse_days(f"{month} {d['days']}", year)
            if days is None:
                raise OfficialCalendarError(f"{url}: fecha inválida {month} {text!r} ({year})")
            current = Meeting(year, days[0], days[1], _kind(d["tag"], False, url), url)
            meetings.append(current)
            month, just_dated = None, True
            continue
        if _MONTH.match(text) and all(p.lower() in MONTHS for p in text.split("/")):
            month = text
            continue
        # "(notation vote)" como texto aparte, justo después de los días.
        if just_dated and current is not None and (t := _TAG.match(text)):
            current.kind = _kind(t["tag"], False, url)
        month, just_dated = None, False
    return meetings


@dataclass(frozen=True)
class ReleasePage:
    """Lo que importa de la página de un comunicado."""

    url: str
    day: date
    at: time | None  # "For release at ..."; None si dice "For immediate release"
    title: str
    text: str


def release_day(url: str) -> date:
    m = _RELEASE_URL.search(url)
    if m is None:
        raise OfficialCalendarError(f"{url}: no es un comunicado monetary{{AAAAMMDD}}x.htm")
    return datetime.strptime(m["day"], "%Y%m%d").date()


def parse_release_page(body: str, url: str) -> ReleasePage:
    """Fecha (de la URL, confirmada en la página) y hora de publicación de un comunicado."""
    day = release_day(url)
    page = read_page(body)
    text = page.text()
    month = MONTH_NAMES[day.month]
    if not re.search(rf"\b{month} 0?{day.day}, {day.year}\b", text):
        raise OfficialCalendarError(f"{url}: la página no trae su fecha ({day})")
    start = [m.start() for m in (_FOR_RELEASE.search(text), _IMMEDIATE.search(text)) if m]
    if not start:
        raise OfficialCalendarError(
            f"{url}: la página no dice 'For release at ...' ni 'For immediate release'"
        )
    article = text[min(start) :]
    end = article.find("Last Update")
    article = article if end < 0 else article[:end]
    times = {
        time((int(m["h"]) % 12) + (12 if m["ap"].lower() == "p" else 0), int(m["m"]))
        for m in _FOR_RELEASE.finditer(article)
    }
    if len(times) > 1:
        raise OfficialCalendarError(f"{url}: más de una hora de publicación {sorted(times)}")
    return ReleasePage(url, day, next(iter(times), None), page.title, article)


def eastern(day: date, at: time, tz: ZoneInfo) -> datetime:
    """Hora del Este de ese día, en UTC (con la base de zonas horarias, nunca un desfase fijo)."""
    return datetime.combine(day, at, tzinfo=tz).astimezone(UTC)


@dataclass(frozen=True)
class ScheduleAnnouncement:
    """Un comunicado del calendario tentativo, ya leído."""

    url: str
    years: tuple[int, ...]
    published_at: datetime
    sections: dict[int, str]  # año → texto de su calendario completo
    text: str

    def _forms(self, first: date, last: date) -> list[str]:
        m1, m2 = MONTH_NAMES[first.month], MONTH_NAMES[last.month]
        if first == last:
            # Un solo día: que no siga otro día ("March 15-16") ni la forma larga de dos días.
            return [rf"{m1} {first.day}(?![\d-])(?!, and)"]
        short = (
            f"{m1} {first.day}-{last.day}"
            if first.month == last.month
            else f"{m1} {first.day}-{m2} {last.day}"
        )
        long = rf"{m1} {first.day}, and [A-Z][a-z]+, {m2} {last.day}"
        return [rf"{short}(?!\d)", rf"{long}(?!\d)"]

    def lists(self, first: date, last: date) -> bool:
        """¿Trae la reunión `first` a `last` con esas fechas exactas?"""
        year = first.year
        for form in self._forms(first, last):
            pattern = re.compile(rf"(?<![\w/]){form}(?P<suffix>,? (?P<year>\d{{4}})\b)?")
            # Con año explícito ("January 28-29, 2014"): en cualquier parte del comunicado.
            for m in pattern.finditer(self.text):
                if m["year"] is not None and int(m["year"]) == year:
                    return True
            # Sin año: solo dentro del calendario completo de ese año.
            section = self.sections.get(year)
            if section is not None:
                for m in pattern.finditer(section):
                    if m["year"] is None or int(m["year"]) == year:
                        return True
        return False

    def covers(self, year: int) -> bool:
        """¿Dice algo del calendario de `year`? (su calendario completo o una fecha con año)."""
        if year in self.sections:
            return True
        return re.search(rf"[A-Z][a-z]+ \d{{1,2}}(?:-\d{{1,2}})?, {year}\b", self.text) is not None


def parse_announcement(body: str, ann: Announcement, tz: ZoneInfo) -> ScheduleAnnouncement:
    """Lee un comunicado del calendario tentativo. Su título tiene que nombrar sus años."""
    page = parse_release_page(body, ann.url)
    for y in ann.years:
        if str(y) not in page.title:
            raise OfficialCalendarError(
                f"{ann.url}: el título {page.title!r} no nombra {y}: no es el calendario esperado"
            )
    if len(ann.years) == 1:
        sections = {ann.years[0]: page.text}
    else:
        marks = list(_SECTION.finditer(page.text))
        sections = {}
        for i, m in enumerate(marks):
            end = marks[i + 1].start() if i + 1 < len(marks) else len(page.text)
            sections[int(m["year"])] = page.text[m.end() : end]
        missing = [y for y in ann.years if y not in sections]
        if missing:
            raise OfficialCalendarError(f"{ann.url}: no se encontró el calendario de {missing}")
        sections = {y: s for y, s in sections.items() if y in ann.years}
    # Sin hora en la página: el final de ese día, del lado seguro (ADR 0016 §4).
    at = page.at if page.at is not None else time(23, 59, 59)
    return ScheduleAnnouncement(ann.url, ann.years, eastern(page.day, at, tz), sections, page.text)


def known_since(
    anns: list[ScheduleAnnouncement], first: date, last: date, scheduled_at: datetime
) -> ScheduleAnnouncement | None:
    """El anuncio desde el que se sabía la reunión, o `None` si no se puede documentar.

    Se recorren los anuncios anteriores a la reunión, del más nuevo al más viejo: el más nuevo
    que habla de ese año tiene que traerla con sus fechas exactas, y se retrocede mientras los
    anteriores la sigan trayendo igual. El primero que hable del año sin traerla corta."""
    found = None
    for a in sorted(
        (a for a in anns if a.published_at < scheduled_at),
        key=lambda a: a.published_at,
        reverse=True,
    ):
        if a.lists(first, last):
            found = a
        elif a.covers(first.year):
            break
    return found


@dataclass(frozen=True)
class FedResult:
    releases: list[OfficialRelease]
    excluded: list[str]  # reuniones programadas sin anuncio documentado (se marcan y se pregunta)
    skipped: list[str]  # reuniones sin comunicado de política (canceladas, conferencias, votos)


def collect_meetings(cfg: FedConfig, get: Callable[[str], str]) -> list[Meeting]:
    """Reuniones de `first_year` a `last_year`, con el conteo por año comprobado."""
    meetings: list[Meeting] = []
    for year in range(cfg.first_year, min(cfg.last_year, cfg.historical_last_year) + 1):
        url = cfg.historical_url.format(year=year)
        meetings += parse_historical_page(get(url), url, year, cfg.release_url_prefix)
    if cfg.last_year > cfg.historical_last_year:
        url = cfg.calendars_url
        first = max(cfg.first_year, cfg.historical_last_year + 1)
        meetings += [
            m
            for m in parse_calendars_page(get(url), url, cfg.release_url_prefix)
            if first <= m.year <= cfg.last_year
        ]
    problems = []
    for year in range(cfg.first_year, cfg.last_year + 1):
        of_year = [m for m in meetings if m.year == year]
        planned = [m for m in of_year if m.kind in ("scheduled", "cancelled")]
        if len(planned) != cfg.scheduled_meetings_per_year:
            problems.append(
                f"{year}: {len(planned)} reuniones programadas (con las canceladas), se esperaban"
                f" {cfg.scheduled_meetings_per_year}"
            )
        problems += [
            f"{m.label()}: programada sin link 'Statement'"
            for m in of_year
            if m.kind == "scheduled" and m.statement_url is None
        ]
    if problems:
        raise OfficialCalendarError("la página de reuniones no cuadra:\n  " + "\n  ".join(problems))
    return meetings


def load_fed(cfg: FedConfig, get: Callable[[str], str]) -> FedResult:
    """Arma las filas de `official_release.v1` del FOMC. `get(url) -> html` pide cada página
    (en los tests, de fixtures; en la carga, `PoliteFetcher`)."""
    meetings = collect_meetings(cfg, get)
    anns = [parse_announcement(get(a.url), a, cfg.tz) for a in cfg.announcements]
    releases: list[OfficialRelease] = []
    excluded: list[str] = []
    skipped: list[str] = []
    for m in meetings:
        if m.statement_url is None:
            skipped.append(f"{m.label()}: sin comunicado de política")
            continue
        if m.kind == "cancelled":
            raise OfficialCalendarError(f"{m.label()}: cancelada pero con comunicado")
        page = parse_release_page(get(m.statement_url), m.statement_url)
        # Programada: el comunicado sale el último día. No programada: ese día o después, hasta
        # `max_unscheduled_lag_days` (la del 2-3-2020 salió el 3; la conferencia del 4-10-2019,
        # el 11-10). Su hora sale siempre de su página, así que el atraso no se supone.
        lag = (page.day - m.last_day).days
        ok = lag == 0 if m.kind == "scheduled" else 0 <= lag <= cfg.max_unscheduled_lag_days
        if not ok:
            raise OfficialCalendarError(f"{m.label()}: comunicado del {page.day}, fuera de fecha")
        if m.kind == "scheduled":
            rule = cfg.rule_for(page.day, m.press_conference)
            if page.at is not None and page.at != rule.at:
                raise OfficialCalendarError(
                    f"{m.statement_url}: la página dice {page.at:%H:%M} y la regla {rule.name}"
                    f" dice {rule.at:%H:%M} (ADR 0016 §3.3: no se elige en silencio)"
                )
            at, time_rule = (page.at, RELEASE_PAGE) if page.at is not None else (rule.at, rule.name)
            scheduled_at = eastern(page.day, at, cfg.tz)
            ann = known_since(anns, m.first_day, m.last_day, scheduled_at)
            if ann is None:
                excluded.append(
                    f"{m.label()}: ningún anuncio del calendario la trae con esas fechas antes"
                    " de la reunión (ADR 0016 §6: se marca y se pregunta)"
                )
                continue
            published_at = ann.published_at
        else:
            if page.at is None:
                raise OfficialCalendarError(
                    f"{m.statement_url}: comunicado no programado sin 'For release at ...':"
                    " no se inventa una hora"
                )
            time_rule = RELEASE_PAGE
            scheduled_at = published_at = eastern(page.day, page.at, cfg.tz)
        releases.append(
            OfficialRelease(
                source=SOURCE,
                series=SERIES,
                reference_period=m.last_day.isoformat(),
                status=m.kind,
                scheduled_at=scheduled_at,
                published_at=published_at,
                time_rule=time_rule,
                with_press_conference=m.press_conference,
                source_url=m.statement_url,
            )
        )
    keys = [r.key() for r in releases]
    if len(set(keys)) != len(keys):
        raise OfficialCalendarError("dos comunicados con la misma clave natural")
    return FedResult(releases, excluded, skipped)
