"""Dataset del modelo de ML v1 leído de PostgreSQL real (#158).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import json
import os
import time
from datetime import UTC, datetime, timedelta

import psycopg
import pytest

from cfds_intel.models.config import load_default
from cfds_intel.models.dataset import DatasetError, load

pytestmark = pytest.mark.skipif(
    "DATABASE_URL" not in os.environ, reason="necesita PostgreSQL con migraciones (DATABASE_URL)"
)

CFG = load_default().dataset
T0 = datetime(2026, 11, 20, 15, 0, tzinfo=UTC)


@pytest.fixture
def conn() -> psycopg.Connection:
    c = psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)
    c.execute(
        """INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone)
           VALUES ('US500', 'US 500', 'index', 'USD', 'America/New_York')
           ON CONFLICT (id) DO NOTHING"""
    )
    return c


def put_run(conn: psycopg.Connection, run_id: str) -> None:
    conn.execute(
        """INSERT INTO backtest_runs (run_id, instruments, timeframe, from_at, to_at,
             config_sha256, initial_equity, costs, news, metrics, started_at, finished_at)
           VALUES (%s, '["US500"]', '15m', %s, %s, %s, 10000, '{}', NULL, '{}', %s, %s)""",
        (run_id, T0, T0 + timedelta(days=1), "0" * 64, T0, T0),
    )


def put_proposal(
    conn: psycopg.Connection, run_id: str, k: int, *, hit: bool, features: bool
) -> str:
    at = T0 + timedelta(minutes=15 * k)
    pid = f"{run_id}:US500:{k}:long"
    conn.execute(
        """INSERT INTO proposals (id, source, run_id, instrument_id, side, signal_at, reference,
             stop, tp1, size, leverage, score, breakdown, triggers)
           VALUES (%s, 'backtest', %s, 'US500', 'long', %s, 100, 98, 103, 1, 5, 6, %s, %s)""",
        (pid, run_id, at, json.dumps([{"component": "trend", "points": 2}]), '["sweep"]'),
    )
    exit_at = (at + timedelta(hours=2)).isoformat().replace("+00:00", "Z")
    exits = [{"at": exit_at, "price": "103", "fraction": "1", "reason": "tp1" if hit else "time"}]
    conn.execute(
        """INSERT INTO outcomes (proposal_id, kind, entry_at, entry_price, exits, commission,
             funding, gross_pnl, net_pnl, hit_tp1, hit_stop, exit_reason)
           VALUES (%s, 'simulated', %s, 100, %s, 0, 0, 1, 1, %s, false, %s)""",
        (pid, at, json.dumps(exits), hit, "tp1" if hit else "time"),
    )
    if features:
        conn.execute(
            """INSERT INTO ml_features (proposal_id, as_of, atr14_m15, realized_vol_tercile,
                 session_band) VALUES (%s, %s, 4, 'high', 'mid')""",
            (pid, at),
        )
    return pid


def test_a_run_is_read_with_its_outcomes_and_features(conn: psycopg.Connection) -> None:
    # Tablas de solo agregar: un run_id propio por corrida del test.
    run_id = f"ml-test-{time.time_ns()}"
    put_run(conn, run_id)
    a = put_proposal(conn, run_id, 1, hit=True, features=True)
    b = put_proposal(conn, run_id, 2, hit=False, features=True)
    put_proposal(conn, run_id, 3, hit=True, features=False)
    d = load(conn, run_id, CFG)
    assert [(e.proposal_id, e.y) for e in d.examples] == [(a, 1), (b, 0)]
    assert d.without_features == 1
    assert d.examples[0].x[d.feature_names.index("stop_atr")] == 0.5
    assert d.examples[0].known_at == T0 + timedelta(minutes=15, hours=2)


def test_an_unknown_run_is_an_error(conn: psycopg.Connection) -> None:
    with pytest.raises(DatasetError):
        load(conn, f"no-existe-{time.time_ns()}", CFG)


def test_ml_features_are_append_only(conn: psycopg.Connection) -> None:
    run_id = f"ml-test-{time.time_ns()}"
    put_run(conn, run_id)
    pid = put_proposal(conn, run_id, 1, hit=True, features=True)
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("UPDATE ml_features SET atr14_m15 = 5 WHERE proposal_id = %s", (pid,))
