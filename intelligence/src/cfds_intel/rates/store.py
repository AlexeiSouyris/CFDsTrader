"""Guardado de tipos de referencia en `reference_rates` (#79).

La tabla es de solo agregar. Un valor ya guardado para la misma serie y fecha, con la misma tasa,
no se repite aunque cambie la hora de publicación calculada. Con otra tasa es una **corrección**
de la fuente: se agrega como fila nueva, publicada cuando se leyó.
"""

from collections.abc import Sequence
from dataclasses import dataclass, replace
from datetime import datetime

import psycopg

from cfds_intel.rates.ecb import Rate


@dataclass(frozen=True)
class StoreReport:
    inserted: int
    unchanged: int
    corrections: int


def insert_rates(
    conn: psycopg.Connection, rates: Sequence[Rate], retrieved_at: datetime
) -> StoreReport:
    """Todo o nada: una transacción por lote."""
    inserted = unchanged = corrections = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in rates:
            cur.execute(
                """
                SELECT rate_pct FROM reference_rates
                WHERE series = %s AND ref_date = %s
                ORDER BY published_at DESC LIMIT 1
                """,
                (r.series, r.ref_date),
            )
            last = cur.fetchone()
            if last is not None and last[0] == r.rate_pct:
                unchanged += 1
                continue
            row = r if last is None else replace(r, published_at=retrieved_at)
            cur.execute(
                """
                INSERT INTO reference_rates (series, ref_date, published_at, rate_pct)
                VALUES (%s, %s, %s, %s)
                """,
                (row.series, row.ref_date, row.published_at, row.rate_pct),
            )
            if last is None:
                inserted += 1
            else:
                corrections += 1
    return StoreReport(inserted, unchanged, corrections)
