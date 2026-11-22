"""Guardado y publicación de interpretaciones de noticias (#186, #129, ADR 0008).

- Tabla `news_interpretations` (de solo agregar) con las columnas de `news_interpretation.v1`.
- Stream de Redis `news_interpretations.v1`: una entrada por interpretación nueva, con el campo
  `data` = JSON del contrato y `interpretation_id` aparte para deduplicar.

Idempotencia: el id es el SHA-256 de la noticia y del intérprete (nombre y versión). Guardar otra
vez una noticia ya interpretada con el mismo intérprete no guarda ni publica nada, y se conserva la
primera interpretación (con aceleración Metal, el resumen de Ollama puede variar entre corridas,
ADR 0008 §4). Si la guardada tiene otro modelo u otro prompt con la misma versión, es un error: el
intérprete tenía que cambiar de versión.

Orden: se publica **dentro** de la transacción, antes del commit. Si Redis falla, no se guarda
nada y la próxima corrida lo reintenta. Si el commit falla después de publicar, la entrada queda
en el stream sin fila: el consumidor deduplica por `interpretation_id` y la tabla es la fuente de
verdad (entrega "al menos una vez").
"""

import json
from typing import Protocol

import psycopg
import redis
from psycopg.types.json import Jsonb

from cfds_intel.news.feeds import NewsItem
from cfds_intel.news.interpret import Interpretation, InterpreterInfo

STREAM = "news_interpretations.v1"


class InterpretationConflictError(ValueError):
    """Ya hay otra interpretación con el mismo id y otro modelo o prompt."""


class Publisher(Protocol):
    """Publica un documento `news_interpretation.v1`. Falla con una excepción, nunca en silencio."""

    def publish(self, doc: dict[str, object]) -> None: ...


class RedisPublisher:
    """`Publisher` sobre un stream de Redis (`XADD` con los campos `data` e `interpretation_id`)."""

    def __init__(self, url: str, stream: str = STREAM) -> None:
        self._redis = redis.Redis.from_url(url)
        self.stream = stream

    def ping(self) -> None:
        """Falla si Redis no responde: se comprueba antes de interpretar nada."""
        self._redis.ping()

    def publish(self, doc: dict[str, object]) -> None:
        data = json.dumps(doc, ensure_ascii=False, separators=(",", ":"))
        self._redis.xadd(
            self.stream, {"data": data, "interpretation_id": str(doc["interpretation_id"])}
        )


def pending(
    conn: psycopg.Connection, info: InterpreterInfo, limit: int | None = None
) -> list[NewsItem]:
    """Noticias de `news_items` sin interpretación de este intérprete (nombre y versión), de la
    más vieja a la más nueva."""
    rows = conn.execute(
        """
        SELECT n.source, n.feed, n.item_id, n.title, n.url, n.published_at, n.retrieved_at
        FROM news_items n
        WHERE NOT EXISTS (
            SELECT 1 FROM news_interpretations i
            WHERE i.source = n.source AND i.item_id = n.item_id
              AND i.interpreter_name = %s AND i.interpreter_version = %s
        )
        ORDER BY n.published_at, n.source, n.item_id
        LIMIT %s
        """,
        (info.name, info.version, limit),
    ).fetchall()
    return [NewsItem(*row) for row in rows]


def save(conn: psycopg.Connection, interp: Interpretation, publisher: Publisher) -> bool:
    """Guarda y publica `interp` en una transacción. `True` si es nueva, `False` si ya estaba."""
    doc = interp.to_contract()
    info = interp.interpreter
    with conn.transaction(), conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO news_interpretations
                (interpretation_id, source, item_id, published_at, interpreted_at,
                 interpreter_name, interpreter_version, interpreter_model,
                 interpreter_prompt_sha256, summary_es, impacts)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT (interpretation_id) DO NOTHING
            """,
            (
                interp.interpretation_id,
                interp.source,
                interp.item_id,
                interp.published_at,
                interp.interpreted_at,
                info.name,
                info.version,
                info.model,
                info.prompt_sha256,
                interp.summary_es,
                Jsonb(doc["impacts"]),
            ),
        )
        if cur.rowcount == 1:
            publisher.publish(doc)
            return True
        cur.execute(
            "SELECT interpreter_model, interpreter_prompt_sha256 FROM news_interpretations"
            " WHERE interpretation_id = %s",
            (interp.interpretation_id,),
        )
        row = cur.fetchone()
    if row is None or tuple(row) != (info.model, info.prompt_sha256):
        raise InterpretationConflictError(
            f"{interp.source}/{interp.item_id}: ya está interpretada por {info.name} "
            f"{info.version} con otro modelo o prompt; cambiá la versión del intérprete"
        )
    return False
