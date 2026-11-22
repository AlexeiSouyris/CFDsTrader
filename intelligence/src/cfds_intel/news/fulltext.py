"""Texto oficial de una noticia, para interpretarla (ADR 0008 §4, decisión del 22-11-2026).

El título solo casi nunca dice hacia dónde va el mercado ("Federal Reserve issues FOMC
statement"). Por eso el intérprete lee el comunicado del enlace del RSS.

Reglas (ADR 0006):
- Solo de los sitios oficiales permitidos (`[text] allowed_hosts`) y solo por HTTPS, con la
  verificación de certificados de siempre (`cfds_intel.net`).
- El texto se usa para interpretar y **nunca se guarda ni se republica**: del texto solo queda la
  interpretación (resumen propio, sentimiento y relevancia).
- Se recorta a `max_chars` para que entre en el contexto del modelo local.

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
from html.parser import HTMLParser

from pypdf import PdfReader
from pypdf.errors import PdfReadError

from cfds_intel.net import https_get
from cfds_intel.news.feeds import NewsItem
from cfds_intel.news.interpret import InterpretationError

_SKIP = {"script", "style", "noscript", "nav", "header", "footer", "form", "svg"}
_SPACES = re.compile(r"\s+")

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
) -> str:
    """Texto oficial de `item`, recortado a `max_chars`. `opener` se reemplaza en los tests.

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
            pdf_to_text(body, max_chars)
            if is_pdf
            else html_to_text(body.decode("utf-8", errors="replace"))
        )
        if not doc:
            raise FullTextError(f"{doc_url}: el documento no tiene texto")
        text = f"{text} {doc}".strip()
    if not text:
        raise FullTextError(f"{item.url}: la página no tiene texto")
    return text[:max_chars]
