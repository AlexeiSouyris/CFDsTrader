"""Reporte de la fase 4b leído de PostgreSQL real (#221): dos corridas sintéticas guardadas como
las guarda `backtest_run` (con noticias y con `news: null`), sin experimento de ML.

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import json
import os
import time
from datetime import UTC, datetime, timedelta

import psycopg
import pytest

from cfds_intel.phase4b.__main__ import report
from cfds_intel.phase4b.report import load_run

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

FROM, TO = datetime(2021, 1, 1, tzinfo=UTC), datetime(2026, 4, 30, tzinfo=UTC)
NEWS = {
    "interpreter_name": "ollama",
    "interpreter_version": "1",
    "interpreter_model": "m",
    "published_from": "2025-12-03T00:00:00Z",
    "interpretations": 12,
}


def seed(conn: psycopg.Connection, run_id: str, news: dict[str, object] | None, bonus: int) -> None:
    conn.execute(
        """INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone)
           VALUES ('US500', 'US 500', 'index', 'USD', 'America/New_York')
           ON CONFLICT (id) DO NOTHING"""
    )
    conn.execute(
        """INSERT INTO backtest_runs (run_id, instruments, timeframe, from_at, to_at,
             config_sha256, initial_equity, costs, news, metrics, started_at, finished_at)
           VALUES (%s, '["US500"]', '15m', %s, %s, %s, 10000, '{}', %s, '{}', %s, %s)""",
        (run_id, FROM, TO, "c" * 64, None if news is None else json.dumps(news), TO, TO),
    )
    # Una operación por semana desde la primera validación (enero de 2023) durante 80 semanas.
    start = datetime(2023, 1, 2, 15, tzinfo=UTC)
    for k in range(80):
        at = start + timedelta(weeks=k)
        pid = f"{run_id}:US500:{k}:long"
        conn.execute(
            """INSERT INTO proposals (id, source, run_id, instrument_id, side, signal_at,
                 reference, stop, tp1, size, leverage, score, breakdown, triggers)
               VALUES (%s, 'backtest', %s, 'US500', 'long', %s, 100, 98, 103, 1, 5, 6,
                 '[]', '["sweep"]')""",
            (pid, run_id, at),
        )
        exit_at = (at + timedelta(hours=2)).isoformat().replace("+00:00", "Z")
        exits = [{"at": exit_at, "price": "101", "fraction": "1", "reason": "tp1"}]
        pnl = 5 + bonus + k % 4
        conn.execute(
            """INSERT INTO outcomes (proposal_id, kind, entry_at, entry_price, exits, commission,
                 funding, gross_pnl, net_pnl, hit_tp1, hit_stop, exit_reason)
               VALUES (%s, 'simulated', %s, 100, %s, 0, 0, %s, %s, true, false, 'tp1')""",
            (pid, at, json.dumps(exits), pnl, pnl),
        )


def test_the_report_reads_both_runs_and_concludes_per_layer() -> None:
    base = f"p4b-{time.time_ns()}"
    on, off = f"{base}-noticias", f"{base}-sin-noticias"
    with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as conn:
        seed(conn, on, NEWS, bonus=3)
        seed(conn, off, None, bonus=0)
        run = load_run(conn, on)
        assert run.news is not None and run.news["interpretations"] == 12
        assert len(run.trades) == 80
        assert load_run(conn, off).news is None
        text = report(conn, on, off)
    assert "| Noticias (ajuste del puntaje) | **mejora** |" in text
    assert "| ML v1 (M1 contra B1) | **datos insuficientes** |" in text
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn, pytest.raises(ValueError):
        load_run(conn, f"{base}-no-existe")
