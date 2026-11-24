"""Guardado de tipos de referencia en `reference_rates` contra PostgreSQL real (#79).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import os
import uuid
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.rates.ecb import Rate
from cfds_intel.rates.store import insert_rates

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

# Fecha fija: la serie es la que cambia en cada test (#483).
DAY = date(2024, 1, 15)


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


@pytest.fixture
def series() -> str:
    # La tabla es de solo agregar y no se puede limpiar: cada test usa una serie propia, que
    # nunca tiene filas previas (#483). Antes se elegía una fecha al azar, que a veces repetía
    # una fila de otra corrida. Cumple `^[A-Z][A-Z0-9_]{0,31}$` (32 caracteres).
    return "T" + uuid.uuid4().hex[:31].upper()


def rate(series: str, day: date, value: str) -> Rate:
    published = datetime.combine(day + timedelta(days=1), datetime.min.time(), UTC)
    return Rate(series, day, published + timedelta(hours=8), Decimal(value))


def rows(conn: psycopg.Connection, series: str, day: date) -> list[tuple[datetime, Decimal]]:
    return [
        (r[0], r[1])
        for r in conn.execute(
            "SELECT published_at, rate_pct FROM reference_rates "
            "WHERE series = %s AND ref_date = %s ORDER BY published_at",
            (series, day),
        )
    ]


def test_insert_then_same_value_is_not_repeated(conn: psycopg.Connection, series: str) -> None:
    day = DAY
    assert rows(conn, series, day) == []
    first = insert_rates(conn, [rate(series, day, "2.441")], datetime.now(UTC))
    again = insert_rates(
        conn,
        [replace(rate(series, day, "2.441"), published_at=datetime.now(UTC))],
        datetime.now(UTC),
    )
    assert (first.inserted, again.unchanged) == (1, 1)
    assert rows(conn, series, day) == [(rate(series, day, "2.441").published_at, Decimal("2.441"))]


def test_a_different_value_is_a_correction_published_when_read(
    conn: psycopg.Connection, series: str
) -> None:
    day = DAY
    assert rows(conn, series, day) == []
    insert_rates(conn, [rate(series, day, "2.441")], datetime.now(UTC))
    read = datetime.now(UTC).replace(microsecond=0)
    r = insert_rates(conn, [rate(series, day, "2.5")], read)
    assert r.corrections == 1
    assert rows(conn, series, day) == [
        (rate(series, day, "2.441").published_at, Decimal("2.441")),
        (read, Decimal("2.5")),
    ]


def test_table_is_append_only(conn: psycopg.Connection, series: str) -> None:
    day = DAY
    assert rows(conn, series, day) == []
    insert_rates(conn, [rate(series, day, "1")], datetime.now(UTC))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute(
            "UPDATE reference_rates SET rate_pct = 2 WHERE series = %s AND ref_date = %s",
            (series, day),
        )


def test_cannot_be_published_before_its_day_ends(conn: psycopg.Connection, series: str) -> None:
    day = DAY
    assert rows(conn, series, day) == []
    early = Rate(series, day, datetime.combine(day, datetime.min.time(), UTC), Decimal("1"))
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_rates(conn, [early], datetime.now(UTC))
