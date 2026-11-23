"""Un experimento de ML v1 de punta a punta contra PostgreSQL real, con propuestas sintéticas
(#158). Necesita `DATABASE_URL` con las migraciones aplicadas; en CI corre en el job de
integración."""

import json
import math
import os
import random
import time
from datetime import UTC, datetime, timedelta

import jsonschema
import psycopg
import pytest

from cfds_intel.models.config import ROOT, load_config
from cfds_intel.models.experiment import CONFIG_FILES, run
from cfds_intel.models.registry import record

pytestmark = pytest.mark.skipif(
    "DATABASE_URL" not in os.environ, reason="necesita PostgreSQL con migraciones (DATABASE_URL)"
)

TEXTS = {n: (ROOT / "config" / n).read_text() for n in CONFIG_FILES}
CFG = load_config(TEXTS["ml.toml"])
SCHEMA = json.loads((ROOT / "contracts" / "ml_experiment.v1.schema.json").read_text())


def seed_run(conn: psycopg.Connection, run_id: str) -> None:
    """Corrida de 2021-01 a 2024-04: 12 meses fuera de muestra y una sola ventana de 24 + 3."""
    start, end = datetime(2021, 1, 1, tzinfo=UTC), datetime(2024, 4, 1, tzinfo=UTC)
    conn.execute(
        """INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone)
           VALUES ('US500', 'US 500', 'index', 'USD', 'America/New_York')
           ON CONFLICT (id) DO NOTHING"""
    )
    conn.execute(
        """INSERT INTO backtest_runs (run_id, instruments, timeframe, from_at, to_at,
             config_sha256, initial_equity, costs, news, metrics, started_at, finished_at)
           VALUES (%s, '["US500"]', '15m', %s, %s, %s, 10000, '{}', NULL, '{}', %s, %s)""",
        (run_id, start, end, "0" * 64, end, end),
    )
    rnd = random.Random(7)  # noqa: S311
    with conn.cursor() as cur:
        for i in range(700):
            at = start + timedelta(hours=37 * i, minutes=15 * (i % 3))
            trig = "sweep" if i % 2 else "double_break"
            atr = 2 + 3 * rnd.random()
            hit = rnd.random() < 1 / (1 + math.exp(-(2.5 - atr)))  # más ATR, menos TP1
            pid = f"{run_id}:US500:{i}:long"
            cur.execute(
                """INSERT INTO proposals (id, source, run_id, instrument_id, side, signal_at,
                     reference, stop, tp1, size, leverage, score, breakdown, triggers)
                   VALUES (%s, 'backtest', %s, 'US500', 'long', %s, 100, 98, 103, 1, 5, %s,
                     %s, %s)""",
                (
                    pid,
                    run_id,
                    at,
                    5 + i % 5,
                    json.dumps([{"component": "trend", "points": 2}]),
                    json.dumps([trig]),
                ),
            )
            exit_at = (at + timedelta(hours=3)).isoformat().replace("+00:00", "Z")
            reason = "tp1" if hit else "stop"
            cur.execute(
                """INSERT INTO outcomes (proposal_id, kind, entry_at, entry_price, exits,
                     commission, funding, gross_pnl, net_pnl, hit_tp1, hit_stop, exit_reason)
                   VALUES (%s, 'simulated', %s, 100, %s, 0, 0, 1, 1, %s, %s, %s)""",
                (
                    pid,
                    at,
                    json.dumps(
                        [{"at": exit_at, "price": "100", "fraction": "1", "reason": reason}]
                    ),
                    hit,
                    not hit,
                    reason,
                ),
            )
            cur.execute(
                """INSERT INTO ml_features (proposal_id, as_of, atr14_m15, realized_vol_tercile,
                     session_band) VALUES (%s, %s, %s, 'mid', 'open')""",
                (pid, at, f"{atr:.4f}"),
            )


def test_an_experiment_runs_end_to_end_and_is_recorded_once() -> None:
    run_id = f"ml-exp-{time.time_ns()}"
    with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as conn:
        seed_run(conn, run_id)
        dataset, evaluation, doc = run(conn, run_id, CFG, TEXTS)
        jsonschema.validate(doc, SCHEMA)
        assert len(dataset.examples) == 700
        assert len(evaluation.windows) == 1
        w = evaluation.windows[0]
        assert w.skipped is None, w.skipped
        # Nada del tramo fuera de muestra (desde 2023-04) se entregó.
        assert all(e.signal_at < datetime(2023, 4, 1, tzinfo=UTC) for e in w.validation)
        assert doc["result"]["status"] in {"m1_improves", "b1_stays"}
        assert record(conn, doc) is True
        assert record(conn, doc) is False, "el mismo experimento no se repite"
        with pytest.raises(psycopg.errors.RaiseException):
            conn.execute(
                "DELETE FROM ml_experiments WHERE experiment_id = %s", (doc["experiment_id"],)
            )
