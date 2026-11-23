"""Guardado del VIX en `vix_daily` contra PostgreSQL real (ADR 0012 §2).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
Los números son inventados.
"""

import os
import random
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.vix.cboe import VixDay
from cfds_intel.vix.store import insert_days

pytestmark = pytest.mark.skipif(
    "DATABASE_URL" not in os.environ, reason="necesita PostgreSQL con migraciones (DATABASE_URL)"
)


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def fresh_day() -> date:
    # La tabla es de solo agregar: cada corrida usa una fecha distinta.
    return date(1900, 1, 1) + timedelta(days=random.randrange(10_000, 40_000))  # noqa: S311


def vix(day: date, close: str = "11") -> VixDay:
    published = datetime.combine(day + timedelta(days=1), datetime.min.time(), UTC)
    return VixDay(
        day,
        published + timedelta(hours=14, minutes=30),
        Decimal("10"),
        Decimal("12"),
        Decimal("9"),
        Decimal(close),
    )


def closes(conn: psycopg.Connection, day: date) -> list[tuple[datetime, Decimal]]:
    return [
        (r[0], r[1])
        for r in conn.execute(
            "SELECT published_at, close FROM vix_daily WHERE ref_date = %s ORDER BY published_at",
            (day,),
        )
    ]


def test_insert_then_same_values_are_not_repeated(conn: psycopg.Connection) -> None:
    day = fresh_day()
    first = insert_days(conn, [vix(day)], datetime.now(UTC))
    again = insert_days(
        conn, [replace(vix(day), published_at=datetime.now(UTC))], datetime.now(UTC)
    )
    assert (first.inserted, again.unchanged) == (1, 1)
    assert closes(conn, day) == [(vix(day).published_at, Decimal("11"))]


def test_a_different_value_is_a_correction_published_when_read(conn: psycopg.Connection) -> None:
    day = fresh_day()
    insert_days(conn, [vix(day)], datetime.now(UTC))
    read = datetime.now(UTC).replace(microsecond=0)
    r = insert_days(conn, [vix(day, "11.5")], read)
    assert r.corrections == 1
    assert closes(conn, day) == [(vix(day).published_at, Decimal("11")), (read, Decimal("11.5"))]


def test_table_is_append_only_and_checks_the_data(conn: psycopg.Connection) -> None:
    day = fresh_day()
    insert_days(conn, [vix(day)], datetime.now(UTC))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("UPDATE vix_daily SET close = 2 WHERE ref_date = %s", (day,))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("DELETE FROM vix_daily WHERE ref_date = %s", (day,))
    other = fresh_day()
    early = datetime.combine(other, datetime.min.time(), UTC) + timedelta(hours=12)
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_days(conn, [replace(vix(other), published_at=early)], early)
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_days(conn, [replace(vix(fresh_day()), high=Decimal("5"))], datetime.now(UTC))
