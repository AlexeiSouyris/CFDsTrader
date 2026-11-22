"""Interpretación de noticias como evento (#122), con un intérprete grabado y sin red."""

import json
from collections.abc import Sequence
from dataclasses import dataclass, field, replace
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.news import NewsItem
from cfds_intel.news.interpret import (
    Impact,
    InterpretationError,
    InterpreterInfo,
    Reading,
    instrument_ids,
    interpret,
)

ROOT = Path(__file__).resolve().parents[2]
PUBLISHED = datetime(2026, 11, 19, 12, 45, tzinfo=UTC)
NOW = PUBLISHED + timedelta(minutes=3)
INSTRUMENTS = ("DE40", "US100", "US30", "US500")
ITEM = NewsItem(
    source="ecb",
    feed="press",
    item_id="https://www.ecb.europa.eu/press/x.html",
    title="Monetary policy decisions",
    url="https://www.ecb.europa.eu/press/x.html",
    published_at=PUBLISHED,
    retrieved_at=PUBLISHED + timedelta(minutes=1),
)
INFO = InterpreterInfo("recorded", "1.0.0", model=None, prompt_sha256=None)
GOOD = Reading(
    "El BCE mantiene los tipos.",
    (Impact("DE40", Decimal("-0.25"), Decimal("0.8")), Impact("US100", Decimal("0"), Decimal("0"))),
)


@dataclass
class Recorded:
    """Devuelve una respuesta grabada, como haría un modelo, y anota qué le preguntaron."""

    reading: Reading
    info: InterpreterInfo = INFO
    calls: list[tuple[str, tuple[str, ...]]] = field(default_factory=list)

    def read(self, item: NewsItem, instruments: Sequence[str]) -> Reading:
        self.calls.append((item.item_id, tuple(instruments)))
        return self.reading


def schema() -> Draft202012Validator:
    doc = json.loads((ROOT / "contracts" / "news_interpretation.v1.schema.json").read_text())
    return Draft202012Validator(doc)


def test_event_follows_the_contract_and_records_the_interpreter() -> None:
    rec = Recorded(GOOD)
    doc = interpret(ITEM, rec, INSTRUMENTS, NOW).to_contract()
    assert list(schema().iter_errors(doc)) == []
    assert doc["published_at"] == "2026-11-19T12:45:00Z"
    assert doc["interpreted_at"] == "2026-11-19T12:48:00Z"
    assert doc["interpreter"] == {
        "name": "recorded",
        "version": "1.0.0",
        "model": None,
        "prompt_sha256": None,
    }
    assert doc["impacts"] == [
        {"instrument_id": "DE40", "sentiment": "-0.25", "relevance": "0.8"},
        {"instrument_id": "US100", "sentiment": "0", "relevance": "0"},
    ]
    assert rec.calls == [(ITEM.item_id, INSTRUMENTS)]


def test_the_score_adjustment_is_not_part_of_the_event() -> None:
    doc = interpret(ITEM, Recorded(GOOD), INSTRUMENTS, NOW).to_contract()
    assert "adjustment" not in json.dumps(doc)


def test_id_is_idempotent_and_changes_with_the_interpreter_version() -> None:
    a = interpret(ITEM, Recorded(GOOD), INSTRUMENTS, NOW)
    b = interpret(ITEM, Recorded(GOOD), INSTRUMENTS, NOW + timedelta(hours=1))
    assert a.interpretation_id == b.interpretation_id
    v2 = Recorded(GOOD, info=replace(INFO, version="1.0.1"))
    assert interpret(ITEM, v2, INSTRUMENTS, NOW).interpretation_id != a.interpretation_id


def test_no_instrument_affected_is_a_valid_event() -> None:
    doc = interpret(ITEM, Recorded(Reading("Sin impacto.", ())), INSTRUMENTS, NOW).to_contract()
    assert doc["impacts"] == []
    assert list(schema().iter_errors(doc)) == []


def test_extremes_and_exact_decimals_are_kept() -> None:
    reading = Reading(
        "Extremos.",
        (
            Impact("DE40", Decimal("-1"), Decimal("1.0000")),
            Impact("US30", Decimal("1"), Decimal("0.0001")),
            Impact("US500", Decimal("0E+2"), Decimal("0")),
        ),
    )
    doc = interpret(ITEM, Recorded(reading), INSTRUMENTS, NOW).to_contract()
    assert list(schema().iter_errors(doc)) == []
    assert [(i["sentiment"], i["relevance"]) for i in doc["impacts"]] == [  # type: ignore[attr-defined]
        ("-1", "1.0000"),
        ("1", "0.0001"),
        ("0", "0"),
    ]


def test_never_before_publication() -> None:
    with pytest.raises(InterpretationError, match="antes de su publicación"):
        interpret(ITEM, Recorded(GOOD), INSTRUMENTS, PUBLISHED - timedelta(seconds=1))
    at_publication = interpret(ITEM, Recorded(GOOD), INSTRUMENTS, PUBLISHED)
    assert at_publication.interpreted_at == PUBLISHED


def test_now_needs_a_time_zone() -> None:
    with pytest.raises(InterpretationError, match="zona horaria"):
        interpret(ITEM, Recorded(GOOD), INSTRUMENTS, datetime(2026, 11, 20, 12, 0))


@pytest.mark.parametrize(
    ("impact", "message"),
    [
        (Impact("DE40", Decimal("1.0001"), Decimal("0.5")), "fuera de"),
        (Impact("DE40", Decimal("-1.5"), Decimal("0.5")), "fuera de"),
        (Impact("DE40", Decimal("0.5"), Decimal("-0.1")), "fuera de"),
        (Impact("DE40", Decimal("0.5"), Decimal("1.1")), "fuera de"),
        (Impact("DE40", Decimal("0.12345"), Decimal("0.5")), "decimales"),
        (Impact("DE40", Decimal("NaN"), Decimal("0.5")), "finito"),
        (Impact("DE40", Decimal("0.5"), Decimal("Infinity")), "finito"),
        (Impact("DE40", 0.5, Decimal("0.5")), "Decimal"),  # type: ignore[arg-type]
        (Impact("DE40", Decimal("0.5"), 1), "Decimal"),  # type: ignore[arg-type]
        (Impact("AAPL", Decimal("0.5"), Decimal("0.5")), "no configurado"),
    ],
)
def test_bad_values_reject_the_whole_interpretation(impact: Impact, message: str) -> None:
    with pytest.raises(InterpretationError, match=message):
        interpret(ITEM, Recorded(Reading("Resumen.", (impact,))), INSTRUMENTS, NOW)


def test_repeated_instrument_is_rejected() -> None:
    imp = Impact("DE40", Decimal("0.1"), Decimal("0.1"))
    with pytest.raises(InterpretationError, match="repetido"):
        interpret(ITEM, Recorded(Reading("Resumen.", (imp, imp))), INSTRUMENTS, NOW)


@pytest.mark.parametrize("summary", ["", "   ", "x" * 1001])
def test_summary_is_required_and_bounded(summary: str) -> None:
    with pytest.raises(InterpretationError, match="summary_es"):
        interpret(ITEM, Recorded(Reading(summary, ())), INSTRUMENTS, NOW)


@pytest.mark.parametrize(
    "kwargs",
    [
        {"name": "Con Mayúsculas"},
        {"version": ""},
        {"model": ""},
        {"model": "m" * 201},
        {"prompt_sha256": "abc"},
    ],
)
def test_interpreter_info_is_validated(kwargs: dict[str, str]) -> None:
    base: dict[str, str | None] = {
        "name": "local",
        "version": "1",
        "model": None,
        "prompt_sha256": None,
    }
    with pytest.raises(InterpretationError):
        InterpreterInfo(**{**base, **kwargs})  # type: ignore[arg-type]


def test_model_and_prompt_are_recorded_when_used() -> None:
    info = InterpreterInfo("local-llm", "0.1.0", "modelo-abierto@sha256:ab12", "0" * 64)
    doc = interpret(ITEM, Recorded(GOOD, info=info), INSTRUMENTS, NOW).to_contract()
    assert list(schema().iter_errors(doc)) == []
    assert doc["interpreter"] == {
        "name": "local-llm",
        "version": "0.1.0",
        "model": "modelo-abierto@sha256:ab12",
        "prompt_sha256": "0" * 64,
    }


def test_contract_rejects_what_the_code_never_produces() -> None:
    good = interpret(ITEM, Recorded(GOOD), INSTRUMENTS, NOW).to_contract()
    v = schema()
    for bad_value in ["1.5", "-1.0001", "0.12345", "0.5e1", ".5", "+0.5"]:
        doc = json.loads(json.dumps(good))
        doc["impacts"][0]["sentiment"] = bad_value
        assert list(v.iter_errors(doc)), bad_value
    for bad_rel in ["-0.1", "-0", "1.01"]:
        doc = json.loads(json.dumps(good))
        doc["impacts"][0]["relevance"] = bad_rel
        assert list(v.iter_errors(doc)), bad_rel
    doc = json.loads(json.dumps(good))
    doc["impacts"][0]["sentiment"] = 0.5
    assert list(v.iter_errors(doc))
    doc = json.loads(json.dumps(good))
    doc["adjustment"] = 1
    assert list(v.iter_errors(doc))


def test_repository_instruments_are_read_from_config() -> None:
    ids = instrument_ids((ROOT / "config" / "instruments.toml").read_text())
    assert set(INSTRUMENTS) <= set(ids)
    with pytest.raises(InterpretationError):
        instrument_ids("[account]\ncurrency = 'EUR'\n")
