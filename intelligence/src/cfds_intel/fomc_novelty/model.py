"""Configuración y fila de la novedad de los comunicados del FOMC (#668).

La fila sigue `contracts/fomc_statement_novelty.v1`. La configuración sale de
`config/fomc_novelty.toml` y se valida al cargarla: si falta algo, falla.
"""

import tomllib
import urllib.parse
from dataclasses import dataclass
from datetime import UTC, date, datetime
from decimal import Decimal
from typing import Any

from cfds_intel.calendar.official.model import utc_z

METHODS = ("word_diff", "embedding_cosine")
STATUSES = ("scheduled", "unscheduled")
_LOCAL_HOSTS = ("127.0.0.1", "localhost", "::1")


class NoveltyError(ValueError):
    """Algo no cumple las reglas: la corrida frena sin guardar nada."""


@dataclass(frozen=True)
class ClientConfig:
    user_agent: str
    timeout_secs: int
    pause_secs: float
    release_url_prefix: str
    html_dir: str


@dataclass(frozen=True)
class EmbeddingConfig:
    url: str
    model: str
    prefix: str
    num_ctx: int
    timeout_secs: int
    method_version: str


@dataclass(frozen=True)
class NoveltyConfig:
    client: ClientConfig
    first_day: date
    last_day: date
    statuses: tuple[str, ...]
    end_marker: str
    word_diff_version: str
    embedding: EmbeddingConfig


def _table(doc: dict[str, Any], name: str) -> dict[str, Any]:
    t = doc.get(name)
    if not isinstance(t, dict):
        raise NoveltyError(f"falta la sección [{name}]")
    return t


def _get(t: dict[str, Any], key: str, kind: type, where: str) -> Any:
    v = t.get(key)
    if kind is float and isinstance(v, int) and not isinstance(v, bool):
        v = float(v)
    if not isinstance(v, kind) or isinstance(v, bool) or (isinstance(v, str) and not v.strip()):
        raise NoveltyError(f"[{where}] {key} falta o no es {kind.__name__}")
    return v


def load_config(text: str) -> NoveltyConfig:
    doc = tomllib.loads(text)
    c, s, w, e = (_table(doc, n) for n in ("client", "statements", "word_diff", "embedding"))
    client = ClientConfig(
        user_agent=_get(c, "user_agent", str, "client"),
        timeout_secs=_get(c, "timeout_secs", int, "client"),
        pause_secs=_get(c, "pause_secs", float, "client"),
        release_url_prefix=_get(c, "release_url_prefix", str, "client"),
        html_dir=_get(c, "html_dir", str, "client"),
    )
    if client.pause_secs < 1:
        raise NoveltyError("[client] pause_secs tiene que ser al menos 1 s (ADR 0016 §1.2)")
    if not client.release_url_prefix.startswith("https://www.federalreserve.gov/"):
        raise NoveltyError("[client] release_url_prefix tiene que ser de federalreserve.gov")
    try:
        first = date.fromisoformat(_get(s, "first_day", str, "statements"))
        last = date.fromisoformat(_get(s, "last_day", str, "statements"))
    except ValueError as err:
        raise NoveltyError(f"[statements] fecha inválida: {err}") from err
    if first > last:
        raise NoveltyError("[statements] first_day es posterior a last_day")
    statuses = s.get("statuses")
    if not isinstance(statuses, list) or not statuses or any(x not in STATUSES for x in statuses):
        raise NoveltyError(f"[statements] statuses tiene que ser una lista de {STATUSES}")
    url = _get(e, "url", str, "embedding").rstrip("/")
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "http" or parsed.hostname not in _LOCAL_HOSTS:
        raise NoveltyError(f"[embedding] url tiene que ser http://127.0.0.1 o localhost: {url!r}")
    embedding = EmbeddingConfig(
        url=url,
        model=_get(e, "model", str, "embedding"),
        prefix=str(e.get("prefix", "")),
        num_ctx=_get(e, "num_ctx", int, "embedding"),
        timeout_secs=_get(e, "timeout_secs", int, "embedding"),
        method_version=_get(e, "method_version", str, "embedding"),
    )
    return NoveltyConfig(
        client=client,
        first_day=first,
        last_day=last,
        statuses=tuple(statuses),
        end_marker=_get(s, "end_marker", str, "statements"),
        word_diff_version=_get(w, "method_version", str, "word_diff"),
        embedding=embedding,
    )


@dataclass(frozen=True)
class Statement:
    """Un comunicado de `official_releases` (solo lo que hace falta acá)."""

    reference_period: str
    status: str
    scheduled_at: datetime
    source_url: str


@dataclass(frozen=True)
class MeasuredText:
    """El texto de un comunicado tal como se mide."""

    statement: Statement
    text: str
    sha256: str
    word_count: int


@dataclass(frozen=True)
class NoveltyRow:
    reference_period: str
    status: str
    statement_url: str
    available_at: datetime
    previous_reference_period: str
    previous_statement_url: str
    method: str
    method_version: str
    model: str | None
    model_digest: str | None
    value: Decimal
    text_sha256: str
    previous_text_sha256: str
    word_count: int
    previous_word_count: int
    computed_at: datetime

    def __post_init__(self) -> None:
        where = f"{self.reference_period} {self.method}"
        if self.method not in METHODS:
            raise NoveltyError(f"{where}: método desconocido")
        if self.status not in STATUSES:
            raise NoveltyError(f"{where}: status inválido {self.status!r}")
        if self.previous_reference_period >= self.reference_period:
            raise NoveltyError(f"{where}: el comunicado anterior no es anterior")
        with_model = self.model is not None and self.model_digest is not None
        without_model = self.model is None and self.model_digest is None
        if (self.method == "word_diff") != without_model or not (with_model or without_model):
            raise NoveltyError(f"{where}: modelo y digest solo en embedding_cosine")
        top = Decimal(1) if self.method == "word_diff" else Decimal(2)
        if not Decimal(0) <= self.value <= top:
            raise NoveltyError(f"{where}: valor fuera de rango {self.value}")
        for t in (self.available_at, self.computed_at):
            if t.tzinfo is None:
                raise NoveltyError(f"{where}: hora sin zona horaria")

    def to_contract(self) -> dict[str, object]:
        return {
            "reference_period": self.reference_period,
            "status": self.status,
            "statement_url": self.statement_url,
            "available_at": utc_z(self.available_at),
            "previous_reference_period": self.previous_reference_period,
            "previous_statement_url": self.previous_statement_url,
            "method": self.method,
            "method_version": self.method_version,
            "model": self.model,
            "model_digest": self.model_digest,
            "value": float(self.value),
            "text_sha256": self.text_sha256,
            "previous_text_sha256": self.previous_text_sha256,
            "word_count": self.word_count,
            "previous_word_count": self.previous_word_count,
            "computed_at": utc_z(self.computed_at.astimezone(UTC)),
        }
