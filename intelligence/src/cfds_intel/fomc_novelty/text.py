"""Texto de un comunicado del FOMC y proporción de palabras cambiadas (#668).

El texto es el de los párrafos del cuerpo de la página oficial
(`<div class="col-xs-12 col-sm-8 col-md-8">` dentro de `#article`), hasta el primer párrafo que
empieza con el marcador de la votación (`[statements] end_marker`). La votación cambia con la
rotación de miembros, no con la política. Si la página no tiene esa forma, falla: no se adivina.
"""

import hashlib
import re
from decimal import ROUND_HALF_EVEN, Decimal
from difflib import SequenceMatcher
from html.parser import HTMLParser

from cfds_intel.calendar.official.pages import normalize
from cfds_intel.fomc_novelty.model import MeasuredText, NoveltyError, Statement

BODY_CLASS = "col-xs-12 col-sm-8 col-md-8"
_WORD = re.compile(r"[a-z0-9]+(?:[.'/-][a-z0-9]+)*")
SIX = Decimal("0.000001")


class _Body(HTMLParser):
    """Párrafos (`<p>`) del primer bloque con la clase exacta del cuerpo, dentro de `#article`."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.paragraphs: list[str] = []
        self._in_article = False
        self._depth = 0  # profundidad de <div> dentro del cuerpo; 0 = fuera
        self._done = False
        self._p: list[str] | None = None

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        a = dict(attrs)
        if tag == "div" and a.get("id") == "article":
            self._in_article = True
        if tag == "div" and self._depth:
            self._depth += 1
        elif tag == "div" and self._in_article and not self._done and a.get("class") == BODY_CLASS:
            self._depth = 1
        elif tag == "p" and self._depth:
            self._p = []

    def handle_endtag(self, tag: str) -> None:
        if tag == "p" and self._p is not None:
            text = normalize(" ".join(self._p))
            if text:
                self.paragraphs.append(text)
            self._p = None
        elif tag == "div" and self._depth:
            self._depth -= 1
            if self._depth == 0:
                self._done = True

    def handle_data(self, data: str) -> None:
        if self._p is not None:
            self._p.append(data)


def extract_statement(html: str, url: str, end_marker: str) -> str:
    """El texto del comunicado, sin la votación ni lo que sigue."""
    p = _Body()
    p.feed(html)
    p.close()
    if not p.paragraphs:
        raise NoveltyError(f"{url}: no se encontró el cuerpo del comunicado")
    for i, para in enumerate(p.paragraphs):
        if para.startswith(end_marker):
            if i == 0:
                raise NoveltyError(f"{url}: el comunicado empieza por la votación")
            return "\n\n".join(p.paragraphs[:i])
    raise NoveltyError(f"{url}: no aparece el párrafo {end_marker!r}")


def words(text: str) -> list[str]:
    return _WORD.findall(text.lower())


def measure(statement: Statement, html: str, end_marker: str) -> MeasuredText:
    text = extract_statement(html, statement.source_url, end_marker)
    n = len(words(text))
    if n == 0:
        raise NoveltyError(f"{statement.source_url}: comunicado sin palabras")
    return MeasuredText(statement, text, hashlib.sha256(text.encode()).hexdigest(), n)


def word_change_share(previous: list[str], current: list[str]) -> Decimal:
    """1 - 2·M/(n₁+n₂), con M las palabras de los bloques iguales (difflib, sin autojunk).

    0 = mismo texto; 1 = ninguna palabra en común en el mismo orden."""
    if not previous or not current:
        raise NoveltyError("no se compara un texto vacío")
    sm = SequenceMatcher(None, previous, current, autojunk=False)
    matched = sum(b.size for b in sm.get_matching_blocks())
    share = Decimal(1) - Decimal(2 * matched) / Decimal(len(previous) + len(current))
    return share.quantize(SIX, rounding=ROUND_HALF_EVEN)
