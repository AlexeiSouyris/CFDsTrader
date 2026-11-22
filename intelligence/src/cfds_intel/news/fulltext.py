"""Texto oficial de una noticia, para interpretarla (ADR 0008 §4, decisión del 22-11-2026).

El título solo casi nunca dice hacia dónde va el mercado ("Federal Reserve issues FOMC
statement"). Por eso el intérprete lee el comunicado del enlace del RSS.

Reglas (ADR 0006):
- Solo de los sitios oficiales permitidos (`[text] allowed_hosts`) y solo por HTTPS, con la
  verificación de certificados de siempre (`cfds_intel.net`).
- El texto se usa para interpretar y **nunca se guarda ni se republica**: del texto solo queda la
  interpretación (resumen propio, sentimiento y relevancia).
- Se recorta a `max_chars` para que entre en el contexto del modelo local.
"""

import re
import urllib.parse
import urllib.request
from collections.abc import Callable, Collection
from html.parser import HTMLParser

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


def html_to_text(html: str) -> str:
    """Texto visible de una página, sin scripts, estilos ni menús, con espacios normalizados."""
    p = _Text()
    p.feed(html)
    p.close()
    return _SPACES.sub(" ", " ".join(p.parts)).strip()


def fetch_text(
    item: NewsItem,
    allowed_hosts: Collection[str],
    max_chars: int,
    user_agent: str,
    timeout: int,
    opener: Opener = https_get,
) -> str:
    """Texto oficial de `item`, recortado a `max_chars`. `opener` se reemplaza en los tests."""
    host = urllib.parse.urlparse(item.url).hostname
    if host not in allowed_hosts:
        raise FullTextError(f"{item.url}: el sitio no está en text.allowed_hosts")
    req = urllib.request.Request(item.url, headers={"User-Agent": user_agent})  # noqa: S310
    try:
        body = opener(req, timeout)
    except OSError as e:
        raise FullTextError(f"{item.url}: no se pudo leer ({e})") from e
    text = html_to_text(body.decode("utf-8", errors="replace"))
    if not text:
        raise FullTextError(f"{item.url}: la página no tiene texto")
    return text[:max_chars]
