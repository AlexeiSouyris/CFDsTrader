"""Guardado y publicación de interpretaciones contra PostgreSQL y Redis reales (#186).

Necesitan `DATABASE_URL` con las migraciones aplicadas (las aplica el motor) y, el de Redis,
`REDIS_URL`. En CI corren en el job de integración; sin base se saltean. Nunca contra la base real:
en el Mac mini, la base de prueba `cfds_test`.
"""

import json
import os
import uuid
from collections.abc import Iterator
from datetime import UTC, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest
import redis

from cfds_intel.news import NewsItem
from cfds_intel.news.interpret import Impact, Interpretation, InterpreterInfo
from cfds_intel.news.interpretations import (
    InterpretationConflictError,
    RedisPublisher,
    pending,
    save,
)
from cfds_intel.news.store import insert_news

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

AT = datetime(2026, 11, 20, 21, 15, tzinfo=UTC)


class Sent:
    def __init__(self, fail: bool = False) -> None:
        self.docs: list[dict[str, object]] = []
        self.fail = fail

    def publish(self, doc: dict[str, object]) -> None:
        if self.fail:
            raise redis.ConnectionError("Redis caído")
        self.docs.append(doc)


@pytest.fixture
def conn() -> Iterator[psycopg.Connection]:
    with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as c:
        yield c


def info(tag: str, model: str = "modelo@1") -> InterpreterInfo:
    # Versión única por corrida: la tabla es de solo agregar y no se puede limpiar.
    return InterpreterInfo("test", tag[:16], model, "c" * 64)


def news(conn: psycopg.Connection, tag: str, n: int) -> list[NewsItem]:
    items = [
        NewsItem(
            "fed",
            "press_all",
            f"https://www.federalreserve.gov/test/interp/{tag}/{i}",
            f"Comunicado {i}",
            f"https://www.federalreserve.gov/test/interp/{tag}/{i}",
            AT + timedelta(minutes=i),
            AT + timedelta(hours=1),
        )
        for i in range(n)
    ]
    insert_news(conn, items)
    return items


def interp(item: NewsItem, who: InterpreterInfo, summary: str = "Resumen.") -> Interpretation:
    return Interpretation(
        item.source,
        item.item_id,
        item.published_at,
        item.retrieved_at,
        who,
        summary,
        (Impact("DE40", Decimal("-0.25"), Decimal("0.8")),),
    )


def test_saved_once_published_once_and_no_longer_pending(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    who = info(tag)
    a, b = news(conn, tag, 2)
    mine = {a.item_id, b.item_id}
    assert {i.item_id for i in pending(conn, who)} >= mine
    sent = Sent()
    assert save(conn, interp(a, who), sent) is True
    # Otra corrida da otro resumen (Ollama con Metal): se conserva el primero, no se publica.
    assert save(conn, interp(a, who, "Otro resumen."), sent) is False
    assert [d["item_id"] for d in sent.docs] == [a.item_id]
    left = {i.item_id for i in pending(conn, who)} & mine
    assert left == {b.item_id}
    row = conn.execute(
        "SELECT summary_es, impacts, interpreted_at FROM news_interpretations"
        " WHERE source = %s AND item_id = %s AND interpreter_version = %s",
        (a.source, a.item_id, who.version),
    ).fetchone()
    assert row == (
        "Resumen.",
        [{"instrument_id": "DE40", "sentiment": "-0.25", "relevance": "0.8"}],
        a.retrieved_at,
    )


def test_same_version_with_another_model_is_a_conflict(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    [a] = news(conn, tag, 1)
    save(conn, interp(a, info(tag)), Sent())
    with pytest.raises(InterpretationConflictError, match="cambiá la versión"):
        save(conn, interp(a, info(tag, model="otro@2")), Sent())


def test_if_redis_fails_nothing_is_saved(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    who = info(tag)
    [a] = news(conn, tag, 1)
    with pytest.raises(redis.ConnectionError):
        save(conn, interp(a, who), Sent(fail=True))
    assert a.item_id in {i.item_id for i in pending(conn, who)}


def test_the_table_is_append_only_and_checks_the_rules(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    who = info(tag)
    [a, b] = news(conn, tag, 2)
    save(conn, interp(a, who), Sent())
    with pytest.raises(psycopg.errors.RaiseException, match="solo agregar"):
        conn.execute(
            "UPDATE news_interpretations SET summary_es = 'x' WHERE interpreter_version = %s",
            (who.version,),
        )
    with pytest.raises(psycopg.errors.RaiseException, match="solo agregar"):
        conn.execute(
            "DELETE FROM news_interpretations WHERE interpreter_version = %s", (who.version,)
        )
    early = interp(b, who)
    early = Interpretation(
        early.source,
        early.item_id,
        early.published_at,
        early.published_at - timedelta(seconds=1),
        who,
        early.summary_es,
        early.impacts,
    )
    with pytest.raises(psycopg.errors.CheckViolation, match="interpreted_after_published"):
        save(conn, early, Sent())
    unknown = NewsItem("fed", "press_all", f"no-guardada-{tag}", "x", "https://x", AT, AT)
    with pytest.raises(psycopg.errors.ForeignKeyViolation):
        save(conn, interp(unknown, who), Sent())


@pytest.mark.skipif("REDIS_URL" not in os.environ, reason="necesita Redis (REDIS_URL)")
def test_redis_stream_gets_the_contract_json(conn: psycopg.Connection) -> None:
    tag = uuid.uuid4().hex
    who = info(tag)
    [a] = news(conn, tag, 1)
    stream = f"test.news_interpretations.v1.{tag}"
    pub = RedisPublisher(os.environ["REDIS_URL"], stream=stream)
    r = redis.Redis.from_url(os.environ["REDIS_URL"], decode_responses=True)
    try:
        pub.ping()
        it = interp(a, who)
        assert save(conn, it, pub) is True
        assert save(conn, it, pub) is False
        entries = r.xrange(stream)
        assert entries is not None and len(entries) == 1
        _, fields = entries[0]
        assert fields is not None
        assert fields["interpretation_id"] == it.interpretation_id
        assert json.loads(fields["data"]) == it.to_contract()
    finally:
        r.delete(stream)
