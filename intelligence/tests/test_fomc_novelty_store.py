"""Guardado de la novedad del FOMC contra PostgreSQL real (#668).

Necesitan `DATABASE_URL` con las migraciones aplicadas (las aplica el motor). En CI corren en el
job de integración; sin base se saltean. La tabla es de solo agregar: cada test usa un día al
azar del siglo XXII para no chocar con otra corrida.
"""

import os
import random
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from decimal import Decimal

import psycopg
import pytest

from cfds_intel.fomc_novelty import NoveltyError, NoveltyRow
from cfds_intel.fomc_novelty.store import insert_rows

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
pytestmark = pytest.mark.pg

PR = "https://www.federalreserve.gov/newsevents/pressreleases/monetary"


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


def row() -> NoveltyRow:
    at = datetime(random.randint(2100, 2999), 1, 1, 19, tzinfo=UTC) + timedelta(  # noqa: S311
        days=random.randint(60, 360)  # noqa: S311
    )
    prev = at - timedelta(days=42)
    return NoveltyRow(
        reference_period=at.date().isoformat(),
        status="scheduled",
        statement_url=f"{PR}{at:%Y%m%d}a.htm",
        available_at=at,
        previous_reference_period=prev.date().isoformat(),
        previous_statement_url=f"{PR}{prev:%Y%m%d}a.htm",
        method="word_diff",
        method_version="word_diff.v1",
        model=None,
        model_digest=None,
        value=Decimal("0.123456"),
        text_sha256="a" * 64,
        previous_text_sha256="b" * 64,
        word_count=300,
        previous_word_count=290,
        computed_at=datetime(2026, 11, 25, tzinfo=UTC),
    )


def test_loading_twice_adds_nothing_and_a_different_value_is_an_error(
    conn: psycopg.Connection,
) -> None:
    r = row()
    assert insert_rows(conn, [r]).inserted == 1
    # Otra corrida, más tarde: mismo resultado, no agrega nada.
    again = insert_rows(conn, [replace(r, computed_at=r.computed_at + timedelta(days=1))])
    assert (again.inserted, again.unchanged) == (0, 1)
    with pytest.raises(NoveltyError, match="no se pisa"):
        insert_rows(conn, [replace(r, value=Decimal("0.5"))])
    # Otro modelo u otra versión es otra fila.
    emb = replace(
        r,
        method="embedding_cosine",
        method_version="embedding_cosine.v1",
        model="nomic-embed-text",
        model_digest="abc123def456",
    )
    assert insert_rows(conn, [emb]).inserted == 1


def test_the_table_is_append_only(conn: psycopg.Connection) -> None:
    r = row()
    insert_rows(conn, [r])
    with pytest.raises(psycopg.errors.RaiseException, match="solo agregar"):
        conn.execute(
            "UPDATE fomc_statement_novelty SET value = 0 WHERE reference_period = %s",
            (r.reference_period,),
        )
