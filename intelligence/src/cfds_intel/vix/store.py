"""Guardado del VIX diario en `vix_daily` (ADR 0012 §2).

La tabla es de solo agregar. Un día ya guardado con los mismos valores no se repite aunque cambie
la hora de publicación calculada. Con otros valores es una **corrección** de Cboe: se agrega como
fila nueva, publicada cuando se leyó.
"""

from collections.abc import Sequence
from dataclasses import dataclass, replace
from datetime import datetime

import psycopg

from cfds_intel.vix.cboe import VixDay


@dataclass(frozen=True)
class StoreReport:
    inserted: int
    unchanged: int
    corrections: int


def insert_days(
    conn: psycopg.Connection, days: Sequence[VixDay], retrieved_at: datetime
) -> StoreReport:
    """Todo o nada: una transacción por lote."""
    inserted = unchanged = corrections = 0
    with conn.transaction(), conn.cursor() as cur:
        for d in days:
            cur.execute(
                """
                SELECT open, high, low, close FROM vix_daily
                WHERE ref_date = %s ORDER BY published_at DESC LIMIT 1
                """,
                (d.ref_date,),
            )
            last = cur.fetchone()
            if last is not None and tuple(last) == (d.open, d.high, d.low, d.close):
                unchanged += 1
                continue
            row = d if last is None else replace(d, published_at=retrieved_at)
            cur.execute(
                """
                INSERT INTO vix_daily (ref_date, published_at, open, high, low, close)
                VALUES (%s, %s, %s, %s, %s, %s)
                """,
                (row.ref_date, row.published_at, row.open, row.high, row.low, row.close),
            )
            if last is None:
                inserted += 1
            else:
                corrections += 1
    return StoreReport(inserted, unchanged, corrections)
