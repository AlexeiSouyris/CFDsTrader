"""Estados emocionales del índice como `emotional_state.v1` (#220).

Los primeros tests no usan base ni red. Los de guardado necesitan `DATABASE_URL` con las
migraciones aplicadas (en CI corren en el job de integración).
"""

import json
import os
import uuid
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import jsonschema
import psycopg
import pytest

from cfds_intel.sentiment.index import ComponentState, NotEmitted, SentimentIndex
from cfds_intel.sentiment.states import index_state_values, load_states_config
from cfds_intel.sentiment.states_store import (
    EmotionRepaintError,
    StatesRow,
    insert_states,
    states_rows,
)

ROOT = Path(__file__).resolve().parents[2]
CFG = load_states_config((ROOT / "config" / "sentiment.toml").read_text())
SCHEMA = json.loads((ROOT / "contracts" / "emotional_state.v1.schema.json").read_text())
T = datetime(2026, 11, 20, 23, tzinfo=UTC)

# Escribe en PostgreSQL: solo contra una base cfds_test* (guardia en conftest.py, #376).
needs_db = pytest.mark.pg


def vol(pct: str) -> ComponentState:
    return ComponentState("realized_vol", "valid", Decimal(1), None, Decimal(pct), None)


def index(iid: str, value: str, at: datetime, pct: str = "0.5") -> SentimentIndex:
    return SentimentIndex(iid, at, at, Decimal(value), True, 3, (vol(pct),))


def test_states_carry_the_values_of_their_rule() -> None:
    assert index_state_values(index("US500", "-0.72", T), CFG) == (
        ("extreme_fear", {"index": "-0.72", "threshold": "-0.6"}),
    )
    assert index_state_values(index("US500", "0.65", T, pct="0.05"), CFG) == (
        ("extreme_euphoria", {"index": "0.65", "threshold": "0.6"}),
        ("complacency", {"percentile": "0.05", "threshold": "0.1"}),
    )
    assert index_state_values(index("US500", "0.1", T), CFG) == ()


def test_one_record_per_moment_valid_against_the_contract() -> None:
    emitted = [index("US500", "-0.7", T), index("US500", "0.2", T + timedelta(days=1))]
    # Un momento sin índice (solo puede haber complacencia) y otro que coincide con un índice.
    skipped = [
        NotEmitted("US500", T - timedelta(days=1), 1, 3, (vol("0.03"),)),
        NotEmitted("US500", T, 1, 3, (vol("0.03"),)),
    ]
    rows = states_rows(emitted, skipped, CFG)
    assert [r.as_of for r in rows] == [T - timedelta(days=1), T, T + timedelta(days=1)]
    assert [tuple(n for n, _ in r.states) for r in rows] == [
        ("complacency",),
        ("extreme_fear",),  # en T vale el índice
        (),
    ]
    for r in rows:
        doc = r.to_contract()
        jsonschema.validate(doc, SCHEMA)
        assert doc["source"] == "sentiment"
        assert doc["basis"] == "sentiment_index"
    assert rows[0].to_contract()["as_of"] == "2026-11-19T23:00:00Z"


@pytest.fixture
def conn() -> psycopg.Connection:
    return psycopg.connect(os.environ["DATABASE_URL"], autocommit=True)


@needs_db
def test_saving_is_idempotent_and_never_repaints(conn: psycopg.Connection) -> None:
    # Tabla de solo agregar: un instrumento propio por test, con la fecha fija T (#488).
    # Cumple `^[A-Z0-9][A-Z0-9._-]{0,31}$` (32 caracteres).
    iid = "ITEMO" + uuid.uuid4().hex[:27].upper()
    before = conn.execute("SELECT count(*) FROM emotional_states WHERE instrument_id = %s", (iid,))
    assert before.fetchone() == (0,)
    rows = states_rows([index(iid, "-0.7", T), index(iid, "0.1", T + timedelta(hours=1))], [], CFG)
    first = insert_states(conn, rows)
    assert (first.inserted, first.unchanged) == (2, 0)
    again = insert_states(conn, rows)
    assert (again.inserted, again.unchanged) == (0, 2)
    other = StatesRow(iid, T, (("extreme_fear", {"index": "-0.8", "threshold": "-0.6"}),))
    new_one = StatesRow(iid, T + timedelta(hours=2), ())
    with pytest.raises(EmotionRepaintError):
        insert_states(conn, [new_one, other])
    stored = conn.execute(
        "SELECT as_of, states FROM emotional_states WHERE instrument_id = %s ORDER BY as_of", (iid,)
    ).fetchall()
    assert [s[0] for s in stored] == [T, T + timedelta(hours=1)]
    assert stored[0][1] == [
        {"state": "extreme_fear", "values": {"index": "-0.7", "threshold": "-0.6"}}
    ]
