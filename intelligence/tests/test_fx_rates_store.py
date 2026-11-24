"""Guardado de tipos de cambio en `fx_rates` contra PostgreSQL real (ADR 0013, #418).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import os
import secrets
import string
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.rates.fx import FxRate, insert_fx_rates

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

# Fecha fija: el par es el que cambia en cada test (#488).
DAY = date(2024, 1, 15)


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


@pytest.fixture
def pair() -> str:
    # La tabla es de solo agregar y no se puede limpiar: cada test usa un par propio, que nunca
    # tiene filas previas (#488). Antes se elegía una fecha al azar, que a veces repetía una fila
    # de otra corrida. Cumple `^[A-Z]{6}$` (26^6 combinaciones).
    return "".join(secrets.choice(string.ascii_uppercase) for _ in range(6))


def fx(pair: str, day: date, value: str) -> FxRate:
    published = datetime.combine(day, datetime.min.time(), UTC) + timedelta(hours=16)
    return FxRate(pair, day, published, Decimal(value))


def rows(conn: psycopg.Connection, pair: str, day: date) -> list[tuple[datetime, Decimal]]:
    return conn.execute(
        "SELECT published_at, rate FROM fx_rates WHERE pair = %s AND ref_date = %s "
        "ORDER BY published_at",
        (pair, day),
    ).fetchall()


def test_same_value_is_not_repeated_and_a_correction_is_a_new_row(
    conn: psycopg.Connection, pair: str
) -> None:
    day = DAY
    assert rows(conn, pair, day) == []
    first = fx(pair, day, "1.05")
    later = first.published_at + timedelta(days=3)
    r = insert_fx_rates(conn, [first], later)
    assert (r.inserted, r.unchanged, r.corrections) == (1, 0, 0)
    r = insert_fx_rates(conn, [replace(first, published_at=later)], later)
    assert (r.inserted, r.unchanged, r.corrections) == (0, 1, 0)
    r = insert_fx_rates(conn, [fx(pair, day, "1.06")], later)
    assert (r.inserted, r.unchanged, r.corrections) == (0, 0, 1)
    assert rows(conn, pair, day) == [
        (first.published_at, Decimal("1.05")),
        (later, Decimal("1.06")),
    ]


def test_the_table_is_append_only(conn: psycopg.Connection, pair: str) -> None:
    day = DAY
    assert rows(conn, pair, day) == []
    insert_fx_rates(conn, [fx(pair, day, "1.05")], datetime.now(UTC))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("DELETE FROM fx_rates WHERE pair = %s AND ref_date = %s", (pair, day))


def test_a_rate_published_before_its_day_or_not_positive_is_rejected(
    conn: psycopg.Connection, pair: str
) -> None:
    day = DAY
    assert rows(conn, pair, day) == []
    early = datetime.combine(day, datetime.min.time(), UTC) - timedelta(days=1)
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_fx_rates(conn, [replace(fx(pair, day, "1.05"), published_at=early)], early)
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_fx_rates(conn, [fx(pair, day, "0")], datetime.now(UTC))
