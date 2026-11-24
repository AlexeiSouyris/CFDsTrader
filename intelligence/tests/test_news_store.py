"""Guardado de noticias contra PostgreSQL real (#111).

Necesitan `DATABASE_URL` con las migraciones aplicadas (las aplica el motor). En CI corren en el
job de integración; sin base se saltean.
"""

import os
import uuid
from datetime import UTC, datetime, timedelta

import psycopg
import pytest

from cfds_intel.news import NewsItem
from cfds_intel.news.store import NewsConflictError, insert_news

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

AT = datetime(2026, 11, 20, 21, 15, tzinfo=UTC)


def news(tag: str, i: int, feed: str = "press_all") -> NewsItem:
    # Ids únicos por corrida: la tabla es de solo agregar y no se puede limpiar.
    return NewsItem(
        source="fed",
        feed=feed,
        item_id=f"https://www.federalreserve.gov/test/{tag}/{i}",
        title=f"Comunicado de prueba {i}",
        url=f"https://www.federalreserve.gov/test/{tag}/{i}",
        published_at=AT + timedelta(minutes=i),
        retrieved_at=AT + timedelta(hours=1),
    )


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def count(conn: psycopg.Connection, tag: str) -> int:
    row = conn.execute(
        "SELECT count(*) FROM news_items WHERE item_id LIKE %s", (f"%/test/{tag}/%",)
    ).fetchone()
    assert row is not None
    return int(row[0])


def test_news_are_stored_once_even_from_another_feed(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    batch = [news(tag, i) for i in range(3)]
    assert insert_news(conn, batch[:2]).inserted == 2
    again = insert_news(conn, [*batch, news(tag, 0, feed="press_monetary")])
    assert (again.inserted, again.unchanged) == (1, 3)
    assert count(conn, tag) == 3
    row = conn.execute(
        "SELECT published_at, feed FROM news_items WHERE item_id = %s", (batch[0].item_id,)
    ).fetchone()
    assert row == (AT, "press_all"), "hora exacta en UTC y el primer feed en el que se leyó"


def test_a_changed_news_item_is_a_conflict_and_the_batch_is_rolled_back(
    conn: psycopg.Connection,
) -> None:
    tag = uuid.uuid4().hex
    insert_news(conn, [news(tag, 0)])
    changed = news(tag, 0)
    changed = NewsItem(**{**changed.__dict__, "title": "Otro título"})
    with pytest.raises(NewsConflictError):
        insert_news(conn, [news(tag, 1), changed])
    assert count(conn, tag) == 1, "la noticia 1 del lote tampoco entró"


def test_the_database_enforces_append_only_and_publication_before_reading(
    conn: psycopg.Connection,
) -> None:
    tag = uuid.uuid4().hex
    insert_news(conn, [news(tag, 0)])
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute("DELETE FROM news_items WHERE item_id LIKE %s", (f"%/test/{tag}/%",))
    with pytest.raises(psycopg.errors.RaiseException):
        conn.execute(
            "UPDATE news_items SET title = 'x' WHERE item_id LIKE %s", (f"%/test/{tag}/%",)
        )
    early = NewsItem(**{**news(tag, 1).__dict__, "retrieved_at": AT - timedelta(hours=1)})
    with pytest.raises(psycopg.errors.CheckViolation):
        insert_news(conn, [early])
