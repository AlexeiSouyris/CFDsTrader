"""Lectura de los comunicados y guardado de la novedad (#668).

- Lee de `official_releases` (ADR 0016) los comunicados del FOMC del período: solo lectura.
- Guarda en `fomc_statement_novelty` (migración 0050), de solo agregar, en una transacción.
  Otra corrida igual no agrega nada; una fila con la misma clave y otros datos es un error.
"""

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, date, datetime, time, timedelta

import psycopg

from cfds_intel.fomc_novelty.model import NoveltyError, NoveltyRow, Statement

_COLUMNS = (
    "reference_period, status, statement_url, available_at, previous_reference_period,"
    " previous_statement_url, method, method_version, model, model_digest, value, text_sha256,"
    " previous_text_sha256, word_count, previous_word_count, computed_at"
)
# Lo que tiene que coincidir con una fila ya guardada (todo menos cuándo se calculó).
_COMPARED = _COLUMNS.rsplit(",", 1)[0]


def statements(
    conn: psycopg.Connection, first_day: date, last_day: date, statuses: Sequence[str]
) -> list[Statement]:
    """Comunicados del FOMC con hora en [first_day, last_day] (UTC), en orden de hora."""
    rows = conn.execute(
        "SELECT reference_period, status, scheduled_at, source_url FROM official_releases"
        " WHERE source = 'fed_fomc' AND series = 'fomc_statement' AND status = ANY(%s)"
        " AND scheduled_at >= %s AND scheduled_at < %s ORDER BY scheduled_at",
        (
            list(statuses),
            datetime.combine(first_day, time(0), UTC),
            datetime.combine(last_day + timedelta(days=1), time(0), UTC),
        ),
    ).fetchall()
    return [Statement(*r) for r in rows]


@dataclass(frozen=True)
class InsertReport:
    inserted: int
    unchanged: int


def _values(r: NoveltyRow) -> tuple[object, ...]:
    return (
        r.reference_period,
        r.status,
        r.statement_url,
        r.available_at,
        r.previous_reference_period,
        r.previous_statement_url,
        r.method,
        r.method_version,
        r.model,
        r.model_digest,
        r.value,
        r.text_sha256,
        r.previous_text_sha256,
        r.word_count,
        r.previous_word_count,
        r.computed_at,
    )


def insert_rows(conn: psycopg.Connection, rows: Sequence[NoveltyRow]) -> InsertReport:
    inserted = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in rows:
            cur.execute(
                f"INSERT INTO fomc_statement_novelty ({_COLUMNS}) VALUES "  # noqa: S608 (columnas fijas)
                "(%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s) "
                "ON CONFLICT ON CONSTRAINT fomc_statement_novelty_key DO NOTHING",
                _values(r),
            )
            if cur.rowcount == 1:
                inserted += 1
                continue
            stored = cur.execute(
                f"SELECT {_COMPARED} FROM fomc_statement_novelty "  # noqa: S608 (columnas fijas)
                "WHERE reference_period = %s AND method = %s AND method_version = %s"
                " AND model IS NOT DISTINCT FROM %s AND model_digest IS NOT DISTINCT FROM %s",
                (r.reference_period, r.method, r.method_version, r.model, r.model_digest),
            ).fetchone()
            if stored is None or tuple(stored) != _values(r)[:-1]:
                raise NoveltyError(
                    f"{r.reference_period} {r.method}: ya está guardada con otros datos"
                    f" ({stored}); la tabla es de solo agregar y no se pisa"
                )
    return InsertReport(inserted, len(rows) - inserted)
