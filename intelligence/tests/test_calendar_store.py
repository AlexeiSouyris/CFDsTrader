"""Guardado del calendario económico contra PostgreSQL real (#239).

Necesitan `DATABASE_URL` con las migraciones aplicadas (las aplica el motor). En CI corren en el
job de integración; sin base se saltean. La tabla es de solo agregar: nombres únicos por corrida.
"""

import os
import uuid
from dataclasses import replace
from datetime import UTC, datetime, timedelta

import psycopg
import pytest

from cfds_intel.calendar import EconomicEvent
from cfds_intel.calendar.store import events_known_at, insert_events

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

SEEN = datetime(2026, 11, 23, 10, 0, 7, tzinfo=UTC)
AT = datetime(2026, 11, 27, 13, 30, tzinfo=UTC)


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def event(tag: str, **over: object) -> EconomicEvent:
    base = EconomicEvent(
        source="forex_factory",
        event=f"Evento de prueba {tag}",
        country="US",
        impact="high",
        scheduled_at=AT,
        published_at=SEEN,
    )
    return replace(base, **over)  # type: ignore[arg-type]


def mine(conn: psycopg.Connection, tag: str, as_of: datetime) -> list[EconomicEvent]:
    found = events_known_at(
        conn, "forex_factory", as_of, AT - timedelta(days=7), AT + timedelta(days=7)
    )
    return [e for e in found if e.event.endswith(tag)]


def test_reading_again_keeps_the_first_published_at(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    first = insert_events(conn, [event(tag)])
    later = event(tag, published_at=SEEN + timedelta(hours=3))
    again = insert_events(conn, [later, later])
    assert (first.inserted, again.inserted, again.unchanged) == (1, 0, 2)
    assert [e.published_at for e in mine(conn, tag, AT)] == [SEEN]


def test_a_change_of_time_or_impact_is_a_new_event_from_when_it_was_seen(
    conn: psycopg.Connection,
) -> None:
    tag = uuid.uuid4().hex
    insert_events(conn, [event(tag)])
    moved = event(
        tag, scheduled_at=AT + timedelta(minutes=30), published_at=SEEN + timedelta(hours=6)
    )
    upgraded = event(tag, impact="medium", published_at=SEEN + timedelta(hours=9))
    assert insert_events(conn, [moved, upgraded]).inserted == 2
    # Sin look-ahead: antes de verlos, los cambios no existen.
    assert mine(conn, tag, SEEN - timedelta(seconds=1)) == []
    assert [e.scheduled_at for e in mine(conn, tag, SEEN + timedelta(hours=1))] == [AT]
    assert len(mine(conn, tag, SEEN + timedelta(hours=10))) == 3


def test_the_table_is_append_only_and_checks_the_contract(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    insert_events(conn, [event(tag)])
    for sql in [
        "UPDATE economic_events SET impact = 'low' WHERE event LIKE %s",
        "DELETE FROM economic_events WHERE event LIKE %s",
    ]:
        with pytest.raises(psycopg.Error):
            conn.execute(sql, (f"%{tag}",))
    for bad in [
        event(tag, impact="holiday"),
        event(tag, country="USA"),
        event(tag, source="Forex Factory"),
        event(tag, event=""),
    ]:
        with pytest.raises(psycopg.Error):
            insert_events(conn, [bad])
    with pytest.raises(psycopg.Error):
        conn.execute(
            "INSERT INTO economic_events (source, event, country, instrument_id, impact,"
            " scheduled_at, published_at) VALUES ('forex_factory', %s, NULL, NULL, 'high', %s, %s)",
            (f"Sin país {tag}", AT, SEEN),
        )


def test_a_bad_event_saves_nothing_of_its_batch(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    with pytest.raises(psycopg.Error):
        insert_events(conn, [event(tag), event(tag, impact="extreme")])
    assert mine(conn, tag, AT) == []
