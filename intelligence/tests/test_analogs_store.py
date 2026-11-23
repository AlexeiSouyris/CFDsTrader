"""Los análogos leen de PostgreSQL el tercio de la volatilidad que guarda Rust
(`daily_vol_terciles`, #158): no lo calculan (CLAUDE.md §1).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import os
import time
from datetime import UTC, date, datetime, timedelta

import psycopg
import pytest

from cfds_intel.analogs.data import load_bars

pytestmark = pytest.mark.skipif(
    "DATABASE_URL" not in os.environ, reason="necesita PostgreSQL con migraciones (DATABASE_URL)"
)


def test_bars_carry_the_stored_tercile_or_none() -> None:
    # Las tablas diarias son de solo agregar: un instrumento propio por corrida.
    inst = f"ITAN{time.time_ns() % 10**12}"
    days = [date(2026, 11, 19), date(2026, 11, 20)]
    with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as conn:
        conn.execute(
            """INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone)
               VALUES (%s, 'test', 'index', 'USD', 'America/New_York')""",
            (inst,),
        )
        for d in days:
            start = datetime(d.year, d.month, d.day, 5, tzinfo=UTC)
            end = start + timedelta(days=1)
            conn.execute(
                """INSERT INTO daily_candles (instrument_id, day, start_at, end_at, open, high,
                     low, close) VALUES (%s, %s, %s, %s, 100, 101, 99, 100)""",
                (inst, d, start, end),
            )
            conn.execute(
                """INSERT INTO daily_indicators (instrument_id, day, published_at, atr_period, atr,
                     realized_vol_window, realized_vol) VALUES (%s, %s, %s, 14, 1, 20, 0.01)""",
                (inst, d, end),
            )
        # Solo el primer día tiene tercio; uno con otros años de historia no cuenta.
        for years, tercile in ((3, "mid"), (2, "high")):
            conn.execute(
                """INSERT INTO daily_vol_terciles (instrument_id, day, published_at,
                     realized_vol_window, history_years, tercile)
                   VALUES (%s, %s, %s, 20, %s, %s)""",
                (inst, days[0], datetime(2026, 11, 20, 5, tzinfo=UTC), years, tercile),
            )
        bars = load_bars(conn, 14, 20, 3)[inst]
    assert [b.day for b in bars] == days
    assert [b.vol_tercile for b in bars] == ["mid", None]
