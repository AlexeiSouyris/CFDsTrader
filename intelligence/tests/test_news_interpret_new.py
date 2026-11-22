"""Comando que interpreta las noticias nuevas (#186). Sin red, Ollama, PostgreSQL ni Redis."""

import json
import urllib.error
from collections.abc import Callable, Sequence
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any

import jsonschema
import psycopg
import pytest
import redis

from cfds_intel.news import NewsItem
from cfds_intel.news.fulltext import FullTextError
from cfds_intel.news.interpret import (
    Impact,
    Interpretation,
    InterpreterInfo,
    Reading,
)
from cfds_intel.news.interpret_new import AbortError, main, run
from cfds_intel.news.interpretations import RedisPublisher
from cfds_intel.news.ollama import OllamaError

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads((ROOT / "contracts" / "news_interpretation.v1.schema.json").read_text())
INFO = InterpreterInfo("ollama", "p2", "modelo@" + "a" * 64, "b" * 64)
INSTRUMENTS = ("DE40", "US100")
AT = datetime(2026, 11, 20, 13, 0, tzinfo=UTC)
NOW = AT + timedelta(hours=1)


def item(i: int) -> NewsItem:
    url = f"https://www.federalreserve.gov/n/{i}.htm"
    return NewsItem("fed", "press_all", url, f"Titular {i}", url, AT + timedelta(minutes=i), NOW)


class Fake:
    """Intérprete grabado: por item_id devuelve una lectura o lanza una excepción."""

    def __init__(self, out: dict[str, Reading | Exception] | None = None) -> None:
        self.out = out or {}
        self.calls: list[str] = []

    @property
    def info(self) -> InterpreterInfo:
        return INFO

    def read(self, news: NewsItem, instruments: Sequence[str]) -> Reading:
        self.calls.append(news.item_id)
        r = self.out.get(
            news.item_id, Reading("Resumen.", (Impact("DE40", Decimal("0.5"), Decimal("1")),))
        )
        if isinstance(r, Exception):
            raise r
        return r


def collect(new: bool = True) -> tuple[list[Interpretation], Callable[[Interpretation], bool]]:
    stored: list[Interpretation] = []

    def store(i: Interpretation) -> bool:
        stored.append(i)
        return new

    return stored, store


def test_each_new_item_is_interpreted_and_stored_as_the_contract() -> None:
    stored, store = collect()
    r = run([item(1), item(2)], Fake(), INSTRUMENTS, store, now=lambda: NOW)
    assert (r.saved, r.already, r.skipped) == (2, 0, [])
    assert [s.item_id for s in stored] == [item(1).item_id, item(2).item_id]
    for s in stored:
        jsonschema.validate(s.to_contract(), SCHEMA)
        assert s.interpreted_at == NOW


def test_already_stored_is_counted_not_saved_again() -> None:
    _, store = collect(new=False)
    r = run([item(1)], Fake(), INSTRUMENTS, store, now=lambda: NOW)
    assert (r.saved, r.already) == (0, 1)


def test_an_item_whose_text_or_reading_fails_is_skipped_and_the_rest_continue() -> None:
    bad = Reading("Resumen.", (Impact("XX99", Decimal("0"), Decimal("0")),))
    fake = Fake({item(1).item_id: FullTextError("404"), item(2).item_id: bad})
    stored, store = collect()
    r = run([item(1), item(2), item(3)], fake, INSTRUMENTS, store, now=lambda: NOW)
    assert [s.item_id for s in stored] == [item(3).item_id]
    assert len(r.skipped) == 2 and "404" in r.skipped[0] and "XX99" in r.skipped[1]


@pytest.mark.parametrize(
    "error",
    [OllamaError("el modelo no respondió JSON"), urllib.error.URLError("connection refused")],
)
def test_ollama_failing_midway_aborts_and_keeps_only_complete_items(error: Exception) -> None:
    fake = Fake({item(2).item_id: error})
    stored, store = collect()
    with pytest.raises(AbortError, match="Ollama"):
        run([item(1), item(2), item(3)], fake, INSTRUMENTS, store, now=lambda: NOW)
    assert [s.item_id for s in stored] == [item(1).item_id]
    assert fake.calls == [item(1).item_id, item(2).item_id], "no sigue después del corte"


@pytest.mark.parametrize(
    "error", [psycopg.OperationalError("caída"), redis.ConnectionError("caída")]
)
def test_store_failing_aborts(error: Exception) -> None:
    def store(_: Interpretation) -> bool:
        raise error

    with pytest.raises(AbortError, match="guardar o publicar"):
        run([item(1), item(2)], Fake(), INSTRUMENTS, store, now=lambda: NOW)


def test_item_from_the_future_is_skipped_not_interpreted() -> None:
    fake = Fake()
    stored, store = collect()
    r = run([item(1)], fake, INSTRUMENTS, store, now=lambda: AT - timedelta(days=1))
    assert stored == [] and fake.calls == [] and len(r.skipped) == 1


def test_publisher_sends_the_contract_json_with_its_id() -> None:
    sent: list[tuple[str, dict[str, str]]] = []

    class FakeRedis:
        def xadd(self, stream: str, fields: dict[str, str]) -> None:
            sent.append((stream, fields))

    pub = RedisPublisher("redis://127.0.0.1:6379/15")
    pub._redis = FakeRedis()  # type: ignore[assignment]
    interp = run_one()
    pub.publish(interp.to_contract())
    [(stream, fields)] = sent
    assert stream == "news_interpretations.v1"
    assert fields["interpretation_id"] == interp.interpretation_id
    assert json.loads(fields["data"]) == interp.to_contract()


def run_one() -> Interpretation:
    stored, store = collect()
    run([item(1)], Fake(), INSTRUMENTS, store, now=lambda: NOW)
    return stored[0]


def test_main_fails_clearly_without_saving_when_ollama_is_down(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def down(*_: Any) -> dict[str, Any]:
        raise urllib.error.URLError("connection refused")

    monkeypatch.setattr(psycopg, "connect", _no_db)
    assert main(["interpret_new"], transport=down) == 2
    assert "Ollama no responde" in capsys.readouterr().err


def test_main_fails_clearly_without_saving_when_the_digest_differs(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def other_digest(*_: Any) -> dict[str, Any]:
        name = "hf.co/mistralai/Ministral-3-8B-Instruct-2512-GGUF:Q4_K_M"
        return {"models": [{"name": name, "digest": "f" * 64}]}

    monkeypatch.setattr(psycopg, "connect", _no_db)
    assert main(["interpret_new"], transport=other_digest) == 2
    assert "no es el fijado" in capsys.readouterr().err


def _no_db(*_: Any, **__: Any) -> None:
    raise AssertionError("no tiene que tocar la base si Ollama falla")
