"""Guardado del calendario económico en PostgreSQL (#239): tabla `economic_events`, de solo
agregar (`infra/migrations/0022_economic_events.sql`).

- Clave natural: fuente, evento, país, instrumento, impacto y hora. Volver a leer el mismo
  evento no lo duplica y **conserva el `published_at` de la primera vez** que se vio.
- Si la fuente cambia la hora o el impacto, la clave es otra: es un evento **nuevo** con su
  propio `published_at`. El anterior no se borra ni se pisa (ADR 0006 §3, ADR 0010 §5).
- Todo o nada: una transacción por lote.
"""

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime

import psycopg

from cfds_intel.calendar.feed import EconomicEvent


@dataclass(frozen=True)
class InsertReport:
    inserted: int
    unchanged: int


def insert_events(conn: psycopg.Connection, events: Sequence[EconomicEvent]) -> InsertReport:
    """Guarda `events` en una sola transacción."""
    inserted = 0
    with conn.transaction(), conn.cursor() as cur:
        for e in events:
            cur.execute(
                """
                INSERT INTO economic_events
                    (source, event, country, instrument_id, impact, scheduled_at, published_at)
                VALUES (%s, %s, %s, NULL, %s, %s, %s)
                ON CONFLICT ON CONSTRAINT economic_events_key DO NOTHING
                """,
                (e.source, e.event, e.country, e.impact, e.scheduled_at, e.published_at),
            )
            inserted += cur.rowcount
    return InsertReport(inserted, len(events) - inserted)


def events_known_at(
    conn: psycopg.Connection,
    source: str,
    as_of: datetime,
    scheduled_from: datetime,
    scheduled_to: datetime,
) -> list[EconomicEvent]:
    """Eventos de país con hora en `[scheduled_from, scheduled_to)` **que ya se conocían en
    `as_of`** (`published_at <= as_of`), en orden de hora. Así nada mira el futuro."""
    rows = conn.execute(
        """
        SELECT source, event, country, impact, scheduled_at, published_at
        FROM economic_events
        WHERE source = %s AND country IS NOT NULL AND published_at <= %s
          AND scheduled_at >= %s AND scheduled_at < %s
        ORDER BY scheduled_at, event, country, impact
        """,
        (source, as_of, scheduled_from, scheduled_to),
    ).fetchall()
    return [EconomicEvent(*r) for r in rows]
