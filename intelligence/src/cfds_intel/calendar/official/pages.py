"""Lectura de páginas HTML oficiales como texto y links, sin depender del diseño (ADR 0016).

Las páginas de la Fed (y después las del BLS) cambian clases y columnas con los años. Por eso
no se buscan clases CSS: la página se reduce a una secuencia de textos y links en orden, y los
lectores de cada fuente reconocen lo que necesitan por su texto. Si algo no aparece, el lector
falla: no se adivina.
"""

import re
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from datetime import date
from html.parser import HTMLParser
from time import monotonic, sleep
from urllib.error import HTTPError, URLError

from cfds_intel.calendar.official.model import OfficialCalendarError
from cfds_intel.net import https_get

MONTHS = {
    name: i
    for i, names in enumerate(
        [
            ("january", "jan"),
            ("february", "feb"),
            ("march", "mar"),
            ("april", "apr"),
            ("may",),
            ("june", "jun"),
            ("july", "jul"),
            ("august", "aug"),
            ("september", "sep", "sept"),
            ("october", "oct"),
            ("november", "nov"),
            ("december", "dec"),
        ],
        start=1,
    )
    for name in names
}
MONTH_NAMES = [
    "",
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
]
_DASHES = re.compile(r"\s*[-\u2010\u2011\u2012\u2013\u2014]\s*")
_SPACES = re.compile(r"\s+")


def normalize(text: str) -> str:
    """Espacios colapsados y guiones (incluidos los tipográficos) como `-` sin espacios."""
    return _DASHES.sub("-", _SPACES.sub(" ", text.replace("\xa0", " "))).strip()


@dataclass(frozen=True)
class Text:
    text: str


@dataclass(frozen=True)
class Link:
    href: str
    text: str


Token = Text | Link


class _Tokenizer(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.tokens: list[Token] = []
        self.title = ""
        self._skip = 0
        self._in_title = False
        self._href: str | None = None
        self._link_text: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag in ("script", "style"):
            self._skip += 1
        elif tag == "title":
            self._in_title = True
        elif tag == "a":
            self._href = dict(attrs).get("href") or ""
            self._link_text = []

    def handle_endtag(self, tag: str) -> None:
        if tag in ("script", "style"):
            self._skip = max(0, self._skip - 1)
        elif tag == "title":
            self._in_title = False
        elif tag == "a" and self._href is not None:
            self.tokens.append(Link(self._href, normalize(" ".join(self._link_text))))
            self._href = None

    def handle_data(self, data: str) -> None:
        if self._skip:
            return
        if self._in_title:
            self.title += data
        elif self._href is not None:
            self._link_text.append(data)
        else:
            text = normalize(data)
            if text:
                self.tokens.append(Text(text))


@dataclass(frozen=True)
class Page:
    title: str
    tokens: list[Token]

    def text(self) -> str:
        """Todo el texto visible en orden, links incluidos."""
        return normalize(" ".join(t.text for t in self.tokens))


def read_page(body: str) -> Page:
    p = _Tokenizer()
    p.feed(body)
    p.close()
    return Page(normalize(p.title), p.tokens)


def parse_days(text: str, year: int) -> tuple[date, date] | None:
    """Primer y último día de "January 28-29", "July 31-August 1", "Apr/May 30-1" o "March 15".

    `None` si el texto no es una fecha así (no es una reunión)."""
    s = normalize(text)
    forms = [
        r"(?P<m1>[A-Za-z]+)/(?P<m2>[A-Za-z]+) (?P<d1>\d{1,2})-(?P<d2>\d{1,2})",
        r"(?P<m1>[A-Za-z]+) (?P<d1>\d{1,2})-(?P<m2>[A-Za-z]+) (?P<d2>\d{1,2})",
        r"(?P<m1>[A-Za-z]+) (?P<d1>\d{1,2})-(?P<d2>\d{1,2})",
        r"(?P<m1>[A-Za-z]+) (?P<d1>\d{1,2})",
    ]
    for form in forms:
        m = re.fullmatch(form, s)
        if m is None:
            continue
        g = m.groupdict()
        m1 = MONTHS.get(g["m1"].lower())
        m2 = MONTHS.get((g.get("m2") or g["m1"]).lower())
        if m1 is None or m2 is None:
            return None
        d1 = int(g["d1"])
        d2 = int(g.get("d2") or d1)
        try:
            first, last = date(year, m1, d1), date(year, m2, d2)
        except ValueError:
            return None
        return (first, last) if first <= last else None
    return None


Opener = Callable[[urllib.request.Request, int], bytes]


class PoliteFetcher:
    """Pide páginas de una lista de prefijos permitidos, de a una, con pausa entre pedidos.

    Sin reintentos: un 403, 404 o 429 frena la carga con un mensaje claro. Nunca se esquiva un
    bloqueo (ni otro User-Agent ni otra vía, ADR 0016 §1.4)."""

    def __init__(
        self,
        user_agent: str,
        timeout_secs: int,
        pause_secs: float,
        allowed_prefixes: tuple[str, ...],
        opener: Opener = https_get,
        wait: Callable[[float], None] = sleep,
        clock: Callable[[], float] = monotonic,
    ) -> None:
        self.user_agent = user_agent
        self.timeout_secs = timeout_secs
        self.pause_secs = pause_secs
        self.allowed = allowed_prefixes
        self.opener = opener
        self.wait = wait
        self.clock = clock
        self.requests = 0
        self._last: float | None = None
        self._cache: dict[str, str] = {}

    def __call__(self, url: str) -> str:
        if url in self._cache:
            return self._cache[url]
        if not any(url.startswith(p) for p in self.allowed):
            raise OfficialCalendarError(f"URL fuera de las documentadas en la config: {url}")
        if self._last is not None:
            remaining = self.pause_secs - (self.clock() - self._last)
            if remaining > 0:
                self.wait(remaining)
        req = urllib.request.Request(url, headers={"User-Agent": self.user_agent})  # noqa: S310
        try:
            body = self.opener(req, self.timeout_secs)
        except HTTPError as e:
            raise OfficialCalendarError(
                f"{url}: el sitio respondió {e.code}. No se reintenta ni se esquiva el bloqueo"
                " (ADR 0016 §1.4): avisá al dueño"
            ) from e
        except (URLError, OSError) as e:
            raise OfficialCalendarError(f"{url}: no se pudo leer ({e})") from e
        finally:
            self.requests += 1
            self._last = self.clock()
        try:
            text = body.decode("utf-8")
        except UnicodeDecodeError as e:
            raise OfficialCalendarError(f"{url}: la página no es UTF-8") from e
        self._cache[url] = text
        return text
