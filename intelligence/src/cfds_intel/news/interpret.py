"""Interpretación de noticias como evento `news_interpretation.v1` (#122, spec §9).

El intérprete es **reemplazable** (`Interpreter`): este módulo no elige modelo ni prompt. Solo
fija las reglas que cualquier intérprete tiene que cumplir:

- La interpretación existe desde `interpreted_at`, que nunca es anterior a `published_at`
  (CLAUDE.md §5b). No se interpreta una noticia "del futuro".
- Sentimiento en [-1, 1] y relevancia en [0, 1], como `Decimal` exacto con hasta 4 decimales.
  Un `float` o un valor fuera de rango es un error, no se redondea ni se recorta en silencio.
- Solo instrumentos de `config/instruments.toml`, sin repetir.
- Modelo, prompt y versión del intérprete quedan registrados en cada evento.
- El ajuste de -2 a +2 del puntaje (spec §6) **no** se calcula acá: es parte del puntaje, que
  vive en el motor en Rust (CLAUDE.md §1).
"""

import hashlib
import re
import tomllib
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from decimal import Decimal
from typing import Protocol

from cfds_intel.news.feeds import NewsItem

_NAME = re.compile(r"^[a-z][a-z0-9_-]{0,63}$")
_VERSION = re.compile(r"^[0-9A-Za-z][0-9A-Za-z._+-]{0,63}$")
_SHA256 = re.compile(r"^[0-9a-f]{64}$")
_INSTRUMENT = re.compile(r"^[A-Z0-9][A-Z0-9._-]{0,31}$")
_MAX_SUMMARY = 1000
_MAX_MODEL = 200
_MAX_IMPACTS = 64
_MAX_DECIMALS = 4


class InterpretationError(ValueError):
    """El intérprete devolvió algo que no cumple las reglas: no se emite ningún evento."""


@dataclass(frozen=True)
class InterpreterInfo:
    """Qué interpreta: se registra en cada evento para reproducirlo y medirlo."""

    name: str
    version: str
    model: str | None
    prompt_sha256: str | None

    def __post_init__(self) -> None:
        if not _NAME.match(self.name):
            raise InterpretationError(f"nombre de intérprete inválido: {self.name!r}")
        if not _VERSION.match(self.version):
            raise InterpretationError(f"versión de intérprete inválida: {self.version!r}")
        if self.model is not None and not (0 < len(self.model) <= _MAX_MODEL):
            raise InterpretationError("model tiene que ser null o un texto de 1 a 200 caracteres")
        if self.prompt_sha256 is not None and not _SHA256.match(self.prompt_sha256):
            raise InterpretationError("prompt_sha256 tiene que ser null o un SHA-256 en hex")


@dataclass(frozen=True)
class Impact:
    instrument_id: str
    sentiment: Decimal
    relevance: Decimal


@dataclass(frozen=True)
class Reading:
    """Lo que devuelve un intérprete para una noticia, antes de validarlo."""

    summary_es: str
    impacts: tuple[Impact, ...]


class Interpreter(Protocol):
    """Un intérprete de noticias. Tiene que ser determinista para la misma noticia y versión."""

    @property
    def info(self) -> InterpreterInfo: ...

    def read(self, item: NewsItem, instruments: Sequence[str]) -> Reading: ...


@dataclass(frozen=True)
class Interpretation:
    source: str
    item_id: str
    published_at: datetime
    interpreted_at: datetime
    interpreter: InterpreterInfo
    summary_es: str
    impacts: tuple[Impact, ...]

    @property
    def interpretation_id(self) -> str:
        """Id idempotente: misma noticia y mismo intérprete → mismo id."""
        key = "\x1f".join(
            (self.source, self.item_id, self.interpreter.name, self.interpreter.version)
        )
        return hashlib.sha256(key.encode()).hexdigest()

    def to_contract(self) -> dict[str, object]:
        """Documento `contracts/news_interpretation.v1`."""
        info = self.interpreter
        return {
            "interpretation_id": self.interpretation_id,
            "source": self.source,
            "item_id": self.item_id,
            "published_at": _utc_z(self.published_at),
            "interpreted_at": _utc_z(self.interpreted_at),
            "interpreter": {
                "name": info.name,
                "version": info.version,
                "model": info.model,
                "prompt_sha256": info.prompt_sha256,
            },
            "summary_es": self.summary_es,
            "impacts": [
                {
                    "instrument_id": i.instrument_id,
                    "sentiment": format(i.sentiment, "f"),
                    "relevance": format(i.relevance, "f"),
                }
                for i in self.impacts
            ],
        }


def instrument_ids(text: str) -> tuple[str, ...]:
    """Ids de instrumentos de `config/instruments.toml`, en orden. Solo los lee, no los valida."""
    raw = tomllib.loads(text).get("instruments", {})
    ids = tuple(sorted(raw))
    if not ids:
        raise InterpretationError("config/instruments.toml no tiene instrumentos")
    for i in ids:
        if not _INSTRUMENT.match(i):
            raise InterpretationError(f"id de instrumento inválido: {i!r}")
    return ids


def interpret(
    item: NewsItem, interpreter: Interpreter, instruments: Sequence[str], now: datetime
) -> Interpretation:
    """Interpreta `item` con `interpreter` y valida el resultado. `now` es la hora actual."""
    if now.tzinfo is None:
        raise InterpretationError("now necesita zona horaria")
    if now < item.published_at:
        raise InterpretationError(
            f"{item.source}/{item.item_id}: no se interpreta antes de su publicación"
        )
    allowed = set(instruments)
    reading = interpreter.read(item, tuple(instruments))
    summary = reading.summary_es
    if not isinstance(summary, str) or not summary.strip() or len(summary) > _MAX_SUMMARY:
        raise InterpretationError("summary_es tiene que ser un texto de 1 a 1000 caracteres")
    if len(reading.impacts) > _MAX_IMPACTS:
        raise InterpretationError(f"más de {_MAX_IMPACTS} instrumentos afectados")
    seen: set[str] = set()
    for imp in reading.impacts:
        if imp.instrument_id not in allowed:
            raise InterpretationError(f"instrumento no configurado: {imp.instrument_id!r}")
        if imp.instrument_id in seen:
            raise InterpretationError(f"instrumento repetido: {imp.instrument_id}")
        seen.add(imp.instrument_id)
        _check_decimal("sentiment", imp.sentiment, Decimal(-1), Decimal(1))
        _check_decimal("relevance", imp.relevance, Decimal(0), Decimal(1))
    return Interpretation(
        source=item.source,
        item_id=item.item_id,
        published_at=item.published_at,
        interpreted_at=now.astimezone(UTC),
        interpreter=interpreter.info,
        summary_es=summary,
        impacts=tuple(reading.impacts),
    )


def _check_decimal(name: str, value: object, low: Decimal, high: Decimal) -> None:
    if not isinstance(value, Decimal) or not value.is_finite():
        raise InterpretationError(f"{name} tiene que ser un Decimal finito, no {value!r}")
    exponent = value.as_tuple().exponent
    if isinstance(exponent, int) and exponent < -_MAX_DECIMALS:
        raise InterpretationError(f"{name} {value} tiene más de {_MAX_DECIMALS} decimales")
    if not low <= value <= high:
        raise InterpretationError(f"{name} {value} fuera de [{low}, {high}]")


def _utc_z(t: datetime) -> str:
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
