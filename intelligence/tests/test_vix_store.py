"""Guardado del VIX en `vix_daily` contra PostgreSQL real (ADR 0012 §2).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
Los números son inventados.
"""

import os
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.vix.cboe import VixDay
from cfds_intel.vix.store import insert_days

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


# Rango de fechas reservado a los tests: ningún dato real es anterior a 1900.
TEST_FIRST_DAY = date(1000, 1, 1)
TEST_LAST_DAY = date(1899, 12, 31)


def free_day(conn: psycopg.Connection) -> date:
    """Un día sin filas, propio de este test (#488).

    La tabla es de solo agregar y no se puede limpiar, y la fecha es la única parte libre de la
    clave, así que no hay una serie propia que usar. Antes se elegía una fecha al azar, que a
    veces repetía una fila de otra corrida. Ahora cada llamada toma el día siguiente al último
    usado dentro del rango reservado: nunca repite uno con filas.
    """
    last = conn.execute(
        "SELECT max(ref_date) FROM vix_daily WHERE ref_date BETWEEN %s AND %s",
        (TEST_FIRST_DAY, TEST_LAST_DAY),
    ).fetchone()
    day = TEST_FIRST_DAY if last is None or last[0] is None else last[0] + timedelta(days=1)
    assert day <= TEST_LAST_DAY, "se agotó el rango de fechas de prueba: recreá la base de tests"
    return day


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
    day = free_day(conn)
    assert closes(conn, day) == []
    first = insert_days(conn, [vix(day)], datetime.now(UTC))
    again = insert_days(
        conn, [replace(vix(day), published_at=datetime.now(UTC))], datetime.now(UTC)
    )
    assert (first.inserted, again.unchanged) == (1, 1)
    assert closes(conn, day) == [(vix(day).published_at, Decimal("11"))]


def test_a_different_value_is_a_correction_published_when_read(conn: psycopg.Connection) -> None:
    day = free_day(conn)
    assert closes(conn, day) == []
    insert_days(conn, [vix(day)], datetime.now(UTC))
    read = datetime.now(UTC).replace(microsecond=0)
    r = insert_days(conn, [vix(day, "11.5")], read)
    assert r.corrections == 1
    assert closes(conn, day) == [(vix(day).published_at, Decimal("11")), (read, Decimal("11.5"))]


def test_table_is_append_only_and_checks_the_data(conn: psycopg.Connection) -> None:
    day = free_day(conn)
    assert closes(conn, day) == []
    insert_days(conn, [vix(day)], datetime.now(UTC))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("UPDATE vix_daily SET close = 2 WHERE ref_date = %s", (day,))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("DELETE FROM vix_daily WHERE ref_date = %s", (day,))
    other = free_day(conn)
    assert other != day and closes(conn, other) == []
    early = datetime.combine(other, datetime.min.time(), UTC) + timedelta(hours=12)
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_days(conn, [replace(vix(other), published_at=early)], early)
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_days(conn, [replace(vix(other), high=Decimal("5"))], datetime.now(UTC))
