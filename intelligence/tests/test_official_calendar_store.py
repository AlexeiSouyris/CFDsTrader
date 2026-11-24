"""Guardado del calendario oficial contra PostgreSQL real (ADR 0016, #520).

Necesitan `DATABASE_URL` con las migraciones aplicadas (las aplica el motor). En CI corren en el
job de integración; sin base se saltean. La tabla es de solo agregar: cada test usa un año al
azar del siglo XXII para no chocar con otra corrida.
"""

import os
import random
import uuid
from dataclasses import replace
from datetime import UTC, datetime, timedelta

import psycopg
import pytest

from cfds_intel.calendar.official import OfficialCalendarError, OfficialRelease
from cfds_intel.calendar.official.store import insert_releases, releases_known_at

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def release() -> OfficialRelease:
    day = datetime(random.randint(2100, 2999), 1, 1, tzinfo=UTC) + timedelta(  # noqa: S311
        days=random.randint(0, 360)  # noqa: S311
    )
    at = day.replace(hour=19)
    return OfficialRelease(
        source="fed_fomc",
        series="fomc_statement",
        reference_period=at.date().isoformat(),
        status="scheduled",
        scheduled_at=at,
        published_at=at - timedelta(days=200),
        time_rule="fomc_1400_from_2013_03",
        with_press_conference=True,
        source_url=f"https://www.federalreserve.gov/test/{uuid.uuid4().hex}.htm",
    )


def test_loading_twice_adds_nothing_and_a_different_row_is_an_error(
    conn: psycopg.Connection,
) -> None:
    r = release()
    assert insert_releases(conn, [r]).inserted == 1
    again = insert_releases(conn, [r])
    assert (again.inserted, again.unchanged) == (0, 1)
    with pytest.raises(OfficialCalendarError, match="no se pisa"):
        insert_releases(conn, [replace(r, published_at=r.published_at + timedelta(days=1))])


def test_nothing_is_known_before_its_published_at(conn: psycopg.Connection) -> None:
    r = release()
    assert r.scheduled_at is not None
    emergency = replace(
        r,
        status="unscheduled",
        scheduled_at=r.scheduled_at - timedelta(days=2),
        published_at=r.scheduled_at - timedelta(days=2),
        time_rule="release_page",
    )
    insert_releases(conn, [r, emergency])
    window = (r.scheduled_at - timedelta(days=10), r.scheduled_at + timedelta(days=1))

    def known(as_of: datetime) -> list[OfficialRelease]:
        rows = releases_known_at(conn, "fomc_statement", as_of, *window)
        return [x for x in rows if x.reference_period == r.reference_period]

    assert known(r.published_at - timedelta(seconds=1)) == []
    assert known(r.published_at) == [r]
    # La de emergencia aparece recién cuando se anunció.
    assert emergency.scheduled_at is not None
    before = known(emergency.scheduled_at - timedelta(seconds=1))
    assert [x.status for x in before] == ["scheduled"]
    assert [x.status for x in known(r.scheduled_at)] == ["unscheduled", "scheduled"]


def test_the_table_is_append_only_and_checks_the_contract(conn: psycopg.Connection) -> None:
    r = release()
    insert_releases(conn, [r])
    for sql in [
        "UPDATE official_releases SET time_rule = 'x' WHERE source_url = %s",
        "DELETE FROM official_releases WHERE source_url = %s",
    ]:
        with pytest.raises(psycopg.errors.RaiseException, match="solo agregar"):
            conn.execute(sql, (r.source_url,))
    columns = (
        "source, series, reference_period, status, scheduled_at, published_at, time_rule,"
        " with_press_conference, source_url"
    )
    assert r.scheduled_at is not None
    later = r.scheduled_at + timedelta(days=1)
    for bad in [
        # Se supo después de publicarse: look-ahead.
        ("fed_fomc", "fomc_statement", "2999-12-30", "scheduled", r.scheduled_at, later),
        # No programada conocida antes del anuncio.
        ("fed_fomc", "fomc_statement", "2999-12-30", "unscheduled", later, r.scheduled_at),
        # Cancelada con hora.
        ("fed_fomc", "fomc_statement", "2999-12-30", "cancelled", later, r.scheduled_at),
        # Serie de otro organismo.
        ("fed_fomc", "cpi", "2999-12", "scheduled", later, r.scheduled_at),
    ]:
        with pytest.raises(psycopg.errors.CheckViolation):
            conn.execute(
                f"INSERT INTO official_releases ({columns}) "  # noqa: S608 (texto fijo)
                "VALUES (%s, %s, %s, %s, %s, %s, 'release_page', true, %s)",
                (*bad, f"https://www.federalreserve.gov/test/{uuid.uuid4().hex}.htm"),
            )
