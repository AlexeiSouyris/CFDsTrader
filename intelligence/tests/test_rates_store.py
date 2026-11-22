"""Guardado de tipos de referencia en `reference_rates` contra PostgreSQL real (#79).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import os
import random
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.rates.ecb import Rate
from cfds_intel.rates.store import insert_rates

pytestmark = pytest.mark.skipif(
    "DATABASE_URL" not in os.environ, reason="necesita PostgreSQL con migraciones (DATABASE_URL)"
)

SERIES = "TEST_ESTR"


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def fresh_day() -> date:
    # La tabla es de solo agregar: cada corrida usa una fecha distinta.
    return date(1900, 1, 1) + timedelta(days=random.randrange(10_000, 40_000))  # noqa: S311


def rate(day: date, value: str) -> Rate:
    published = datetime.combine(day + timedelta(days=1), datetime.min.time(), UTC)
    return Rate(SERIES, day, published + timedelta(hours=8), Decimal(value))


def rows(conn: psycopg.Connection, day: date) -> list[tuple[datetime, Decimal]]:
    return [
        (r[0], r[1])
        for r in conn.execute(
            "SELECT published_at, rate_pct FROM reference_rates "
            "WHERE series = %s AND ref_date = %s ORDER BY published_at",
            (SERIES, day),
        )
    ]


def test_insert_then_same_value_is_not_repeated(conn: psycopg.Connection) -> None:
    day = fresh_day()
    first = insert_rates(conn, [rate(day, "2.441")], datetime.now(UTC))
    again = insert_rates(
        conn, [replace(rate(day, "2.441"), published_at=datetime.now(UTC))], datetime.now(UTC)
    )
    assert (first.inserted, again.unchanged) == (1, 1)
    assert rows(conn, day) == [(rate(day, "2.441").published_at, Decimal("2.441"))]


def test_a_different_value_is_a_correction_published_when_read(conn: psycopg.Connection) -> None:
    day = fresh_day()
    insert_rates(conn, [rate(day, "2.441")], datetime.now(UTC))
    read = datetime.now(UTC).replace(microsecond=0)
    r = insert_rates(conn, [rate(day, "2.5")], read)
    assert r.corrections == 1
    assert rows(conn, day) == [
        (rate(day, "2.441").published_at, Decimal("2.441")),
        (read, Decimal("2.5")),
    ]


def test_table_is_append_only(conn: psycopg.Connection) -> None:
    day = fresh_day()
    insert_rates(conn, [rate(day, "1")], datetime.now(UTC))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute(
            "UPDATE reference_rates SET rate_pct = 2 WHERE series = %s AND ref_date = %s",
            (SERIES, day),
        )


def test_cannot_be_published_before_its_day_ends(conn: psycopg.Connection) -> None:
    day = fresh_day()
    early = Rate(SERIES, day, datetime.combine(day, datetime.min.time(), UTC), Decimal("1"))
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_rates(conn, [early], datetime.now(UTC))
