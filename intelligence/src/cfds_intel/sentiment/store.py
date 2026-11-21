"""Guardado de observaciones de sentimiento en `sentiment_observations` (#113).

La tabla es de solo agregar (#27). Una observación ya guardada para la misma fuente, serie,
instrumento y momento, con el mismo valor, no se repite aunque cambie la hora de publicación
calculada. Con otro valor es una **corrección** de la fuente: se agrega como fila nueva,
publicada cuando se leyó.
"""

from collections.abc import Sequence
from dataclasses import dataclass, replace
from datetime import datetime

import psycopg

from cfds_intel.sentiment.cot import SOURCE, Observation


@dataclass(frozen=True)
class StoreReport:
    inserted: int
    unchanged: int
    corrections: int


def insert_observations(
    conn: psycopg.Connection, obs: Sequence[Observation], retrieved_at: datetime
) -> StoreReport:
    """Todo o nada: una transacción por lote."""
    inserted = unchanged = corrections = 0
    with conn.transaction(), conn.cursor() as cur:
        for o in obs:
            cur.execute(
                """
                SELECT value FROM sentiment_observations
                WHERE source = %s AND series = %s AND instrument_id = %s AND observed_for = %s
                ORDER BY published_at DESC LIMIT 1
                """,
                (SOURCE, o.series, o.instrument_id, o.observed_for),
            )
            last = cur.fetchone()
            if last is not None and last[0] == o.value:
                unchanged += 1
                continue
            row = o if last is None else replace(o, published_at=retrieved_at)
            cur.execute(
                """
                INSERT INTO sentiment_observations
                    (source, series, instrument_id, observed_for, published_at, value)
                VALUES (%s, %s, %s, %s, %s, %s)
                """,
                (
                    SOURCE,
                    row.series,
                    row.instrument_id,
                    row.observed_for,
                    row.published_at,
                    row.value,
                ),
            )
            if last is None:
                inserted += 1
            else:
                corrections += 1
    return StoreReport(inserted, unchanged, corrections)
