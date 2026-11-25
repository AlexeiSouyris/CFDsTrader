"""CPI y empleo (*Employment Situation*) del BLS, 2011-2026 (ADR 0016, #608).

**Sin robot** (ADR 0016 §1.3, elección del dueño): el dueño guardó a mano desde su navegador las
páginas por año (`https://www.bls.gov/schedule/{AAAA}/home.htm`) y los avisos de fechas
revisadas por los cortes de fondos. Este módulo **solo lee esos HTML locales**; nunca pide
bls.gov ni esquiva su 403 (§1.4). Fuente: U.S. Bureau of Labor Statistics (dominio público).

Reglas (ADR 0016 §4, lectura conservadora):

- Hora: la del calendario (08:30 ET), pasada a UTC con `America/New_York` (EST o EDT según el
  día, nunca un desfase fijo).
- `published_at` de una publicación programada = la hora del **informe anterior de la misma
  serie**, que anuncia la fecha siguiente. La primera de cada serie no tiene informe anterior en
  las páginas: **queda fuera**.
- Las páginas por año muestran la fecha **final**. Los avisos de cierre dicen cuáles se movieron
  y su fecha original. La fila original **queda** (`scheduled`) si el informe anterior salió
  antes del corte (entonces anunció la fecha original); si no, queda fuera. La fecha nueva es
  otra fila (`rescheduled`) que se conoce desde el informe anterior si este salió **después** del
  corte (ya con el calendario revisado, como el empleo del 22-10-2013 que anuncia el del
  8-11-2013), o si no desde el final del día de "Last modified date" del aviso, si es anterior a
  la publicación. Si no hay ninguna de las dos, queda fuera.
- Una cancelación (CPI y empleo de octubre de 2025) es una fila `cancelled` sin hora, conocida
  desde el final del día de "Last modified date" del aviso (la única fecha documentada).
- Una publicación sin aviso cuyo informe anterior salió antes de un corte que cae entre los dos
  (el CPI de septiembre de 2025, del 24-10) pudo moverse sin que lo sepamos: **queda fuera**.
- Cualquier estructura inesperada (año faltante o repetido, otra hora, meses salteados, cuentas
  por año que no dan, un aviso que no coincide con el calendario) **hace fallar la carga**.
"""

import itertools
import re
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import UTC, date, datetime, time
from html.parser import HTMLParser
from pathlib import Path

from cfds_intel.calendar.official.model import (
    BlsConfig,
    Lapse,
    OfficialCalendarError,
    OfficialRelease,
)
from cfds_intel.calendar.official.pages import MONTH_NAMES, MONTHS, normalize

SERIES = {"Consumer Price Index": "cpi", "Employment Situation": "employment_situation"}
RELEASES_PER_YEAR = 12
_YEAR_TITLE = re.compile(r"^Schedule of (?:Selected )?Releases (?:for )?(\d{4})$")
_MONTH_HEADING = re.compile(r"^([A-Z][a-z]+),? (\d{4})$")
_DESC = re.compile(r"^(Consumer Price Index|Employment Situation) for ([A-Z][a-z]+) (\d{4})$")
_LAST_MODIFIED = re.compile(r"Last modified date: ([A-Z][a-z]+ \d{1,2}, \d{4})", re.IGNORECASE)
_NOTICE_TIME = re.compile(r"^(\d{1,2}):(\d{2}) ?(?:a\.m\.|AM)(?: ET)?$")
_WEEKDAYS = ("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday")


class _Tables(HTMLParser):
    """Título, link canónico, encabezados h1 y filas de tablas (texto de cada celda)."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.title = ""
        self.canonical: str | None = None
        self.headings: list[str] = []
        self.rows: list[list[str]] = []
        self.text: list[str] = []
        self._skip = 0
        self._in: str | None = None  # "title", "h1"
        self._buf: list[str] = []
        self._row: list[str] | None = None
        self._cell: list[str] | None = None

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        a = dict(attrs)
        if tag in ("script", "style"):
            self._skip += 1
        elif tag in ("title", "h1"):
            self._in, self._buf = tag, []
        elif tag == "link" and (a.get("rel") or "").lower() == "canonical":
            self.canonical = a.get("href")
        elif tag == "tr":
            self._row = []
        elif tag in ("td", "th") and self._row is not None:
            self._cell = []

    def handle_endtag(self, tag: str) -> None:
        if tag in ("script", "style"):
            self._skip = max(0, self._skip - 1)
        elif tag == self._in:
            text = normalize(" ".join(self._buf))
            if tag == "title":
                self.title = text
            else:
                self.headings.append(text)
            self._in = None
        elif tag in ("td", "th") and self._row is not None and self._cell is not None:
            self._row.append(normalize(" ".join(self._cell)))
            self._cell = None
        elif tag == "tr" and self._row is not None:
            self.rows.append(self._row)
            self._row = None

    def handle_data(self, data: str) -> None:
        if self._skip:
            return
        if self._in is not None:
            self._buf.append(data)
        if self._cell is not None:
            self._cell.append(data)
        self.text.append(data)


def _read(html: str) -> _Tables:
    p = _Tables()
    p.feed(html)
    p.close()
    return p


def _month(name: str, where: str) -> int:
    m = MONTHS.get(name.lower())
    if m is None:
        raise OfficialCalendarError(f"{where}: mes desconocido {name!r}")
    return m


def parse_day(text: str, where: str) -> date:
    """`Friday, March 08, 2013` o `Friday, March 8, 2025`, con el día de la semana comprobado."""
    m = re.fullmatch(r"([A-Za-z]+), ([A-Za-z]+) (\d{1,2}), (\d{4})", normalize(text))
    if m is None:
        raise OfficialCalendarError(f"{where}: fecha con forma inesperada {text!r}")
    try:
        day = date(int(m[4]), _month(m[2], where), int(m[3]))
    except ValueError as e:
        raise OfficialCalendarError(f"{where}: fecha inválida {text!r}") from e
    if _WEEKDAYS[day.weekday()] != m[1].lower():
        raise OfficialCalendarError(f"{where}: {text!r} no cae ese día de la semana")
    return day


def _ref(month: str, year: str, where: str) -> str:
    return f"{int(year):04d}-{_month(month, where):02d}"


@dataclass(frozen=True)
class YearRow:
    """Una publicación tal como figura (fecha final) en la página del año."""

    series: str
    reference_period: str  # AAAA-MM
    day: date
    source_url: str


@dataclass(frozen=True)
class NoticeEntry:
    """Una fila de CPI o empleo de un aviso de fechas revisadas."""

    series: str
    reference_period: str
    original: date
    kind: str  # "rescheduled", "cancelled" o "no_change"
    revised: date | None
    source_url: str
    last_modified: date


def parse_year_page(html: str, cfg: BlsConfig) -> tuple[int, list[YearRow]]:
    """Año (por el título, no por el nombre del archivo) y sus filas de CPI y empleo."""
    page = _read(html)
    m = _YEAR_TITLE.match(page.title)
    if m is None:
        raise OfficialCalendarError(f"página por año con título inesperado: {page.title!r}")
    year = int(m[1])
    where = f"calendario del BLS {year}"
    url = cfg.year_url.format(year=year)
    path = url.split("bls.gov", 1)[1]
    if path not in html:
        raise OfficialCalendarError(f"{where}: la página no nombra su URL {url}")
    months = []
    for h in page.headings:
        hm = _MONTH_HEADING.match(h)
        if hm is None or int(hm[2]) != year:
            raise OfficialCalendarError(f"{where}: encabezado inesperado {h!r}")
        months.append(_month(hm[1], where))
    if months != list(range(1, 13)):
        raise OfficialCalendarError(f"{where}: los meses no son enero a diciembre: {months}")
    want = f"{cfg.release_time.hour:02d}:{cfg.release_time.minute:02d} AM"
    rows: list[YearRow] = []
    for cells in page.rows:
        if len(cells) != 3:
            continue
        desc = cells[2]
        if not any(desc.startswith(f"{name} for ") for name in SERIES):
            continue
        dm = _DESC.fullmatch(desc)
        if dm is None:
            raise OfficialCalendarError(f"{where}: fila con forma inesperada {desc!r}")
        day = parse_day(cells[0], where)
        if day.year != year:
            raise OfficialCalendarError(f"{where}: {desc!r} cae en {day}, fuera del año")
        if cells[1] != want:
            raise OfficialCalendarError(f"{where}: {desc!r} a las {cells[1]!r}, no {want!r}")
        rows.append(YearRow(SERIES[dm[1]], _ref(dm[2], dm[3], where), day, url))
    return year, rows


def parse_notice_page(html: str, cfg: BlsConfig) -> list[NoticeEntry]:
    """Filas de CPI y empleo de un aviso de fechas revisadas por un corte de fondos."""
    page = _read(html)
    url = page.canonical
    if url not in {n.url for n in cfg.notices}:
        raise OfficialCalendarError(f"aviso desconocido (link canónico {url!r})")
    lm = _LAST_MODIFIED.search(normalize(" ".join(page.text)))
    if lm is None:
        raise OfficialCalendarError(f"{url}: falta 'Last modified date'")
    try:
        last_modified = datetime.strptime(lm[1], "%B %d, %Y").date()
    except ValueError as e:
        raise OfficialCalendarError(f"{url}: 'Last modified date' inválida {lm[1]!r}") from e
    if not any(
        len(r) == 5 and "previously scheduled release date" in r[2].lower() for r in page.rows
    ):
        raise OfficialCalendarError(f"{url}: no tiene la tabla de fechas revisadas")
    entries: list[NoticeEntry] = []
    for cells in page.rows:
        if len(cells) != 5 or cells[0] not in SERIES:
            continue
        series = SERIES[cells[0]]
        rm = re.fullmatch(r"([A-Z][a-z]+) (\d{4})", cells[1])
        if rm is None:
            raise OfficialCalendarError(f"{url}: período inesperado {cells[1]!r}")
        ref = _ref(rm[1], rm[2], url)
        where = f"{url} {series} {ref}"
        original = parse_day(cells[2], where)
        revised_text = cells[3]
        revised: date | None = None
        if revised_text.lower().startswith("canceled"):
            kind = "cancelled"
        elif revised_text.upper() == "NO CHANGE":
            kind = "no_change"
        else:
            kind, revised = "rescheduled", parse_day(revised_text, where)
        if kind != "cancelled":
            tm = _NOTICE_TIME.fullmatch(cells[4])
            if tm is None or time(int(tm[1]), int(tm[2])) != cfg.release_time:
                raise OfficialCalendarError(f"{where}: hora inesperada {cells[4]!r}")
        entries.append(NoticeEntry(series, ref, original, kind, revised, url, last_modified))
    return entries


@dataclass
class BlsResult:
    releases: list[OfficialRelease]
    excluded: list[str]  # filas que quedan fuera por la regla del ADR 0016 §4 (con el motivo)
    counts: dict[tuple[str, int], int]  # publicaciones por (serie, año) en las páginas


def _months_between(ref: str, day: date) -> int:
    y, m = int(ref[:4]), int(ref[5:])
    return (day.year - y) * 12 + day.month - m


def _next_ref(ref: str) -> str:
    y, m = int(ref[:4]), int(ref[5:])
    return f"{y + m // 12:04d}-{m % 12 + 1:02d}"


def build_bls(
    cfg: BlsConfig,
    year_pages: Iterable[str],
    notice_pages: Iterable[str],
    now: datetime,
) -> BlsResult:
    """Arma las filas de `official_release.v1` del CPI y del empleo desde los HTML guardados."""
    rows_by_year: dict[int, list[YearRow]] = {}
    for html in year_pages:
        year, rows = parse_year_page(html, cfg)
        if year in rows_by_year:
            raise OfficialCalendarError(f"calendario del BLS {year} repetido")
        rows_by_year[year] = rows
    want_years = set(range(cfg.first_year, cfg.last_year + 1))
    if set(rows_by_year) != want_years:
        missing = sorted(want_years - set(rows_by_year))
        extra = sorted(set(rows_by_year) - want_years)
        raise OfficialCalendarError(f"calendarios del BLS: faltan {missing}, sobran {extra}")

    entries: dict[tuple[str, str], NoticeEntry] = {}
    seen_notices: set[str] = set()
    for html in notice_pages:
        parsed = parse_notice_page(html, cfg)
        url = _read(html).canonical or ""
        if url in seen_notices:
            raise OfficialCalendarError(f"aviso repetido: {url}")
        seen_notices.add(url)
        for e in parsed:
            k = (e.series, e.reference_period)
            if k in entries:
                raise OfficialCalendarError(f"{e.series} {e.reference_period}: dos avisos")
            entries[k] = e
    missing_notices = {n.url for n in cfg.notices} - seen_notices
    if missing_notices:
        raise OfficialCalendarError(f"faltan avisos de cierre: {sorted(missing_notices)}")

    def at(day: date) -> datetime:
        return datetime.combine(day, cfg.release_time, cfg.tz).astimezone(UTC)

    def end_of(day: date) -> datetime:
        return datetime.combine(day, time(23, 59, 59), cfg.tz).astimezone(UTC)

    def start_of(day: date) -> datetime:
        return datetime.combine(day, time(0, 0), cfg.tz).astimezone(UTC)

    def lapse_for(day: date, where: str) -> Lapse:
        found = [lp for lp in cfg.lapses if lp.start <= day]
        if not found:
            raise OfficialCalendarError(f"{where}: aviso de cierre sin corte de fondos en config")
        return found[-1]

    releases: list[OfficialRelease] = []
    excluded: list[str] = []
    counts: dict[tuple[str, int], int] = {}
    for series in SERIES.values():
        actual: dict[str, YearRow] = {}
        for year in sorted(rows_by_year):
            year_rows = [r for r in rows_by_year[year] if r.series == series]
            cancelled_here = sum(
                1
                for e in entries.values()
                if e.series == series and e.kind == "cancelled" and e.original.year == year
            )
            counts[(series, year)] = len(year_rows)
            if len(year_rows) != RELEASES_PER_YEAR - cancelled_here:
                raise OfficialCalendarError(
                    f"{series} {year}: {len(year_rows)} publicaciones, se esperaban"
                    f" {RELEASES_PER_YEAR - cancelled_here}"
                )
            for r in year_rows:
                if r.reference_period in actual:
                    raise OfficialCalendarError(f"{series} {r.reference_period}: repetida")
                actual[r.reference_period] = r
        mine = {ref: e for (s, ref), e in entries.items() if s == series}
        for ref, e in mine.items():
            where = f"{series} {ref} ({e.source_url})"
            row = actual.get(ref)
            if e.kind == "cancelled":
                if row is not None:
                    raise OfficialCalendarError(f"{where}: cancelada pero figura el {row.day}")
            elif row is None:
                if e.original.year in want_years:
                    raise OfficialCalendarError(f"{where}: el aviso la nombra y no figura")
            elif row.day != (e.revised if e.kind == "rescheduled" else e.original):
                raise OfficialCalendarError(
                    f"{where}: el calendario dice {row.day} y el aviso no coincide"
                )
        refs = sorted(set(actual) | {ref for ref, e in mine.items() if e.kind == "cancelled"})
        for a, b in itertools.pairwise(refs):
            if _next_ref(a) != b:
                raise OfficialCalendarError(f"{series}: falta el mes entre {a} y {b}")
        last_day: date | None = None
        for ref in refs:
            row = actual.get(ref)
            if row is None:
                continue
            if not 1 <= _months_between(ref, row.day) <= 2:
                raise OfficialCalendarError(
                    f"{series} {ref}: publicada el {row.day}, fuera de rango"
                )
            if last_day is not None and row.day <= last_day:
                raise OfficialCalendarError(f"{series} {ref}: fechas fuera de orden")
            last_day = row.day

        prev: datetime | None = None  # hora del informe anterior publicado de la serie
        for ref in refs:
            row = actual.get(ref)
            entry = mine.get(ref)
            label = f"{series} {ref}"
            if entry is not None and entry.kind in ("cancelled", "rescheduled"):
                lapse = lapse_for(entry.original, label)
                o_at = at(entry.original)
                if prev is not None and prev < start_of(lapse.start) and prev <= o_at:
                    releases.append(
                        OfficialRelease(
                            "bls", series, ref, "scheduled", o_at, prev,
                            cfg.time_rule, None, entry.source_url,
                        )
                    )  # fmt: skip
                else:
                    excluded.append(
                        f"{label} fecha original {entry.original}: el informe anterior salió"
                        " después de empezar el corte (no se sabe que la anunciara)"
                    )
                notice_known = end_of(entry.last_modified)
                if entry.kind == "cancelled":
                    releases.append(
                        OfficialRelease(
                            "bls", series, ref, "cancelled", None, notice_known,
                            cfg.notice_time_rule, None, entry.source_url,
                        )
                    )  # fmt: skip
                    continue
                assert row is not None and entry.revised is not None  # noqa: S101 (validado arriba)
                d_at = at(entry.revised)
                if prev is not None and prev > end_of(lapse.end):
                    known, rule = prev, cfg.time_rule
                else:
                    known, rule = notice_known, cfg.notice_time_rule
                if known <= d_at:
                    releases.append(
                        OfficialRelease(
                            "bls", series, ref, "rescheduled", d_at, known, rule, None,
                            entry.source_url,
                        )
                    )  # fmt: skip
                else:
                    excluded.append(
                        f"{label} fecha revisada {entry.revised}: no hay aviso documentado antes"
                        f" (el informe anterior es del corte o previo y el aviso es del"
                        f" {entry.last_modified})"
                    )
                prev = d_at
                continue
            assert row is not None  # noqa: S101 (solo las canceladas no tienen fila)
            r_at = at(row.day)
            if prev is None:
                excluded.append(f"{label} del {row.day}: sin informe anterior en las páginas")
            elif entry is None and any(prev < start_of(lp.start) <= r_at for lp in cfg.lapses):
                excluded.append(
                    f"{label} del {row.day}: un corte de fondos cae entre el informe anterior"
                    " y esta fecha y ningún aviso la documenta (pudo moverse)"
                )
            else:
                releases.append(
                    OfficialRelease(
                        "bls", series, ref, "scheduled", r_at, prev, cfg.time_rule, None,
                        row.source_url,
                    )
                )  # fmt: skip
            prev = r_at

    known_now = [rel for rel in releases if rel.published_at <= now]
    excluded.extend(
        f"{rel.series} {rel.reference_period} {rel.status}: todavía no se conoce"
        for rel in releases
        if rel.published_at > now
    )
    known_now.sort(key=lambda rel: (rel.series, rel.reference_period, rel.published_at, rel.status))
    return BlsResult(known_now, excluded, counts)


def read_bls_dir(raw_dir: Path, cfg: BlsConfig) -> tuple[list[str], list[str]]:
    """Separa los HTML guardados en páginas por año y avisos, **por su contenido**. Un archivo
    que no es ninguna de las dos cosas hace fallar la carga."""
    if not raw_dir.is_dir():
        raise OfficialCalendarError(f"no existe la carpeta de páginas del BLS: {raw_dir}")
    notice_urls = {n.url for n in cfg.notices}
    years: list[str] = []
    notices: list[str] = []
    files = sorted(raw_dir.glob("*.htm*"))
    if not files:
        raise OfficialCalendarError(f"{raw_dir}: no hay páginas guardadas")
    for f in files:
        try:
            html = f.read_text(encoding="utf-8")
        except UnicodeDecodeError as e:
            raise OfficialCalendarError(f"{f.name}: no es UTF-8") from e
        page = _read(html)
        if _YEAR_TITLE.match(page.title):
            years.append(html)
        elif page.canonical in notice_urls:
            notices.append(html)
        else:
            raise OfficialCalendarError(f"{f.name}: no es un calendario ni un aviso conocido")
    return years, notices


def load_bls(cfg: BlsConfig, raw_dir: Path, now: datetime) -> BlsResult:
    years, notices = read_bls_dir(raw_dir, cfg)
    return build_bls(cfg, years, notices, now)


def month_name(ref: str) -> str:
    """`2013-09` → `September 2013` (para mensajes y fixtures)."""
    return f"{MONTH_NAMES[int(ref[5:])]} {ref[:4]}"
