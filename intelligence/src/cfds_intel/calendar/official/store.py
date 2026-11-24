"""Guardado del calendario oficial en `official_releases` (ADR 0016, #520), de solo agregar
(`infra/migrations/0046_official_releases.sql`).

- Clave natural: fuente, serie, período, estado y hora. Otra corrida no agrega nada.
- Si una fila ya guardada con la misma clave difiere en algo (otro `published_at`, otra regla,
  otra URL), es un **error**: la tabla no se pisa y la carga no sigue en silencio.
- Todo o nada: una transacción por lote.
"""

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime

import psycopg

from cfds_intel.calendar.official.model import OfficialCalendarError, OfficialRelease

_COLUMNS = (
    "source, series, reference_period, status, scheduled_at, published_at, time_rule,"
    " with_press_conference, source_url"
)


@dataclass(frozen=True)
class InsertReport:
    inserted: int
    unchanged: int


def _row(r: OfficialRelease) -> tuple[object, ...]:
    return (
        r.source,
        r.series,
        r.reference_period,
        r.status,
        r.scheduled_at,
        r.published_at,
        r.time_rule,
        r.with_press_conference,
        r.source_url,
    )


def insert_releases(conn: psycopg.Connection, releases: Sequence[OfficialRelease]) -> InsertReport:
    """Guarda `releases` en una sola transacción."""
    inserted = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in releases:
            cur.execute(
                f"INSERT INTO official_releases ({_COLUMNS}) VALUES "  # noqa: S608 (columnas fijas)
                "(%s, %s, %s, %s, %s, %s, %s, %s, %s) "
                "ON CONFLICT ON CONSTRAINT official_releases_key DO NOTHING",
                _row(r),
            )
            if cur.rowcount == 1:
                inserted += 1
                continue
            stored = cur.execute(
                f"SELECT {_COLUMNS} FROM official_releases "  # noqa: S608 (columnas fijas)
                "WHERE source = %s AND series = %s AND reference_period = %s AND status = %s"
                " AND scheduled_at IS NOT DISTINCT FROM %s",
                r.key(),
            ).fetchone()
            if stored is None or OfficialRelease(*stored) != r:
                raise OfficialCalendarError(
                    f"{r.source}/{r.series} {r.reference_period}: ya está guardada con otros"
                    f" datos ({stored}); la tabla es de solo agregar y no se pisa"
                )
    return InsertReport(inserted, len(releases) - inserted)


def releases_known_at(
    conn: psycopg.Connection,
    series: str,
    as_of: datetime,
    scheduled_from: datetime,
    scheduled_to: datetime,
) -> list[OfficialRelease]:
    """Publicaciones de `series` con hora en `[scheduled_from, scheduled_to)` **que ya se
    conocían en `as_of`** (`published_at <= as_of`), en orden de hora. Así nada mira el futuro."""
    rows = conn.execute(
        f"SELECT {_COLUMNS} FROM official_releases "  # noqa: S608 (columnas fijas)
        "WHERE series = %s AND published_at <= %s AND scheduled_at >= %s AND scheduled_at < %s"
        " ORDER BY scheduled_at, source, reference_period, status",
        (series, as_of, scheduled_from, scheduled_to),
    ).fetchall()
    return [OfficialRelease(*r) for r in rows]
