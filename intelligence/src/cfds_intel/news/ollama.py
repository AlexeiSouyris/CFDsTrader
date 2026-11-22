"""Intérprete de noticias con un modelo abierto local en Ollama (ADR 0008, #129).

- Solo habla con Ollama en `localhost`: ninguna noticia sale de la máquina.
- El modelo queda fijado por nombre y `digest` (`GET /api/tags`). Si el instalado no coincide, o
  todavía no se eligió un modelo (`config/news_interpreter.toml`), no se interpreta nada.
- Temperatura 0 y semilla fija. Modelo, digest, versión y SHA-256 del prompt quedan en cada
  evento (`InterpreterInfo`).
- Lee el **texto oficial** del comunicado (`fulltext.py`), no solo el título, y nunca lo guarda.
- La respuesta se convierte en un `Reading`; las reglas (rangos, instrumentos, publicación) las
  aplica `interpret()` (#122). Un número que no viene como texto decimal es un error: no se
  redondea nada.

API usada, de la documentación oficial (github.com/ollama/ollama/blob/main/docs/api.md):
`POST /api/chat` (`stream: false`, `format` con JSON Schema, `options.temperature`,
`options.seed`, `options.num_ctx`, `keep_alive`, `think`), `POST /api/generate` sin prompt (cargar
el modelo), `GET /api/tags` (digest) y `GET /api/ps` (memoria del modelo cargado).
"""

import hashlib
import json
import re
import tomllib
import urllib.parse
import urllib.request
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

from cfds_intel.news.feeds import NewsItem
from cfds_intel.news.fulltext import Layout, fetch_text, follow_patterns, layout_from
from cfds_intel.news.interpret import Impact, InterpretationError, InterpreterInfo, Reading

PROMPT_VERSION = "2"
PROMPT = (Path(__file__).parent / "prompts" / f"interpret_v{PROMPT_VERSION}.txt").read_text()
PROMPT_SHA256 = hashlib.sha256(PROMPT.encode()).hexdigest()
_LOCAL_HOSTS = {"127.0.0.1", "localhost"}

# Forma de la respuesta que se le pide al modelo (`format` de /api/chat).
RESPONSE_SCHEMA: dict[str, Any] = {
    "type": "object",
    "additionalProperties": False,
    "required": ["summary_es", "impacts"],
    "properties": {
        "summary_es": {"type": "string", "minLength": 1, "maxLength": 1000},
        "impacts": {
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": False,
                "required": ["instrument_id", "sentiment", "relevance"],
                "properties": {
                    "instrument_id": {"type": "string"},
                    "sentiment": {"type": "string", "pattern": r"^-?[01](\.\d{1,4})?$"},
                    "relevance": {"type": "string", "pattern": r"^[01](\.\d{1,4})?$"},
                },
            },
        },
    },
}


class OllamaError(InterpretationError):
    """Ollama no está, respondió algo inesperado o el modelo no es el fijado."""


@dataclass(frozen=True)
class Selection:
    min_relevant_pairs: int
    # Pares relevantes con signo "+" y con signo "-" que hacen falta, de cada uno.
    min_signed_pairs_each: int
    # El modelo marca un par como relevante si su relevancia es ≥ este valor.
    relevance_threshold: Decimal
    # Acierto balanceado mínimo de la detección de relevancia, en %.
    min_relevance_accuracy_pct: Decimal
    min_valid_pct: Decimal
    max_memory_gb: Decimal
    max_seconds_p95: Decimal
    tie_points: Decimal


@dataclass(frozen=True)
class OllamaConfig:
    url: str
    timeout_secs: int
    temperature: int
    seed: int
    # Ventana de contexto fija (tokens): sin ella, Ollama reserva la máxima del modelo.
    num_ctx: int
    # Segundos que el modelo queda cargado después de cada pedido. 0 = se descarga y la
    # próxima noticia se procesa desde cero (sin reusar cálculos de la anterior).
    keep_alive: int
    think: bool
    model: str
    digest: str
    selection: Selection
    # Texto oficial de cada noticia (fulltext.py): sitios permitidos, recorte, cliente y espera.
    text_hosts: frozenset[str]
    text_max_chars: int
    text_user_agent: str
    text_timeout_secs: int
    # Documentos que el comunicado solo enlaza (actas y proyecciones del FOMC, #168).
    text_follow: tuple[re.Pattern[str], ...]
    # Qué parte del texto se lee cuando no entra entero (#190).
    text_layout: Layout
    # SHA-256 de todo lo de `[text]` que cambia el texto que lee el modelo: entra en la versión.
    text_sha256: str


def load_config(text: str) -> OllamaConfig:
    """Lee `config/news_interpreter.toml` y lo valida."""
    raw = tomllib.loads(text)
    o, g, m, s, t = (raw.get(k, {}) for k in ("ollama", "generation", "model", "selection", "text"))
    url = o.get("url")
    parsed = urllib.parse.urlparse(url) if isinstance(url, str) else None
    if parsed is None or parsed.scheme != "http" or parsed.hostname not in _LOCAL_HOSTS:
        raise OllamaError(f"ollama.url tiene que ser http://127.0.0.1 o localhost: {url!r}")

    def int_at_least(table: dict[str, Any], key: str, low: int) -> int:
        v = table.get(key)
        if not isinstance(v, int) or isinstance(v, bool) or v < low:
            raise OllamaError(f"{key} tiene que ser un entero ≥ {low}")
        return v

    def string_field(table: dict[str, Any], key: str) -> str:
        v = table.get(key, "")
        if not isinstance(v, str):
            raise OllamaError(f"model.{key} tiene que ser un texto")
        return v

    if not isinstance(g.get("think"), bool):
        raise OllamaError("generation.think tiene que ser true o false")
    pct = Decimal(int_at_least(s, "min_valid_pct", 1))
    rel_pct = Decimal(int_at_least(s, "min_relevance_accuracy_pct", 1))
    if pct > 100 or rel_pct > 100:
        raise OllamaError("los porcentajes de selection no pueden pasar de 100")
    try:
        threshold = Decimal(str(s.get("relevance_threshold")))
    except ArithmeticError as e:
        raise OllamaError("selection.relevance_threshold tiene que ser un decimal") from e
    if not isinstance(s.get("relevance_threshold"), str) or not 0 < threshold <= 1:
        raise OllamaError('selection.relevance_threshold: texto decimal en (0, 1], como "0.5"')
    hosts = t.get("allowed_hosts")
    if (
        not isinstance(hosts, list)
        or not hosts
        or not all(isinstance(h, str) and re.fullmatch(r"[a-z0-9.-]+", h) for h in hosts)
    ):
        raise OllamaError("text.allowed_hosts tiene que ser una lista de dominios")
    agent = t.get("user_agent")
    if not isinstance(agent, str) or not agent.strip():
        raise OllamaError("text.user_agent tiene que ser un texto")
    return OllamaConfig(
        url=str(url).rstrip("/"),
        timeout_secs=int_at_least(o, "timeout_secs", 1),
        temperature=int_at_least(g, "temperature", 0),
        seed=int_at_least(g, "seed", 0),
        num_ctx=int_at_least(g, "num_ctx", 512),
        keep_alive=int_at_least(g, "keep_alive", 0),
        think=g["think"],
        model=string_field(m, "name"),
        digest=string_field(m, "digest"),
        selection=Selection(
            min_relevant_pairs=int_at_least(s, "min_relevant_pairs", 1),
            min_valid_pct=pct,
            max_memory_gb=Decimal(int_at_least(s, "max_memory_gb", 1)),
            max_seconds_p95=Decimal(int_at_least(s, "max_seconds_p95", 1)),
            tie_points=Decimal(int_at_least(s, "tie_points", 0)),
            min_signed_pairs_each=int_at_least(s, "min_signed_pairs_each", 1),
            relevance_threshold=threshold,
            min_relevance_accuracy_pct=rel_pct,
        ),
        text_hosts=frozenset(hosts),
        text_max_chars=int_at_least(t, "max_chars", 500),
        text_user_agent=agent,
        text_timeout_secs=int_at_least(t, "timeout_secs", 1),
        text_follow=follow_patterns(t),
        text_layout=layout_from(t, int_at_least(t, "max_chars", 500)),
        text_sha256=_text_sha256(t),
    )


def _text_sha256(t: dict[str, Any]) -> str:
    """Huella de lo que cambia el texto de entrada (no el cliente ni la espera)."""
    keys = (
        "allowed_hosts",
        "max_chars",
        "follow",
        "drop",
        "head_chars",
        "sections",
        "section_max_chars",
        "scan_max_chars",
    )
    canon = json.dumps({k: t.get(k) for k in keys}, sort_keys=True, ensure_ascii=False)
    return hashlib.sha256(canon.encode()).hexdigest()


def interpreter_version(text_sha256: str, digest: str) -> str:
    """Versión del intérprete (#190): prompt, texto de entrada y modelo. Si cambia cualquiera de
    los tres, cambia la versión y las noticias se vuelven a interpretar (id nuevo)."""
    model = re.sub(r"[^0-9A-Za-z]", "", digest.removeprefix("sha256:"))[:12] or "x"
    return f"p{PROMPT_VERSION}.t{text_sha256[:8]}.m{model}"


# (método, URL, cuerpo JSON o None, timeout) → respuesta JSON. Se reemplaza en los tests.
Transport = Callable[[str, str, dict[str, Any] | None, int], dict[str, Any]]


def local_http(method: str, url: str, body: dict[str, Any] | None, timeout: int) -> dict[str, Any]:
    """HTTP solo contra `localhost` (Ollama no usa TLS en la máquina)."""
    host = urllib.parse.urlparse(url).hostname
    if not url.startswith("http://") or host not in _LOCAL_HOSTS:
        raise OllamaError(f"solo http://localhost: {url!r}")
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(  # noqa: S310 (solo http://localhost, validado arriba)
        url, data=data, method=method, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:  # noqa: S310
        out: dict[str, Any] = json.loads(resp.read())
        return out


def installed_digest(cfg: OllamaConfig, model: str, transport: Transport = local_http) -> str:
    """Digest del modelo instalado según `GET /api/tags`."""
    tags = transport("GET", f"{cfg.url}/api/tags", None, cfg.timeout_secs)
    for m in tags.get("models", []):
        if model in (m.get("name"), m.get("model")):
            digest = m.get("digest")
            if isinstance(digest, str) and digest:
                return digest
    raise OllamaError(f"el modelo {model!r} no está instalado en Ollama")


def load_model(cfg: OllamaConfig, model: str, transport: Transport = local_http) -> None:
    """Carga `model` sin preguntarle nada (`POST /api/generate` sin prompt), con el mismo
    `num_ctx` que la interpretación, y lo deja cargado 60 s para poder medir su memoria."""
    transport(
        "POST",
        f"{cfg.url}/api/generate",
        {"model": model, "keep_alive": 60, "options": {"num_ctx": cfg.num_ctx}},
        cfg.timeout_secs,
    )


def loaded_memory_bytes(cfg: OllamaConfig, model: str, transport: Transport = local_http) -> int:
    """Memoria del modelo cargado según `GET /api/ps` (campo `size`)."""
    ps = transport("GET", f"{cfg.url}/api/ps", None, cfg.timeout_secs)
    for m in ps.get("models", []):
        if model in (m.get("name"), m.get("model")) and isinstance(m.get("size"), int):
            return int(m["size"])
    raise OllamaError(f"el modelo {model!r} no está cargado en memoria")


def _decimal_text(value: object, field: str) -> Decimal:
    if not isinstance(value, str):
        raise OllamaError(f"{field} tiene que venir como texto decimal, no {value!r}")
    try:
        return Decimal(value)
    except InvalidOperation as e:
        raise OllamaError(f"{field} no es un decimal: {value!r}") from e


class OllamaInterpreter:
    """`Interpreter` (#122) que le pregunta a un modelo local de Ollama."""

    def __init__(
        self,
        cfg: OllamaConfig,
        model: str | None = None,
        transport: Transport = local_http,
        text_for: Callable[[NewsItem], str] | None = None,
    ) -> None:
        """`model` vacío usa el elegido en la config; otro modelo, solo en la prueba de selección.
        El digest se lee de Ollama y, si hay uno fijado en la config, tiene que coincidir."""
        name = model or cfg.model
        if not name:
            raise OllamaError(
                "no hay modelo elegido (config/news_interpreter.toml, model.name): "
                "primero la prueba de selección del ADR 0008"
            )
        digest = installed_digest(cfg, name, transport)
        if model is None and cfg.digest and digest != cfg.digest:
            raise OllamaError(f"el digest instalado de {name} no es el fijado: {digest}")
        self._cfg, self._model, self._transport = cfg, name, transport
        # Texto oficial de cada noticia; en los tests se reemplaza (sin red).
        self._text_for = text_for or (
            lambda item: fetch_text(
                item,
                cfg.text_hosts,
                cfg.text_max_chars,
                cfg.text_user_agent,
                cfg.text_timeout_secs,
                follow=cfg.text_follow,
                layout=cfg.text_layout,
            )
        )
        self._info = InterpreterInfo(
            name="ollama",
            version=interpreter_version(cfg.text_sha256, digest),
            model=f"{name}@{digest}"[:200],
            prompt_sha256=PROMPT_SHA256,
        )

    @property
    def info(self) -> InterpreterInfo:
        return self._info

    def request_body(self, item: NewsItem, instruments: Sequence[str], text: str) -> dict[str, Any]:
        """El texto oficial va solo en el pedido al modelo local: no se guarda (ADR 0006)."""
        news = {
            "fuente": item.source,
            "titular": item.title,
            "texto_oficial": text,
            "publicado_utc": item.published_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "instrumentos_permitidos": list(instruments),
        }
        return {
            "model": self._model,
            "messages": [
                {"role": "system", "content": PROMPT},
                {"role": "user", "content": json.dumps(news, ensure_ascii=False)},
            ],
            "stream": False,
            "format": RESPONSE_SCHEMA,
            "think": self._cfg.think,
            "keep_alive": self._cfg.keep_alive,
            "options": {
                "temperature": self._cfg.temperature,
                "seed": self._cfg.seed,
                "num_ctx": self._cfg.num_ctx,
            },
        }

    def read(self, item: NewsItem, instruments: Sequence[str]) -> Reading:
        resp = self._transport(
            "POST",
            f"{self._cfg.url}/api/chat",
            self.request_body(item, instruments, self._text_for(item)),
            self._cfg.timeout_secs,
        )
        content = resp.get("message", {}).get("content")
        if not isinstance(content, str):
            raise OllamaError("la respuesta de Ollama no trae message.content")
        try:
            doc = json.loads(content)
        except json.JSONDecodeError as e:
            raise OllamaError("el modelo no respondió JSON") from e
        if not isinstance(doc, dict) or not isinstance(doc.get("impacts"), list):
            raise OllamaError("el JSON del modelo no tiene la forma pedida")
        impacts = []
        for x in doc["impacts"]:
            if not isinstance(x, dict) or not isinstance(x.get("instrument_id"), str):
                raise OllamaError(f"impacto mal formado: {x!r}")
            impacts.append(
                Impact(
                    x["instrument_id"],
                    _decimal_text(x.get("sentiment"), "sentiment"),
                    _decimal_text(x.get("relevance"), "relevance"),
                )
            )
        summary = doc.get("summary_es")
        return Reading(summary if isinstance(summary, str) else "", tuple(impacts))
