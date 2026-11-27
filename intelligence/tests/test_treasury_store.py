"""Guardado de la curva del Tesoro en `treasury_yields` contra PostgreSQL real (ADR 0012).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
Los números son sintéticos.
"""

import os
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.treasury.store import insert_yields
from cfds_intel.treasury.ust import TreasuryYield

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
        "SELECT max(ref_date) FROM treasury_yields WHERE ref_date BETWEEN %s AND %s",
        (TEST_FIRST_DAY, TEST_LAST_DAY),
    ).fetchone()
    day = TEST_FIRST_DAY if last is None or last[0] is None else last[0] + timedelta(days=1)
    assert day <= TEST_LAST_DAY, "se agotó el rango de fechas de prueba: recreá la base de tests"
    return day


def y(day: date, value: str, tenor: str = "10Y") -> TreasuryYield:
    published = datetime.combine(day + timedelta(days=1), datetime.min.time(), UTC)
    return TreasuryYield(tenor, day, published + timedelta(hours=14, minutes=30), Decimal(value))


def rows(conn: psycopg.Connection, day: date) -> list[tuple[str, datetime, Decimal]]:
    return [
        (r[0], r[1], r[2])
        for r in conn.execute(
            "SELECT tenor, published_at, yield_pct FROM treasury_yields "
            "WHERE ref_date = %s ORDER BY tenor, published_at",
            (day,),
        )
    ]


def test_insert_then_same_value_is_not_repeated(conn: psycopg.Connection) -> None:
    day = free_day(conn)
    assert rows(conn, day) == []
    first = insert_yields(conn, [y(day, "1.5"), y(day, "0.5", "2Y")], datetime.now(UTC))
    again = insert_yields(
        conn, [replace(y(day, "1.5"), published_at=datetime.now(UTC))], datetime.now(UTC)
    )
    assert (first.inserted, again.unchanged) == (2, 1)
    assert rows(conn, day) == [
        ("10Y", y(day, "1.5").published_at, Decimal("1.5")),
        ("2Y", y(day, "0.5").published_at, Decimal("0.5")),
    ]


def test_a_different_value_is_a_correction_published_when_read(conn: psycopg.Connection) -> None:
    day = free_day(conn)
    assert rows(conn, day) == []
    insert_yields(conn, [y(day, "1.5")], datetime.now(UTC))
    read = datetime.now(UTC).replace(microsecond=0)
    r = insert_yields(conn, [y(day, "1.6")], read)
    assert r.corrections == 1
    assert rows(conn, day) == [
        ("10Y", y(day, "1.5").published_at, Decimal("1.5")),
        ("10Y", read, Decimal("1.6")),
    ]


def test_table_is_append_only_and_rejects_early_publication(conn: psycopg.Connection) -> None:
    day = free_day(conn)
    assert rows(conn, day) == []
    insert_yields(conn, [y(day, "1")], datetime.now(UTC))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("UPDATE treasury_yields SET yield_pct = 2 WHERE ref_date = %s", (day,))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("DELETE FROM treasury_yields WHERE ref_date = %s", (day,))
    # antes de las 15:30 ET de su día, el valor no existe
    other = free_day(conn)
    assert other != day and rows(conn, other) == []
    early = datetime.combine(other, datetime.min.time(), UTC) + timedelta(hours=12)
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_yields(conn, [replace(y(other, "1"), published_at=early)], early)
