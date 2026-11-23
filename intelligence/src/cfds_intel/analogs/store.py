"""Guardado de los informes de análogos en `analog_reports` (#294, migración 0029).

La tabla es de solo agregar. Si el último informe guardado del mismo instrumento, fuente y día
es igual (sin contar `computed_at`), no se repite; si cambió (por ejemplo, la config o los datos
de los vecinos), se agrega como un informe nuevo, con su propia hora de cálculo.
"""

from typing import Any

import psycopg
from psycopg.types.json import Jsonb


def _without_time(doc: dict[str, Any]) -> dict[str, Any]:
    return {k: v for k, v in doc.items() if k != "computed_at"}


def insert_report(conn: psycopg.Connection, doc: dict[str, Any]) -> bool:
    """`True` si se guardó; `False` si el último informe de ese día ya era igual."""
    with conn.transaction(), conn.cursor() as cur:
        cur.execute(
            """
            SELECT document FROM analog_reports
            WHERE instrument_id = %s AND source = %s AND window_end_day = %s
            ORDER BY id DESC LIMIT 1
            """,
            (doc["instrument_id"], doc["source"], doc["window_end_day"]),
        )
        last = cur.fetchone()
        if last is not None and _without_time(last[0]) == _without_time(doc):
            return False
        cur.execute(
            """
            INSERT INTO analog_reports
              (instrument_id, source, window_end_day, as_of, computed_at, document)
            VALUES (%s, %s, %s, %s, %s, %s)
            """,
            (
                doc["instrument_id"],
                doc["source"],
                doc["window_end_day"],
                doc["as_of"],
                doc["computed_at"],
                Jsonb(doc),
            ),
        )
    return True
