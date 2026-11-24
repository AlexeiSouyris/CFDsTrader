"""Guardado de las barras de Databento (#247, `infra/migrations/0023_futures_bars.sql`).

- `futures_bars_1m`: las barras tal como llegan (`contracts/futures_bar.v1`), de solo agregar.
- `futures_bars_1d`: las velas diarias de `ohlcv-1d` (`contracts/futures_bar_1d.v1`, #522,
  `infra/migrations/0047_futures_bars_1d.sql`), con las mismas reglas. Cubren un día UTC, no la
  sesión de la bolsa.
  Clave `(source, dataset, symbol, start_at)`. La misma barra otra vez no cambia nada, y otra
  barra con la misma clave es un conflicto: no se pisa y no se guarda nada del tramo.
- `futures_bar_loads`: un registro por tramo bajado, con su esquema y su costo estimado. Así
  una corrida siguiente baja solo lo que falta antes del primer tramo y después del último, y
  **no vuelve a pagar** lo ya guardado.
- No se tocan las velas de IG (`candles`).
- Todo o nada: un tramo y su registro van en una sola transacción.
"""

from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, date, datetime
from decimal import Decimal

import psycopg
from psycopg import sql

from cfds_intel.databento.bars import SOURCE, Bar, Stored
from cfds_intel.databento.config import DAILY, MINUTE, SymbolConfig

# Tabla de cada esquema. Ninguna otra: un esquema nuevo necesita su migración y su contrato.
TABLES = {MINUTE: "futures_bars_1m", DAILY: "futures_bars_1d"}


class BarsConflictError(ValueError):
    """Una barra ya guardada llegó con otros valores."""


@dataclass(frozen=True)
class ChunkReport:
    inserted: int
    unchanged: int


def loaded_spans(conn: psycopg.Connection, schema: str = MINUTE) -> dict[tuple[str, str], Stored]:
    """Desde qué día y hasta qué día (exclusivo) está bajado cada símbolo en `schema`."""
    rows = conn.execute(
        "SELECT dataset, symbol, min(from_at), max(to_at) FROM futures_bar_loads"
        " WHERE source = %s AND schema = %s GROUP BY dataset, symbol",
        (SOURCE, schema),
    ).fetchall()
    return {
        (d, s): Stored(f.astimezone(UTC).date(), t.astimezone(UTC).date()) for d, s, f, t in rows
    }


def _midnight(d: date) -> datetime:
    return datetime(d.year, d.month, d.day, tzinfo=UTC)


def insert_chunk(
    conn: psycopg.Connection,
    sym: SymbolConfig,
    start: date,
    end: date,
    bars: Sequence[Bar],
    cost_usd: Decimal,
    schema: str = MINUTE,
) -> ChunkReport:
    """Guarda las barras de `[start, end)` de `schema` en su tabla y el registro del tramo, en
    una transacción."""
    table = sql.Identifier(TABLES[schema])
    with conn.transaction(), conn.cursor() as cur:
        # Si la conexión ya tenía una transacción abierta, `conn.transaction()` es un savepoint y
        # el ON COMMIT DROP no corre hasta el commit final: sin este DROP, el segundo tramo falla
        # con "relation incoming already exists".
        cur.execute("DROP TABLE IF EXISTS pg_temp.incoming")
        cur.execute(
            sql.SQL(
                "CREATE TEMP TABLE incoming (LIKE {} INCLUDING DEFAULTS) ON COMMIT DROP"
            ).format(table)
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
            sql.SQL(
                """
            SELECT count(*) FROM incoming i JOIN {} f
              USING (source, dataset, symbol, start_at)
            WHERE (i.instrument_id, i.contract_id, i.open, i.high, i.low, i.close, i.volume)
               IS DISTINCT FROM (f.instrument_id, f.contract_id, f.open, f.high, f.low, f.close,
                                 f.volume)
            """
            ).format(table)
        )
        row = cur.fetchone()
        if row is not None and row[0]:
            raise BarsConflictError(
                f"{sym.symbol}: {row[0]} barras ya guardadas llegaron con otros valores:"
                " no se pisan"
            )
        cur.execute(
            sql.SQL(
                """
            INSERT INTO {} (source, dataset, symbol, instrument_id, contract_id,
                start_at, open, high, low, close, volume)
            SELECT source, dataset, symbol, instrument_id, contract_id, start_at, open, high,
                low, close, volume FROM incoming
            ON CONFLICT ON CONSTRAINT {} DO NOTHING
            """
            ).format(table, sql.Identifier(f"{TABLES[schema]}_key"))
        )
        inserted = cur.rowcount
        cur.execute(
            """
            INSERT INTO futures_bar_loads
                (source, dataset, schema, symbol, from_at, to_at, bars, cost_usd)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
            ON CONFLICT ON CONSTRAINT futures_bar_loads_key DO NOTHING
            """,
            (
                SOURCE,
                sym.dataset,
                schema,
                sym.symbol,
                _midnight(start),
                _midnight(end),
                len(bars),
                cost_usd,
            ),
        )
    return ChunkReport(inserted, len(bars) - inserted)
