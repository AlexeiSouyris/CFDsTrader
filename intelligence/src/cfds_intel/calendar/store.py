"""Guardado del calendario económico en PostgreSQL (#239): tabla `economic_events`, de solo
agregar (`infra/migrations/0022_economic_events.sql`).

- Clave natural: fuente, evento, país, instrumento, impacto y hora. Volver a leer el mismo
  evento no lo duplica y **conserva el `published_at` de la primera vez** que se vio.
- Si la fuente cambia la hora o el impacto, la clave es otra: es un evento **nuevo** con su
  propio `published_at`. El anterior no se borra ni se pisa (ADR 0006 §3, ADR 0010 §5).
- Todo o nada: una transacción por lote.

Instantáneas de `forecast` y `previous` (ADR 0018 §1, #696): tabla `calendar_estimates`, de solo
agregar (`infra/migrations/0051_calendar_surprise.sql`). Hay fila nueva **solo si el par cambió**
respecto de la última instantánea guardada del mismo evento (de A a B y de vuelta a A son tres
filas). La primera instantánea de un evento sin ningún valor no se guarda: no dice nada.
"""

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime

import psycopg

from cfds_intel.calendar.estimates import CalendarEstimate
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


# Lo toma cada guardado de instantáneas: dos corridas a la vez no comparan contra la misma
# "última" y guardan dos veces el mismo cambio.
_ESTIMATES_LOCK = 0x0CF0_0018


def insert_estimates(
    conn: psycopg.Connection, estimates: Sequence[CalendarEstimate]
) -> InsertReport:
    """Guarda las instantáneas que cambiaron, en una sola transacción."""
    inserted = 0
    with conn.transaction(), conn.cursor() as cur:
        cur.execute("SELECT pg_advisory_xact_lock(%s)", (_ESTIMATES_LOCK,))
        for e in estimates:
            last = cur.execute(
                """
                SELECT forecast_raw, previous_raw FROM calendar_estimates
                WHERE source = %s AND event = %s AND country = %s AND scheduled_at = %s
                  AND seen_at <= %s
                ORDER BY seen_at DESC LIMIT 1
                """,
                (*e.event_key(), e.seen_at),
            ).fetchone()
            if (last is None and e.pair() == (None, None)) or (
                last is not None and tuple(last) == e.pair()
            ):
                continue
            cur.execute(
                """
                INSERT INTO calendar_estimates
                    (source, event, country, scheduled_at, forecast_raw, previous_raw,
                     forecast_value, previous_value, unit, seen_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT ON CONSTRAINT calendar_estimates_key DO NOTHING
                """,
                (
                    e.source,
                    e.event,
                    e.country,
                    e.scheduled_at,
                    e.forecast_raw,
                    e.previous_raw,
                    e.forecast_value,
                    e.previous_value,
                    e.unit,
                    e.seen_at,
                ),
            )
            inserted += cur.rowcount
    return InsertReport(inserted, len(estimates) - inserted)


def estimates_of(
    conn: psycopg.Connection, source: str, event: str, country: str, scheduled_at: datetime
) -> list[CalendarEstimate]:
    """Todas las instantáneas guardadas de un evento, en orden de `seen_at`. Elegir cuál vale
    como consenso (la última antes de `scheduled_at`) es parte de la sorpresa: lo hace Rust."""
    rows = conn.execute(
        """
        SELECT source, event, country, scheduled_at, forecast_raw, previous_raw,
               forecast_value, previous_value, unit, seen_at
        FROM calendar_estimates
        WHERE source = %s AND event = %s AND country = %s AND scheduled_at = %s
        ORDER BY seen_at
        """,
        (source, event, country, scheduled_at),
    ).fetchall()
    return [CalendarEstimate(*r) for r in rows]
