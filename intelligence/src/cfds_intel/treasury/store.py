"""Guardado de la curva del Tesoro en `treasury_yields` (ADR 0012).

La tabla es de solo agregar. Un valor ya guardado para el mismo plazo y fecha, con el mismo
rendimiento, no se repite aunque cambie la hora de publicación calculada. Con otro rendimiento es
una **corrección** del Tesoro: se agrega como fila nueva, publicada cuando se leyó.
"""

from collections.abc import Sequence
from dataclasses import dataclass, replace
from datetime import datetime

import psycopg

from cfds_intel.treasury.ust import TreasuryYield


@dataclass(frozen=True)
class StoreReport:
    inserted: int
    unchanged: int
    corrections: int


def insert_yields(
    conn: psycopg.Connection, rows: Sequence[TreasuryYield], retrieved_at: datetime
) -> StoreReport:
    """Todo o nada: una transacción por lote."""
    inserted = unchanged = corrections = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in rows:
            cur.execute(
                """
                SELECT yield_pct FROM treasury_yields
                WHERE tenor = %s AND ref_date = %s
                ORDER BY published_at DESC LIMIT 1
                """,
                (r.tenor, r.ref_date),
            )
            last = cur.fetchone()
            if last is not None and last[0] == r.yield_pct:
                unchanged += 1
                continue
            row = r if last is None else replace(r, published_at=retrieved_at)
            cur.execute(
                """
                INSERT INTO treasury_yields (tenor, ref_date, published_at, yield_pct)
                VALUES (%s, %s, %s, %s)
                """,
                (row.tenor, row.ref_date, row.published_at, row.yield_pct),
            )
            if last is None:
                inserted += 1
            else:
                corrections += 1
    return StoreReport(inserted, unchanged, corrections)
