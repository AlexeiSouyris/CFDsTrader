"""Novedad de los comunicados del FOMC (#668): texto, diff de palabras, embeddings y contrato.

Sin red ni base: las páginas son recortes reales de federalreserve.gov (`fixtures/`) y Ollama
se reemplaza por un transporte falso.
"""

import json
from dataclasses import replace
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.fomc_novelty import NoveltyError, NoveltyRow, Statement, load_config
from cfds_intel.fomc_novelty.__main__ import DEFAULT_CONFIG, cached_getter
from cfds_intel.fomc_novelty.embed import cosine_distance, embed, model_digest
from cfds_intel.fomc_novelty.model import MeasuredText
from cfds_intel.fomc_novelty.series import embedding_rows, word_diff_rows
from cfds_intel.fomc_novelty.text import extract_statement, measure, word_change_share, words

ROOT = Path(__file__).resolve().parents[2]
FIX = Path(__file__).parent / "fixtures"
SCHEMA = json.loads((ROOT / "contracts" / "fomc_statement_novelty.v1.schema.json").read_text())
VALIDATOR = Draft202012Validator(SCHEMA)
PR = "https://www.federalreserve.gov/newsevents/pressreleases/monetary"
CFG = load_config(DEFAULT_CONFIG.read_text())
NOW = datetime(2026, 11, 25, 12, tzinfo=UTC)


def page(day: str) -> str:
    return (FIX / f"fed_monetary{day}a_article.htm").read_text()


def statement(day: str, hour: int = 19, status: str = "scheduled") -> Statement:
    d = date(int(day[:4]), int(day[4:6]), int(day[6:]))
    return Statement(
        d.isoformat(), status, datetime(d.year, d.month, d.day, hour, tzinfo=UTC), f"{PR}{day}a.htm"
    )


def measured(day: str, text: str, status: str = "scheduled") -> MeasuredText:
    return MeasuredText(statement(day, status=status), text, "0" * 64, len(words(text)))


# --- texto -----------------------------------------------------------------------------------


def test_the_text_is_the_body_up_to_the_vote() -> None:
    text = extract_statement(page("20110126"), "x", "Voting for")
    assert text.startswith("Information received since the Federal Open Market Committee met")
    assert "Voting for" not in text and "Bernanke" not in text
    assert text.count("\n\n") == 4  # cinco párrafos antes de la votación
    assert "Last Update" not in text and "Share" not in text


def test_what_follows_the_vote_is_left_out() -> None:
    text = extract_statement(page("20200315"), "x", "Voting for")
    assert text.startswith("The coronavirus outbreak has harmed communities")
    assert "For media inquiries" not in text
    assert "In a related set of actions" not in text  # después de la votación


def test_a_page_without_the_vote_or_the_body_fails() -> None:
    with pytest.raises(NoveltyError, match="no aparece"):
        extract_statement(page("20241218").replace("Voting for", "Votes for"), "x", "Voting for")
    with pytest.raises(NoveltyError, match="cuerpo"):
        extract_statement("<html><body><p>Voting for</p></body></html>", "x", "Voting for")


def test_words_are_lowercase_and_keep_numbers_and_fractions() -> None:
    assert words("The Committee decided to lower the range by 1/4 percentage point to 4-1/4.") == [
        "the",
        "committee",
        "decided",
        "to",
        "lower",
        "the",
        "range",
        "by",
        "1/4",
        "percentage",
        "point",
        "to",
        "4-1/4",
    ]


def test_measure_hashes_and_counts_the_measured_text() -> None:
    m = measure(statement("20241218"), page("20241218"), "Voting for")
    assert len(m.sha256) == 64 and m.word_count == len(words(m.text)) > 100


# --- diff de palabras --------------------------------------------------------------------------


def test_word_change_share_limits() -> None:
    a = words("the committee will maintain the target range")
    assert word_change_share(a, a) == Decimal(0)
    assert word_change_share(a, words("inflation remains elevated")) == Decimal(1)
    # 6 de 7 iguales: 1 - 2·6/(7+7) = 1/7
    b = words("the committee will lower the target range")
    assert word_change_share(a, b) == Decimal("0.142857")
    with pytest.raises(NoveltyError):
        word_change_share([], a)


def test_word_diff_series_compares_each_with_the_one_before() -> None:
    texts = [
        measured("20240131", "the committee will maintain the target range"),
        measured("20240320", "the committee will maintain the target range"),
        measured("20240501", "the committee will lower the target range"),
    ]
    rows = word_diff_rows(texts, "word_diff.v1", NOW)
    assert [(r.reference_period, r.previous_reference_period, r.value) for r in rows] == [
        ("2024-03-20", "2024-01-31", Decimal(0)),
        ("2024-05-01", "2024-03-20", Decimal("0.142857")),
    ]
    # El valor existe desde la hora de su comunicado, no antes.
    assert all(
        r.available_at == t.statement.scheduled_at for r, t in zip(rows, texts[1:], strict=True)
    )
    for r in rows:
        assert list(VALIDATOR.iter_errors(r.to_contract())) == [], r


def test_out_of_order_or_repeated_statements_fail() -> None:
    a, b = measured("20240131", "a b c"), measured("20240320", "a b d")
    with pytest.raises(NoveltyError, match="fuera de orden"):
        word_diff_rows([b, a], "word_diff.v1", NOW)
    with pytest.raises(NoveltyError, match="fuera de orden"):
        word_diff_rows([a, a], "word_diff.v1", NOW)


def test_real_pages_give_a_value_between_zero_and_one() -> None:
    texts = [measure(statement(d), page(d), "Voting for") for d in ("20110126", "20241218")]
    (row,) = word_diff_rows(texts, "word_diff.v1", NOW)
    assert Decimal("0.3") < row.value < Decimal(1)


# --- embeddings ----------------------------------------------------------------------------------


class FakeOllama:
    def __init__(self, vectors: dict[str, list[float]], tags: list[dict[str, Any]]) -> None:
        self.vectors = vectors
        self.tags = tags
        self.calls: list[tuple[str, str, dict[str, Any] | None]] = []

    def __call__(
        self, method: str, url: str, body: dict[str, Any] | None, timeout: int
    ) -> dict[str, Any]:
        self.calls.append((method, url, body))
        if url.endswith("/api/tags"):
            return {"models": self.tags}
        assert body is not None
        return {"embeddings": [self.vectors[body["input"]]]}


def test_embed_sends_the_prefix_without_truncating() -> None:
    e = CFG.embedding
    fake = FakeOllama({e.prefix + "hola": [1.0, 0.0]}, [])
    assert embed(e, "hola", fake) == [1.0, 0.0]
    method, url, body = fake.calls[0]
    assert (method, url) == ("POST", "http://127.0.0.1:11434/api/embed")
    assert body is not None and body["truncate"] is False
    assert body["options"]["num_ctx"] == e.num_ctx


def test_the_model_is_pinned_by_digest_and_must_be_installed() -> None:
    e = CFG.embedding
    ok = FakeOllama({}, [{"name": "nomic-embed-text:latest", "digest": "abc123def456"}])
    assert model_digest(e, ok) == "abc123def456"
    with pytest.raises(NoveltyError, match="ollama pull"):
        model_digest(e, FakeOllama({}, [{"name": "qwen3:8b", "digest": "ffffffffff"}]))


def test_cosine_distance() -> None:
    assert cosine_distance([1.0, 0.0], [2.0, 0.0]) == Decimal(0)
    assert cosine_distance([1.0, 0.0], [0.0, 3.0]) == Decimal(1)
    assert cosine_distance([1.0, 0.0], [-1.0, 0.0]) == Decimal(2)
    with pytest.raises(NoveltyError):
        cosine_distance([1.0], [1.0, 2.0])
    with pytest.raises(NoveltyError):
        cosine_distance([0.0, 0.0], [1.0, 2.0])


def test_embedding_series_records_model_and_digest() -> None:
    texts = [measured("20240131", "uno"), measured("20240320", "dos", status="unscheduled")]
    vec = {"uno": [1.0, 0.0], "dos": [0.0, 1.0]}
    (row,) = embedding_rows(
        texts,
        vec.__getitem__,
        cosine_distance,
        "nomic-embed-text",
        "abc123def456",
        "embedding_cosine.v1",
        NOW,
    )
    assert (row.method, row.model, row.model_digest, row.value, row.status) == (
        "embedding_cosine",
        "nomic-embed-text",
        "abc123def456",
        Decimal(1),
        "unscheduled",
    )
    assert list(VALIDATOR.iter_errors(row.to_contract())) == []


# --- contrato y fila ------------------------------------------------------------------------------


def ok_row() -> NoveltyRow:
    (row,) = word_diff_rows(
        [measured("20240131", "a b c"), measured("20240320", "a b d")], "word_diff.v1", NOW
    )
    return row


@pytest.mark.parametrize(
    "over",
    [
        {"method": "sentiment"},
        {"status": "cancelled"},
        {"previous_reference_period": "2024-03-20"},
        {"model": "nomic-embed-text", "model_digest": "abc123def456"},
        {"value": Decimal("1.5")},
        {"available_at": datetime(2024, 3, 20, 18)},
    ],
)
def test_rows_that_break_the_rules_are_rejected(over: dict[str, object]) -> None:
    with pytest.raises(NoveltyError):
        replace(ok_row(), **over)  # type: ignore[arg-type]


@pytest.mark.parametrize(
    "over",
    [
        {"method": "sentiment"},
        {"model": "nomic-embed-text"},
        {"value": 1.5},
        {"text_sha256": "xyz"},
        {"available_at": "2024-03-20T18:00:00"},
        {"statement_url": "https://example.com/x.htm"},
        {"extra": 1},
    ],
)
def test_the_json_contract_rejects_the_same(over: dict[str, object]) -> None:
    doc = ok_row().to_contract()
    assert list(VALIDATOR.iter_errors(doc)) == []
    assert list(VALIDATOR.iter_errors({**doc, **over})) != []


# --- configuración y copia local ------------------------------------------------------------


def test_config_is_validated() -> None:
    text = DEFAULT_CONFIG.read_text()
    assert CFG.client.pause_secs >= 1 and CFG.end_marker == "Voting for"
    with pytest.raises(NoveltyError, match="pause_secs"):
        load_config(text.replace("pause_secs = 1.5", "pause_secs = 0.2"))
    with pytest.raises(NoveltyError, match=r"127\.0\.0\.1"):
        load_config(text.replace("http://127.0.0.1:11434", "http://example.com:11434"))
    with pytest.raises(NoveltyError, match="statuses"):
        load_config(text.replace('statuses = ["scheduled"]', 'statuses = ["cancelled"]'))


def test_pages_already_saved_are_not_requested_again(tmp_path: Path) -> None:
    asked: list[str] = []

    def get(url: str) -> str:
        asked.append(url)
        return "<html>x</html>"

    read = cached_getter(tmp_path, get)
    url = f"{PR}20240131a.htm"
    assert read(url) == read(url) == "<html>x</html>"
    assert asked == [url]
    assert (tmp_path / "monetary20240131a.htm").exists()
