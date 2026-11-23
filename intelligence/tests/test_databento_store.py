"""Guardado de las barras de Databento contra PostgreSQL real (#247).

Necesitan `DATABASE_URL` con las migraciones aplicadas (las aplica el motor). En CI corren en el
job de integración; sin base se saltean. Las tablas son de solo agregar: cada corrida usa un
dataset propio (sintético).
"""

import os
import uuid
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.databento.bars import Bar
from cfds_intel.databento.config import SymbolConfig
from cfds_intel.databento.store import BarsConflictError, insert_chunk, loaded_until

pytestmark = pytest.mark.skipif(
    "DATABASE_URL" not in os.environ, reason="necesita PostgreSQL con migraciones (DATABASE_URL)"
)

DAY = date(2026, 11, 18)


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def sym() -> SymbolConfig:
    return SymbolConfig(f"T{uuid.uuid4().int % 10**12}.TEST", "ES.c.0", "US500", 22)


def bars(s: SymbolConfig, n: int) -> list[Bar]:
    t0 = datetime(2026, 11, 18, 14, 30, tzinfo=UTC)
    return [
        Bar(
            dataset=s.dataset,
            symbol=s.symbol,
            instrument_id=s.instrument_id,
            contract_id=42,
            start=t0 + timedelta(minutes=i),
            open=Decimal("5000.25"),
            high=Decimal("5000.75"),
            low=Decimal("5000"),
            close=Decimal("5000.5"),
            volume=17 + i,
        )
        for i in range(n)
    ]


def stored(conn: psycopg.Connection, s: SymbolConfig) -> int:
    row = conn.execute(
        "SELECT count(*) FROM futures_bars_1m WHERE dataset = %s", (s.dataset,)
    ).fetchone()
    assert row is not None
    return int(row[0])


def test_chunks_are_idempotent_and_record_where_to_continue(conn: psycopg.Connection) -> None:
    s = sym()
    candles_before = conn.execute("SELECT count(*) FROM candles").fetchone()
    first = insert_chunk(conn, s, DAY, DAY + timedelta(days=1), bars(s, 3), Decimal("0.01"))
    again = insert_chunk(conn, s, DAY, DAY + timedelta(days=1), bars(s, 3), Decimal("0.01"))
    assert (first.inserted, again.inserted, again.unchanged) == (3, 0, 3)
    assert stored(conn, s) == 3
    assert loaded_until(conn)[(s.dataset, s.symbol)] == DAY + timedelta(days=1)
    # Las velas de IG no se tocan.
    assert conn.execute("SELECT count(*) FROM candles").fetchone() == candles_before
    row = conn.execute(
        "SELECT open, start_at FROM futures_bars_1m WHERE dataset = %s ORDER BY start_at LIMIT 1",
        (s.dataset,),
    ).fetchone()
    assert row == (Decimal("5000.25"), datetime(2026, 11, 18, 14, 30, tzinfo=UTC))


def test_a_different_bar_is_a_conflict_and_saves_nothing(conn: psycopg.Connection) -> None:
    s = sym()
    insert_chunk(conn, s, DAY, DAY + timedelta(days=1), bars(s, 2), Decimal(0))
    changed = [*bars(s, 1), replace(bars(s, 2)[1], close=Decimal("5000.25")), *bars(s, 3)[2:]]
    with pytest.raises(BarsConflictError):
        insert_chunk(conn, s, DAY, DAY + timedelta(days=2), changed, Decimal(0))
    assert stored(conn, s) == 2
    assert loaded_until(conn)[(s.dataset, s.symbol)] == DAY + timedelta(days=1)


def test_the_tables_are_append_only_and_check_the_contract(conn: psycopg.Connection) -> None:
    s = sym()
    insert_chunk(conn, s, DAY, DAY + timedelta(days=1), bars(s, 1), Decimal(0))
    for sql in [
        "UPDATE futures_bars_1m SET volume = 0 WHERE dataset = %s",
        "DELETE FROM futures_bars_1m WHERE dataset = %s",
        "UPDATE futures_bar_loads SET bars = 0 WHERE dataset = %s",
        "DELETE FROM futures_bar_loads WHERE dataset = %s",
    ]:
        with pytest.raises(psycopg.Error):
            conn.execute(sql, (s.dataset,))
    later = replace(bars(s, 1)[0], start=datetime(2026, 11, 23, 14, 30, tzinfo=UTC))
    for bad in [
        replace(bars(s, 1)[0], start=datetime(2026, 11, 18, 14, 30, 30, tzinfo=UTC)),
        replace(later, low=Decimal("6000")),
        replace(later, symbol="ESZ6"),
    ]:
        with pytest.raises(psycopg.Error):
            insert_chunk(
                conn, s, DAY + timedelta(days=5), DAY + timedelta(days=6), [bad], Decimal(0)
            )
