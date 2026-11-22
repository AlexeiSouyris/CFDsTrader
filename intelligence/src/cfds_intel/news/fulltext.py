"""Texto oficial de una noticia, para interpretarla (ADR 0008 §4, decisión del 22-11-2026).

El título solo casi nunca dice hacia dónde va el mercado ("Federal Reserve issues FOMC
statement"). Por eso el intérprete lee el comunicado del enlace del RSS.

Reglas (ADR 0006):
- Solo de los sitios oficiales permitidos (`[text] allowed_hosts`) y solo por HTTPS, con la
  verificación de certificados de siempre (`cfds_intel.net`).
- El texto se usa para interpretar y **nunca se guarda ni se republica**: del texto solo queda la
  interpretación (resumen propio, sentimiento y relevancia).
- Se recorta a `max_chars` para que entre en el contexto del modelo local.

Selección del texto (#190): las actas del FOMC tienen ~45.000 caracteres y lo más relevante (la
decisión, la discusión de los miembros) está al final, después del recorte. Con un `Layout`:
1. Se quita el texto del sitio que no es la noticia (menús, avisos): patrones de `[text] drop`.
2. Si todavía no entra en `max_chars`, se toma el inicio (`head_chars`) y después cada sección de
   `[text] sections`, en orden de prioridad: desde donde coincide el patrón hasta el próximo
   título de la lista o `section_max_chars`, mientras quede lugar. Lo que sobra completa el
   inicio. Los cortes se marcan con ` […] ` y nunca se pasa de `max_chars`.
Sin `drop` ni `sections` (el `Layout()` vacío), el resultado es el de antes: el inicio.

Documentos enlazados (#168): en las actas del FOMC y las proyecciones, la página del comunicado
solo enlaza al documento. Se sigue **un solo enlace, un solo nivel**: el primero cuya ruta
coincide con un patrón de `[text] follow` (el orden es la preferencia: HTML antes que PDF), y
solo si es HTTPS y de un sitio permitido. Un PDF se lee en memoria, sin escribirlo a disco.
"""

import io
import re
import urllib.parse
import urllib.request
from collections.abc import Callable, Collection, Mapping, Sequence
from dataclasses import dataclass
from html.parser import HTMLParser

from pypdf import PdfReader
from pypdf.errors import PdfReadError

from cfds_intel.net import https_get
from cfds_intel.news.feeds import NewsItem
from cfds_intel.news.interpret import InterpretationError

_SKIP = {"script", "style", "noscript", "nav", "header", "footer", "form", "svg"}
_SPACES = re.compile(r"\s+")
# Marca de texto omitido entre dos partes elegidas.
GAP = " […] "

Opener = Callable[[urllib.request.Request, int], bytes]


class FullTextError(InterpretationError):
    """No se pudo leer el texto oficial: esa noticia no se interpreta."""


class _Text(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self._skipping = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag in _SKIP:
            self._skipping += 1

    def handle_endtag(self, tag: str) -> None:
        if tag in _SKIP and self._skipping:
            self._skipping -= 1

    def handle_data(self, data: str) -> None:
        if not self._skipping:
            self.parts.append(data)


class _Links(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.hrefs: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "a":
            href = dict(attrs).get("href")
            if href:
                self.hrefs.append(href.strip())


def follow_patterns(text_table: Mapping[str, object]) -> tuple[re.Pattern[str], ...]:
    """Patrones de `[text] follow`, validados. Sin la clave, no se sigue ningún enlace."""
    raw = text_table.get("follow", [])
    if not isinstance(raw, list) or not all(isinstance(r, str) and r.startswith("^") for r in raw):
        raise FullTextError('text.follow: lista de expresiones de ruta que empiezan con "^"')
    try:
        return tuple(re.compile(r) for r in raw)
    except re.error as e:
        raise FullTextError(f"text.follow: expresión inválida ({e})") from e


@dataclass(frozen=True)
class Layout:
    """Qué parte del texto oficial se lee cuando no entra entero (#190)."""

    # Texto del sitio que no es la noticia: se quita antes de elegir.
    drop: tuple[re.Pattern[str], ...] = ()
    # Caracteres del inicio que siempre se toman.
    head_chars: int = 0
    # Secciones clave, en orden de prioridad.
    sections: tuple[re.Pattern[str], ...] = ()
    section_max_chars: int = 0
    # Caracteres de un PDF enlazado que se leen para buscar las secciones (0 = max_chars).
    scan_max_chars: int = 0


def _patterns(table: Mapping[str, object], key: str) -> tuple[re.Pattern[str], ...]:
    raw = table.get(key, [])
    if not isinstance(raw, list) or not all(isinstance(r, str) and r for r in raw):
        raise FullTextError(f"text.{key}: lista de expresiones regulares")
    try:
        return tuple(re.compile(r, re.DOTALL) for r in raw)
    except re.error as e:
        raise FullTextError(f"text.{key}: expresión inválida ({e})") from e


def layout_from(table: Mapping[str, object], max_chars: int) -> Layout:
    """`Layout` de la tabla `[text]`, validado. Sin las claves, el `Layout()` vacío."""

    def chars(key: str) -> int:
        v = table.get(key, 0)
        if not isinstance(v, int) or isinstance(v, bool) or v < 0:
            raise FullTextError(f"text.{key} tiene que ser un entero ≥ 0")
        return v

    sections = _patterns(table, "sections")
    head, section_max, scan = (
        chars("head_chars"),
        chars("section_max_chars"),
        chars("scan_max_chars"),
    )
    if head > max_chars:
        raise FullTextError("text.head_chars no puede pasar de text.max_chars")
    if sections and section_max < 1:
        raise FullTextError("text.section_max_chars tiene que ser ≥ 1 si hay text.sections")
    if scan and scan < max_chars:
        raise FullTextError("text.scan_max_chars tiene que ser 0 o ≥ text.max_chars")
    return Layout(_patterns(table, "drop"), head, sections, section_max, scan)


def select_text(text: str, max_chars: int, layout: Layout) -> str:
    """El texto que lee el modelo: nunca más de `max_chars` caracteres (ver el módulo)."""
    for p in layout.drop:
        text = p.sub(" ", text)
    text = _SPACES.sub(" ", text).strip()
    if len(text) <= max_chars:
        return text
    found = [(p, p.search(text)) for p in layout.sections]
    starts = sorted({m.start() for _, m in found if m})
    taken = [False] * len(text)
    # Lugar para las marcas de corte: a lo sumo una por sección más una al final del inicio.
    budget = max_chars - len(GAP) * len(starts)
    head = min(layout.head_chars, budget)
    for i in range(head):
        taken[i] = True
    budget -= head
    for _, m in found:
        if m is None or budget <= 0:
            continue
        start = m.start()
        end = min([s for s in starts if s > start] + [start + layout.section_max_chars, len(text)])
        for i in range(start, end):
            if budget <= 0:
                break
            if not taken[i]:
                taken[i] = True
                budget -= 1
    # Lo que sobra completa el texto desde el inicio.
    for i in range(len(text)):
        if budget <= 0:
            break
        if not taken[i]:
            taken[i] = True
            budget -= 1
    parts: list[str] = []
    i = 0
    while i < len(text):
        if taken[i]:
            j = i
            while j < len(text) and taken[j]:
                j += 1
            parts.append(text[i:j].strip())
            i = j
        else:
            i += 1
    return GAP.join(p for p in parts if p)


def linked_document(
    page_url: str, html: str, allowed_hosts: Collection[str], follow: Sequence[re.Pattern[str]]
) -> str | None:
    """URL del documento que enlaza la página, o `None`. Solo HTTPS y de un sitio permitido."""
    p = _Links()
    p.feed(html)
    p.close()
    candidates = [urllib.parse.urlparse(urllib.parse.urljoin(page_url, h)) for h in p.hrefs]
    ok = [c for c in candidates if c.scheme == "https" and c.hostname in allowed_hosts and c.path]
    for pattern in follow:
        for c in ok:
            if pattern.search(c.path):
                return urllib.parse.urlunparse(c._replace(fragment=""))
    return None


def pdf_to_text(data: bytes, max_chars: int) -> str:
    """Texto de un PDF, leído en memoria, hasta juntar `max_chars`."""
    try:
        reader = PdfReader(io.BytesIO(data))
        parts: list[str] = []
        size = 0
        for page in reader.pages:
            chunk = page.extract_text() or ""
            parts.append(chunk)
            size += len(chunk)
            if size >= max_chars:
                break
    except (PdfReadError, ValueError, KeyError) as e:
        raise FullTextError(f"el PDF no se pudo leer ({e})") from e
    return _SPACES.sub(" ", " ".join(parts)).strip()


def html_to_text(html: str) -> str:
    """Texto visible de una página, sin scripts, estilos ni menús, con espacios normalizados."""
    p = _Text()
    p.feed(html)
    p.close()
    return _SPACES.sub(" ", " ".join(p.parts)).strip()


def _get(url: str, user_agent: str, timeout: int, opener: Opener) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": user_agent})  # noqa: S310
    try:
        return opener(req, timeout)
    except OSError as e:
        raise FullTextError(f"{url}: no se pudo leer ({e})") from e


def fetch_text(
    item: NewsItem,
    allowed_hosts: Collection[str],
    max_chars: int,
    user_agent: str,
    timeout: int,
    opener: Opener = https_get,
    follow: Sequence[re.Pattern[str]] = (),
    layout: Layout = Layout(),
) -> str:
    """Texto oficial de `item`, elegido con `layout` y de hasta `max_chars` caracteres.
    `opener` se reemplaza en los tests.

    Con `follow`, si la página enlaza un documento que coincide, se suma su texto.
    """
    host = urllib.parse.urlparse(item.url).hostname
    if host not in allowed_hosts:
        raise FullTextError(f"{item.url}: el sitio no está en text.allowed_hosts")
    html = _get(item.url, user_agent, timeout, opener).decode("utf-8", errors="replace")
    text = html_to_text(html)
    doc_url = linked_document(item.url, html, allowed_hosts, follow) if follow else None
    if doc_url:
        body = _get(doc_url, user_agent, timeout, opener)
        is_pdf = body.startswith(b"%PDF") or urllib.parse.urlparse(doc_url).path.endswith(".pdf")
        doc = (
            pdf_to_text(body, layout.scan_max_chars or max_chars)
            if is_pdf
            else html_to_text(body.decode("utf-8", errors="replace"))
        )
        if not doc:
            raise FullTextError(f"{doc_url}: el documento no tiene texto")
        text = f"{text} {doc}".strip()
    if not text:
        raise FullTextError(f"{item.url}: la página no tiene texto")
    return select_text(text, max_chars, layout)
