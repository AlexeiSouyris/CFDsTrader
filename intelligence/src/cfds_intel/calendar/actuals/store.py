"""Guardado del dato real en `release_actuals` (ADR 0018, #697), de solo agregar
(`infra/migrations/0051_calendar_surprise.sql`).

- Clave natural `(source, series_id, reference_period, vintage, value)`: otra lectura con el mismo
  valor no agrega nada; un valor revisado distinto es otra fila.
- Un solo `first` por serie y mes (índice único parcial): si ya hay uno, el nuevo no entra.
- Todo o nada: una transacción por lote.
"""

from collections.abc import Sequence
from dataclasses import dataclass

import psycopg

from cfds_intel.calendar.actuals import SOURCE, ReleaseActual, ReleaseRow


@dataclass(frozen=True)
class InsertReport:
    inserted: int
    unchanged: int


def insert_actuals(conn: psycopg.Connection, rows: Sequence[ReleaseActual]) -> InsertReport:
    inserted = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in rows:
            cur.execute(
                """
                INSERT INTO release_actuals
                    (source, series_id, reference_period, value, unit, vintage,
                     released_at, retrieved_at, source_url)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT DO NOTHING
                """,
                (
                    r.source,
                    r.series_id,
                    r.reference_period,
                    r.value,
                    r.unit,
                    r.vintage,
                    r.released_at,
                    r.retrieved_at,
                    r.source_url,
                ),
            )
            inserted += cur.rowcount
    return InsertReport(inserted, len(rows) - inserted)


def release_rows(conn: psycopg.Connection) -> list[ReleaseRow]:
    """Las filas del BLS en `official_releases` (CPI y empleo); el filtro por `as_of` lo hace
    `effective_releases`."""
    rows = conn.execute(
        """
        SELECT series, reference_period, status, scheduled_at, published_at
        FROM official_releases WHERE source = 'bls'
        """
    ).fetchall()
    return [ReleaseRow(*r) for r in rows]


def periods_with_first(conn: psycopg.Connection, series_ids: Sequence[str]) -> set[tuple[str, str]]:
    """(serie, mes) que ya tienen su primer dato."""
    rows = conn.execute(
        """
        SELECT series_id, reference_period FROM release_actuals
        WHERE source = %s AND vintage = 'first' AND series_id = ANY(%s)
        """,
        (SOURCE, list(series_ids)),
    ).fetchall()
    return {(r[0], r[1]) for r in rows}
