"""Guardado de los informes de análogos en `analog_reports` contra PostgreSQL real (#294).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import copy
import os
import time
from typing import Any

import psycopg
import pytest

from cfds_intel.analogs.store import insert_report

pytestmark = pytest.mark.skipif(
    "DATABASE_URL" not in os.environ, reason="necesita PostgreSQL con migraciones (DATABASE_URL)"
)


def doc() -> dict[str, Any]:
    # La tabla es de solo agregar: un instrumento propio por corrida.
    inst = f"ITAR{time.time_ns() % 10**12}.FUT"
    return {
        "instrument_id": inst,
        "source": "futures",
        "window_end_day": "2024-02-09",
        "as_of": "2024-02-09T22:00:00Z",
        "computed_at": "2026-11-23T12:00:00Z",
        "window": 20,
        "min_neighbors": 20,
        "context": {"vol": "mid", "rates": None},
        "neighbors": [],
        "groups": [
            {
                "group": "all",
                "count": 0,
                "insufficient": True,
                "horizons": [],
                "hits": {"up": 0, "down": 0, "none": 0},
                "without_atr": 0,
            },
            {
                "group": "same_context",
                "count": 0,
                "insufficient": True,
                "horizons": [],
                "hits": {"up": 0, "down": 0, "none": 0},
                "without_atr": 0,
            },
        ],
    }


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def count(conn: psycopg.Connection, inst: str) -> int:
    row = conn.execute("SELECT count(*) FROM analog_reports WHERE instrument_id = %s", (inst,))
    return int(row.fetchone()[0])  # type: ignore[index]


def test_the_same_report_recomputed_is_not_repeated(conn: psycopg.Connection) -> None:
    d = doc()
    assert insert_report(conn, d)
    later = {**d, "computed_at": "2026-11-23T13:00:00Z"}
    assert not insert_report(conn, later)
    assert count(conn, d["instrument_id"]) == 1


def test_a_changed_report_is_added_and_nothing_is_rewritten(conn: psycopg.Connection) -> None:
    d = doc()
    insert_report(conn, d)
    changed = copy.deepcopy(d)
    changed["context"]["rates"] = "sube"
    changed["computed_at"] = "2026-11-23T14:00:00Z"
    assert insert_report(conn, changed)
    assert count(conn, d["instrument_id"]) == 2
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("DELETE FROM analog_reports WHERE instrument_id = %s", (d["instrument_id"],))


def test_the_table_rejects_reports_before_the_window_closes(conn: psycopg.Connection) -> None:
    d = {**doc(), "computed_at": "2024-02-09T21:00:00Z"}
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_report(conn, d)
