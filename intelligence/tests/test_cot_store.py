"""Guardado del COT en `sentiment_observations` contra PostgreSQL real (#113).

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import os
import uuid
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.sentiment.cot import Observation
from cfds_intel.sentiment.store import insert_observations

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg


@pytest.fixture
def conn() -> psycopg.Connection:
    c = psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)
    # El instrumento tiene que existir (clave foránea).
    c.execute(
        """
        INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone)
        VALUES ('US100', 'US Tech 100', 'index', 'USD', 'America/New_York')
        ON CONFLICT (id) DO NOTHING
        """
    )
    return c


# Martes fijo: la serie es la que cambia en cada test (#488).
TUESDAY = datetime(2024, 1, 2, tzinfo=UTC)


@pytest.fixture
def series() -> str:
    # La tabla es de solo agregar y no se puede limpiar: cada test usa una serie propia, que
    # nunca tiene filas previas (#488). Antes se elegía una fecha al azar, que a veces repetía
    # una fila de otra corrida. Cumple `^[a-z][a-z0-9_]{0,31}$` (32 caracteres).
    return "t" + uuid.uuid4().hex[:31]


def obs(series: str, observed: datetime, value: str) -> Observation:
    return Observation(
        series=series,
        instrument_id="US100",
        observed_for=observed,
        published_at=observed + timedelta(days=7, hours=20),
        value=Decimal(value),
    )


def rows(
    conn: psycopg.Connection, series: str, observed: datetime
) -> list[tuple[datetime, Decimal]]:
    return [
        (r[0], r[1])
        for r in conn.execute(
            """
            SELECT published_at, value FROM sentiment_observations
            WHERE source = 'cftc_cot' AND series = %s AND instrument_id = 'US100'
                AND observed_for = %s
            ORDER BY published_at
            """,
            (series, observed),
        ).fetchall()
    ]


def test_a_repeated_value_is_not_duplicated_even_with_another_publication_time(
    conn: psycopg.Connection, series: str
) -> None:
    t = TUESDAY
    assert rows(conn, series, t) == []
    first = obs(series, t, "314318")
    assert insert_observations(conn, [first], t + timedelta(days=30)).inserted == 1
    # Leído de nuevo en vivo, antes: otra published_at calculada, mismo valor → no se repite.
    again = replace(first, published_at=t + timedelta(days=3))
    r = insert_observations(conn, [again], t + timedelta(days=3))
    assert (r.inserted, r.unchanged, r.corrections) == (0, 1, 0)
    assert rows(conn, series, t) == [(first.published_at, Decimal("314318"))]


def test_a_correction_is_a_new_row_published_when_it_was_read(
    conn: psycopg.Connection, series: str
) -> None:
    t = TUESDAY
    assert rows(conn, series, t) == []
    insert_observations(conn, [obs(series, t, "100")], t + timedelta(days=30))
    read_at = t + timedelta(days=40)
    r = insert_observations(conn, [obs(series, t, "101")], read_at)
    assert r.corrections == 1
    assert rows(conn, series, t) == [
        (t + timedelta(days=7, hours=20), Decimal("100")),
        (read_at, Decimal("101")),
    ]
