"""Guardado del dato real en `release_actuals` contra PostgreSQL (ADR 0018, #697).

Necesitan `DATABASE_URL` de una base `cfds_test*` con las migraciones aplicadas; sin base se
saltean. La tabla es de solo agregar: series únicas por corrida.
"""

import os
import uuid
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.calendar.actuals import ReleaseActual
from cfds_intel.calendar.actuals.store import insert_actuals, periods_with_first

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

RELEASED = datetime(2026, 11, 10, 13, 30, tzinfo=UTC)
READ = datetime(2026, 11, 11, 15, tzinfo=UTC)


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def actual(sid: str, **over: object) -> ReleaseActual:
    base = ReleaseActual(
        source="bls",
        series_id=sid,
        reference_period="2026-10",
        value=Decimal("337.476"),
        unit="level",
        vintage="first",
        released_at=RELEASED,
        retrieved_at=READ,
        source_url="https://api.bls.gov/publicAPI/v1/timeseries/data/",
    )
    return replace(base, **over)  # type: ignore[arg-type]


def sid() -> str:
    return "T" + uuid.uuid4().hex[:20].upper()


def test_reading_again_adds_nothing_and_one_first_per_month(conn: psycopg.Connection) -> None:
    s = sid()
    assert insert_actuals(conn, [actual(s)]).inserted == 1
    again = insert_actuals(conn, [actual(s, retrieved_at=READ + timedelta(days=1))])
    other_first = insert_actuals(conn, [actual(s, value=Decimal("337.5"))])
    assert (again.inserted, other_first.inserted) == (0, 0)
    assert periods_with_first(conn, [s]) == {(s, "2026-10")}


def test_a_revised_value_is_a_new_row_and_keeps_its_exact_scale(conn: psycopg.Connection) -> None:
    s = sid()
    late = READ + timedelta(days=40)
    insert_actuals(conn, [actual(s), actual(s, vintage="revised", value=Decimal("337.500"))])
    insert_actuals(
        conn, [actual(s, vintage="revised", value=Decimal("337.500"), retrieved_at=late)]
    )
    rows = conn.execute(
        "SELECT vintage, value::text FROM release_actuals WHERE series_id = %s ORDER BY vintage",
        (s,),
    ).fetchall()
    assert rows == [("first", "337.476"), ("revised", "337.500")]


def test_read_before_release_is_rejected_by_the_table(conn: psycopg.Connection) -> None:
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_actuals(conn, [actual(sid(), retrieved_at=RELEASED - timedelta(minutes=1))])
