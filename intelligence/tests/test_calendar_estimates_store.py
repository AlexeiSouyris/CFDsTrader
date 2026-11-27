"""Guardado de las instantáneas de `forecast` y `previous` contra PostgreSQL (ADR 0018, #696).

Necesitan `DATABASE_URL` de una base `cfds_test*` con las migraciones aplicadas; sin base se
saltean. La tabla es de solo agregar: nombres de evento únicos por corrida.
"""

import os
import uuid
from datetime import UTC, datetime, timedelta

import psycopg
import pytest

from cfds_intel.calendar import CalendarEstimate, build_estimate
from cfds_intel.calendar.store import estimates_of, insert_estimates

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

AT = datetime(2026, 12, 10, 13, 30, tzinfo=UTC)
T0 = datetime(2026, 12, 7, 10, 0, 7, tzinfo=UTC)
H = timedelta(hours=1)


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def snap(tag: str, forecast: str | None, previous: str | None, seen: datetime) -> CalendarEstimate:
    return build_estimate("forex_factory", f"CPI {tag}", "US", AT, forecast, previous, seen)


def stored(conn: psycopg.Connection, tag: str) -> list[tuple[str | None, str | None, datetime]]:
    return [
        (e.forecast_raw, e.previous_raw, e.seen_at)
        for e in estimates_of(conn, "forex_factory", f"CPI {tag}", "US", AT)
    ]


def test_a_read_without_changes_adds_nothing(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    first = insert_estimates(conn, [snap(tag, "0.3%", "0.2%", T0)])
    again = insert_estimates(conn, [snap(tag, "0.3%", "0.2%", T0 + H)])
    assert (first.inserted, again.inserted, again.unchanged) == (1, 0, 1)
    assert stored(conn, tag) == [("0.3%", "0.2%", T0)]  # conserva la primera lectura


def test_a_change_and_back_are_three_snapshots(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    for i, f in enumerate(["0.3%", "0.4%", "0.4%", "0.3%"]):
        insert_estimates(conn, [snap(tag, f, "0.2%", T0 + i * H)])
    assert stored(conn, tag) == [
        ("0.3%", "0.2%", T0),
        ("0.4%", "0.2%", T0 + H),
        ("0.3%", "0.2%", T0 + 3 * H),
    ]


def test_values_and_unit_are_exact(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    insert_estimates(conn, [snap(tag, "4.50%", "-0.1%", T0)])
    (e,) = estimates_of(conn, "forex_factory", f"CPI {tag}", "US", AT)
    assert (str(e.forecast_value), str(e.previous_value), e.unit) == ("4.50", "-0.1", "percent")


def test_empty_values_are_stored_only_as_a_change(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    insert_estimates(conn, [snap(tag, None, None, T0)])  # nada que decir: no se guarda
    insert_estimates(conn, [snap(tag, "0.3%", None, T0 + H)])
    insert_estimates(conn, [snap(tag, None, None, T0 + 2 * H)])  # lo quitaron: es un cambio
    assert stored(conn, tag) == [("0.3%", None, T0 + H), (None, None, T0 + 2 * H)]


def test_snapshots_after_the_event_are_kept_too(conn: psycopg.Connection) -> None:
    # Se guardan (sirven para ver revisiones del previous); no cuentan como consenso, y eso lo
    # decide quien calcula la sorpresa (Rust), con seen_at < scheduled_at.
    tag = uuid.uuid4().hex
    insert_estimates(conn, [snap(tag, "0.3%", "0.2%", AT - H)])
    insert_estimates(conn, [snap(tag, "0.3%", "0.1%", AT + H)])
    assert [s[2] for s in stored(conn, tag)] == [AT - H, AT + H]


def test_the_table_is_append_only(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    insert_estimates(conn, [snap(tag, "0.3%", "0.2%", T0)])
    with pytest.raises(psycopg.errors.RaiseException, match="solo agregar"):
        conn.execute(
            "UPDATE calendar_estimates SET forecast_raw = %s WHERE event = %s", ("9%", f"CPI {tag}")
        )
