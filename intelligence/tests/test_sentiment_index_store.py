"""Índice de sentimiento contra PostgreSQL real (#213): lectura de los datos crudos y guardado.

Necesitan `DATABASE_URL` con las migraciones aplicadas. En CI corren en el job de integración.
"""

import os
import random
from datetime import UTC, date, datetime, time, timedelta
from decimal import Decimal
from pathlib import Path

import psycopg
import pytest

from cfds_intel.sentiment.compute_index import daily_windows
from cfds_intel.sentiment.index import (
    COT_LONG,
    COT_OPEN_INTEREST,
    COT_SHORT,
    SentimentIndex,
    index_series,
    load_config,
)
from cfds_intel.sentiment.index_store import (
    SentimentRepaintError,
    db_value,
    insert_index,
    load_points,
)

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

ROOT = Path(__file__).resolve().parents[2]
CFG = load_config((ROOT / "config" / "sentiment.toml").read_text())


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def instrument(conn: psycopg.Connection) -> str:
    """Las tablas son de solo agregar: un instrumento propio por corrida."""
    iid = f"ITSI{random.randrange(10**9)}"  # noqa: S311 (no es criptografía)
    conn.execute(
        """
        INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone)
        VALUES (%s, 'prueba', 'index', 'USD', 'America/New_York')
        """,
        (iid,),
    )
    return iid


def fill(conn: psycopg.Connection, iid: str, days: int) -> None:
    """`days` días de los tres componentes, con valores que suben y bajan."""
    start = date(2020, 1, 6)
    for i in range(days):
        d = start + timedelta(days=i)
        end = datetime.combine(d, time(23), UTC)
        wave = Decimal((i * 37) % 101)
        conn.execute(
            """
            INSERT INTO daily_indicators (instrument_id, day, published_at, atr_period, atr,
                realized_vol_window, realized_vol)
            VALUES (%s, %s, %s, 14, 1, 20, %s)
            """,
            (iid, d, end, wave / 1000),
        )
        conn.execute(
            """
            INSERT INTO daily_drawdowns
                (instrument_id, day, published_at, drawdown_window, drawdown)
            VALUES (%s, %s, %s, 252, %s)
            """,
            (iid, d, end, -wave / 500),
        )
        if i % 7 == 0:
            observed = datetime.combine(d, time(0), UTC)
            for series, value in (
                (COT_LONG, 1000 + wave),
                (COT_SHORT, 900),
                (COT_OPEN_INTEREST, 5000),
            ):
                conn.execute(
                    """
                    INSERT INTO sentiment_observations
                        (source, series, instrument_id, observed_for, published_at, value)
                    VALUES ('cftc_cot', %s, %s, %s, %s, %s)
                    """,
                    (series, iid, observed, observed + timedelta(days=7, hours=20), value),
                )


def test_reads_raw_values_builds_and_stores_the_index_without_repainting(
    conn: psycopg.Connection,
) -> None:
    iid = instrument(conn)
    fill(conn, iid, 1200)
    vol_window, atr_period, dd_window = daily_windows(
        (ROOT / "config" / "indicators.toml").read_text()
    )
    data = load_points(conn, iid, vol_window, atr_period, dd_window)
    assert {k: len(v) for k, v in data.items()} == {
        "cot": 172,
        "realized_vol": 1200,
        "drawdown": 1200,
    }
    emitted, _ = index_series(iid, data, CFG)
    assert emitted and all(isinstance(r, SentimentIndex) for r in emitted)

    first = insert_index(conn, emitted)
    again = insert_index(conn, emitted)
    assert (first.inserted, first.unchanged) == (len(emitted), 0)
    assert (again.inserted, again.unchanged) == (0, len(emitted))

    # Otro valor para un índice ya guardado es repintado: error, y no se guarda nada del lote.
    fake = SentimentIndex(**{**emitted[0].__dict__, "value": emitted[0].value / 2})
    with pytest.raises(SentimentRepaintError):
        insert_index(conn, [fake])
    n = conn.execute("SELECT count(*) FROM sentiment_index WHERE instrument_id = %s", (iid,))
    assert n.fetchone() == (len(emitted),)
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("DELETE FROM sentiment_index WHERE instrument_id = %s", (iid,))


def test_without_three_years_nothing_is_emitted(conn: psycopg.Connection) -> None:
    iid = instrument(conn)
    fill(conn, iid, 300)
    data = load_points(conn, iid, 20, 14, 252)
    emitted, skipped = index_series(iid, data, CFG)
    assert emitted == []
    assert "0 componentes válidos de 3" in skipped[-1].reason()


def test_db_value_keeps_at_most_28_decimals() -> None:
    # rust_decimal, con el que el motor lee la tabla, admite hasta 28 decimales (#372).
    long = Decimal("-0.00004546281142025822876886693333")
    assert db_value(long) == Decimal("-0.0000454628114202582287688669")
    assert db_value(long).as_tuple().exponent == -28
    assert db_value(Decimal("-0.35")) == Decimal("-0.35")
    assert db_value(Decimal("1")).as_tuple().exponent == 0
