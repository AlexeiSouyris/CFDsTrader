"""Guardado de las barras de Databento (#247, `infra/migrations/0023_futures_bars.sql`).

- `futures_bars_1m`: las barras tal como llegan (`contracts/futures_bar.v1`), de solo agregar.
  Clave `(source, dataset, symbol, start_at)`. La misma barra otra vez no cambia nada, y otra
  barra con la misma clave es un conflicto: no se pisa y no se guarda nada del tramo.
- `futures_bar_loads`: un registro por tramo bajado, con su costo estimado. Así una corrida
  siguiente sigue desde el último tramo y **no vuelve a pagar** lo ya guardado.
- No se tocan las velas de IG (`candles`).
- Todo o nada: un tramo y su registro van en una sola transacción.
"""

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, date, datetime
from decimal import Decimal

import psycopg

from cfds_intel.databento.bars import SOURCE, Bar
from cfds_intel.databento.config import SymbolConfig


class BarsConflictError(ValueError):
    """Una barra ya guardada llegó con otros valores."""


@dataclass(frozen=True)
class ChunkReport:
    inserted: int
    unchanged: int


def loaded_until(conn: psycopg.Connection) -> dict[tuple[str, str], date]:
    """Hasta qué día (exclusivo) está bajado cada símbolo."""
    rows = conn.execute(
        "SELECT dataset, symbol, max(to_at) FROM futures_bar_loads WHERE source = %s"
        " GROUP BY dataset, symbol",
        (SOURCE,),
    ).fetchall()
    return {(d, s): t.astimezone(UTC).date() for d, s, t in rows}


def _midnight(d: date) -> datetime:
    return datetime(d.year, d.month, d.day, tzinfo=UTC)


def insert_chunk(
    conn: psycopg.Connection,
    sym: SymbolConfig,
    start: date,
    end: date,
    bars: Sequence[Bar],
    cost_usd: Decimal,
) -> ChunkReport:
    """Guarda las barras de `[start, end)` y el registro del tramo, en una transacción."""
    with conn.transaction(), conn.cursor() as cur:
        cur.execute(
            "CREATE TEMP TABLE incoming (LIKE futures_bars_1m INCLUDING DEFAULTS) ON COMMIT DROP"
        )
        with cur.copy(
            "COPY incoming (source, dataset, symbol, instrument_id, contract_id, start_at,"
            " open, high, low, close, volume) FROM STDIN"
        ) as copy:
            for b in bars:
                copy.write_row(
                    (
                        SOURCE,
                        b.dataset,
                        b.symbol,
                        b.instrument_id,
                        b.contract_id,
                        b.start,
                        b.open,
                        b.high,
                        b.low,
                        b.close,
                        b.volume,
                    )
                )
        cur.execute(
            """
            SELECT count(*) FROM incoming i JOIN futures_bars_1m f
              USING (source, dataset, symbol, start_at)
            WHERE (i.instrument_id, i.contract_id, i.open, i.high, i.low, i.close, i.volume)
               IS DISTINCT FROM (f.instrument_id, f.contract_id, f.open, f.high, f.low, f.close,
                                 f.volume)
            """
        )
        row = cur.fetchone()
        if row is not None and row[0]:
            raise BarsConflictError(
                f"{sym.symbol}: {row[0]} barras ya guardadas llegaron con otros valores:"
                " no se pisan"
            )
        cur.execute(
            """
            INSERT INTO futures_bars_1m (source, dataset, symbol, instrument_id, contract_id,
                start_at, open, high, low, close, volume)
            SELECT source, dataset, symbol, instrument_id, contract_id, start_at, open, high,
                low, close, volume FROM incoming
            ON CONFLICT ON CONSTRAINT futures_bars_1m_key DO NOTHING
            """
        )
        inserted = cur.rowcount
        cur.execute(
            """
            INSERT INTO futures_bar_loads (source, dataset, symbol, from_at, to_at, bars, cost_usd)
            VALUES (%s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT ON CONSTRAINT futures_bar_loads_key DO NOTHING
            """,
            (
                SOURCE,
                sym.dataset,
                sym.symbol,
                _midnight(start),
                _midnight(end),
                len(bars),
                cost_usd,
            ),
        )
    return ChunkReport(inserted, len(bars) - inserted)
