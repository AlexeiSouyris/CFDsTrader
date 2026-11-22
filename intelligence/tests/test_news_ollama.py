"""Intérprete con Ollama y prueba de selección (ADR 0008, #129). Sin red ni Ollama."""

import json
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any

import pytest

from cfds_intel.news import NewsItem
from cfds_intel.news.interpret import Interpretation, InterpretationError, interpret
from cfds_intel.news.ollama import (
    PROMPT_SHA256,
    OllamaError,
    OllamaInterpreter,
    load_config,
    local_http,
)
from cfds_intel.news.select_model import (
    SelectionError,
    is_spanish,
    load_labels,
    news_adjustment,
    p95,
    pick,
    score,
    wilson,
)

ROOT = Path(__file__).resolve().parents[2]
REAL = (ROOT / "config" / "news_interpreter.toml").read_text()
CFG = load_config(REAL)
MODEL = "hf.co/Qwen/Qwen3-8B-GGUF:Q4_K_M"
DIGEST = "a" * 64
PUBLISHED = datetime(2026, 11, 20, 13, 25, tzinfo=UTC)
ITEM = NewsItem(
    "fed",
    "speeches",
    "https://www.federalreserve.gov/x.htm",
    "Waller, Opening Remarks",
    "https://www.federalreserve.gov/x.htm",
    PUBLISHED,
    PUBLISHED,
)
INSTRUMENTS = ("DE40", "US100", "US30", "US500")
TH = Decimal("0.5")


def TEXT(item: NewsItem) -> str:  # noqa: N802 (hace de función de texto oficial en los tests)
    return "The Committee decided to raise the target range for the federal funds rate."


class Fake:
    """Ollama falso: responde /api/tags, /api/ps y /api/chat, y guarda lo que le pidieron."""

    def __init__(self, content: str | list[str], digest: str = DIGEST) -> None:
        self.contents = [content] if isinstance(content, str) else list(content)
        self.digest = digest
        self.calls: list[tuple[str, str, dict[str, Any] | None]] = []

    def __call__(
        self, method: str, url: str, body: dict[str, Any] | None, timeout: int
    ) -> dict[str, Any]:
        self.calls.append((method, url, body))
        if url.endswith("/api/tags"):
            return {"models": [{"name": MODEL, "model": MODEL, "digest": self.digest}]}
        if url.endswith("/api/ps"):
            return {"models": [{"name": MODEL, "model": MODEL, "size": 5_368_709_120}]}
        content = self.contents.pop(0) if len(self.contents) > 1 else self.contents[0]
        return {"model": MODEL, "message": {"role": "assistant", "content": content}, "done": True}


def answer(summary: str = "La Fed abre un discurso sin anuncios.", impacts: Any = ()) -> str:
    return json.dumps({"summary_es": summary, "impacts": list(impacts)})


def test_repository_config_has_no_model_until_the_selection_test() -> None:
    assert CFG.url == "http://127.0.0.1:11434"
    assert (CFG.temperature, CFG.seed, CFG.think) == (0, 42, False)
    assert CFG.model == "" and CFG.digest == ""
    assert CFG.selection.min_relevant_pairs == 20
    with pytest.raises(OllamaError, match="no hay modelo elegido"):
        OllamaInterpreter(CFG, transport=Fake(answer()), text_for=TEXT)


@pytest.mark.parametrize(
    "url", ["https://127.0.0.1:11434", "http://10.0.0.5:11434", "http://ollama.example.com"]
)
def test_only_localhost_is_accepted(url: str) -> None:
    with pytest.raises(OllamaError, match="localhost"):
        load_config(REAL.replace('url = "http://127.0.0.1:11434"', f'url = "{url}"'))
    with pytest.raises(OllamaError, match="localhost"):
        local_http("GET", url + "/api/tags", None, 5)


def test_request_is_deterministic_structured_and_records_model_and_prompt() -> None:
    fake = Fake(answer(impacts=[{"instrument_id": "US500", "sentiment": "0", "relevance": "0.1"}]))
    interp = OllamaInterpreter(CFG, model=MODEL, transport=fake, text_for=TEXT)
    doc = interpret(ITEM, interp, INSTRUMENTS, PUBLISHED + timedelta(minutes=1)).to_contract()
    assert doc["interpreter"] == {
        "name": "ollama",
        "version": "p2",
        "model": f"{MODEL}@{DIGEST}",
        "prompt_sha256": PROMPT_SHA256,
    }
    method, url, body = fake.calls[-1]
    assert (method, url) == ("POST", "http://127.0.0.1:11434/api/chat")
    assert body is not None
    assert body["stream"] is False and body["think"] is False
    assert body["options"] == {"temperature": 0, "seed": 42}
    assert body["format"]["required"] == ["summary_es", "impacts"]
    user = json.loads(body["messages"][1]["content"])
    assert user["titular"] == "Waller, Opening Remarks"
    assert user["texto_oficial"] == TEXT(ITEM)
    # El texto se usa para interpretar y no queda en el evento (ADR 0006).
    assert TEXT(ITEM) not in json.dumps(doc)
    assert user["publicado_utc"] == "2026-11-20T13:25:00Z"
    assert user["instrumentos_permitidos"] == list(INSTRUMENTS)


def test_a_pinned_digest_must_match_the_installed_model() -> None:
    pinned = load_config(
        REAL.replace('name = ""', f'name = "{MODEL}"').replace(
            'digest = ""', f'digest = "{"b" * 64}"'
        )
    )
    with pytest.raises(OllamaError, match="no es el fijado"):
        OllamaInterpreter(pinned, transport=Fake(answer()), text_for=TEXT)
    ok = load_config(
        REAL.replace('name = ""', f'name = "{MODEL}"').replace(
            'digest = ""', f'digest = "{DIGEST}"'
        )
    )
    assert (
        OllamaInterpreter(ok, transport=Fake(answer()), text_for=TEXT).info.model
        == f"{MODEL}@{DIGEST}"
    )


@pytest.mark.parametrize(
    "content",
    [
        "no es JSON",
        json.dumps({"summary_es": "x"}),
        answer(impacts=[{"instrument_id": "US500", "sentiment": 0.5, "relevance": "0.5"}]),
        answer(impacts=[{"instrument_id": "US500", "sentiment": "mucho", "relevance": "0.5"}]),
        answer(impacts=[{"instrument_id": "AAPL", "sentiment": "0.5", "relevance": "0.5"}]),
        answer(impacts=[{"instrument_id": "US500", "sentiment": "1.5", "relevance": "0.5"}]),
        answer(summary=""),
    ],
)
def test_bad_model_answers_never_become_events(content: str) -> None:
    interp = OllamaInterpreter(CFG, model=MODEL, transport=Fake(content), text_for=TEXT)
    with pytest.raises(InterpretationError):
        interpret(ITEM, interp, INSTRUMENTS, PUBLISHED + timedelta(minutes=1))


LABELS = """source,feed,item_id,title,url,published_at,instrument_id,sign,relevant
fed,press_monetary,a,FOMC raises rates,https://f/a,2026-11-20T19:00:00Z,US500,-,si
fed,press_monetary,a,FOMC raises rates,https://f/a,2026-11-20T19:00:00Z,US100,-,si
fed,press_all,b,College Fed Challenge winners,https://f/b,2026-11-20T21:15:00Z,,,no
"""


def test_labels_group_pairs_by_news_and_require_every_row_labeled() -> None:
    sample = load_labels(LABELS)
    assert [x.relevant for x in sample] == [(("US500", "-"), ("US100", "-")), ()]
    with pytest.raises(SelectionError, match="relevant"):
        load_labels(LABELS.replace(",,,no", ",,,"))
    with pytest.raises(SelectionError, match="instrumento y signo"):
        load_labels(LABELS.replace("US500,-,si", "US500,,si"))


def test_spanish_check_and_evaluation_adjustment_match_the_rules() -> None:
    assert is_spanish("La Reserva Federal sube los tipos de interés.")
    assert not is_spanish("The Federal Reserve raises the rates of interest.")
    # Igual que cfds_strategy::single_adjustment (#130): ±0,5 hacia cero.
    assert news_adjustment(Decimal("0.5"), Decimal("0.5")) == 0
    assert news_adjustment(Decimal("-0.75"), Decimal("1")) == -1
    assert news_adjustment(Decimal("0.7501"), Decimal("1")) == 2


def test_wilson_interval_and_p95() -> None:
    low, high = wilson(18, 20)
    assert Decimal("0.68") < low < Decimal("0.70") and Decimal("0.97") < high < Decimal("0.98")
    assert wilson(0, 0) == (Decimal(0), Decimal(1))
    assert p95([Decimal(i) for i in range(1, 21)]) == Decimal(19)


def reading(
    sample_item: NewsItem, impacts: Any, summary: str = "La Fed sube los tipos."
) -> Interpretation:
    interp = OllamaInterpreter(
        CFG, model=MODEL, transport=Fake(answer(summary, impacts)), text_for=TEXT
    )
    return interpret(sample_item, interp, INSTRUMENTS, sample_item.published_at)


def test_score_counts_valid_deterministic_and_sign_hits() -> None:
    sample = load_labels(LABELS)
    neg = [{"instrument_id": "US500", "sentiment": "-0.8", "relevance": "0.9"}]
    a = reading(sample[0].item, neg)
    b = reading(
        sample[0].item, [{"instrument_id": "US500", "sentiment": "-0.2", "relevance": "0.9"}]
    )
    english = reading(sample[1].item, [], summary="The Fed announces winners.")
    r = score(
        "m", sample, [(a, b), (english, english)], [Decimal(3)] * 4, 4 * 2**30, INSTRUMENTS, TH
    )
    assert (r.items, r.valid, r.deterministic) == (2, 1, 0)  # el resumen en inglés no vale
    assert (r.sign_hits, r.sign_pairs) == (1, 2)  # US100 sin impacto cuenta como "0", no "-"
    assert r.memory_gb == Decimal("4.00")
    assert "deterministas 0.0 % < 100 %" in r.failures(CFG.selection)


def test_pick_prefers_accuracy_breaks_ties_by_memory_and_can_pick_none() -> None:
    sample = load_labels(LABELS)
    good = reading(
        sample[0].item,
        [
            {"instrument_id": "US500", "sentiment": "-0.8", "relevance": "0.9"},
            {"instrument_id": "US100", "sentiment": "-0.8", "relevance": "0.9"},
        ],
    )
    none_ = reading(sample[1].item, [])
    big = score(
        "big", sample, [(good, good), (none_, none_)], [Decimal(1)] * 4, 7 * 2**30, INSTRUMENTS, TH
    )
    small = score(
        "small",
        sample,
        [(good, good), (none_, none_)],
        [Decimal(1)] * 4,
        5 * 2**30,
        INSTRUMENTS,
        TH,
    )
    winner, why = pick([big, small], CFG.selection)
    assert winner is small and "empate" in why
    huge = score(
        "huge", sample, [(good, good), (none_, none_)], [Decimal(1)] * 4, 9 * 2**30, INSTRUMENTS, TH
    )
    winner, why = pick([huge], CFG.selection)
    assert winner is None and "ningún candidato" in why


def test_relevance_is_measured_with_balanced_accuracy() -> None:
    sample = load_labels(LABELS)
    flags_one = [{"instrument_id": "US500", "sentiment": "-0.8", "relevance": "0.9"}]
    a = reading(sample[0].item, flags_one)
    none_ = reading(sample[1].item, [])
    r = score("m", sample, [(a, a), (none_, none_)], [Decimal(1)] * 4, 2**30, INSTRUMENTS, TH)
    # US500 bien (TP), US100 se le escapó (FN), y los 6 no relevantes bien (TN).
    assert (r.rel_tp, r.rel_fn, r.rel_tn, r.rel_fp) == (1, 1, 6, 0)
    assert r.relevance_accuracy() == Decimal("75.0")  # (1/2 + 6/6) / 2
    always_no = score(
        "no", sample, [(none_, none_), (none_, none_)], [Decimal(1)] * 4, 2**30, INSTRUMENTS, TH
    )
    assert always_no.relevance_accuracy() == Decimal("50.0")  # decir siempre "no" no aprueba
    assert any("relevancia" in f for f in always_no.failures(CFG.selection))


def test_selection_stops_without_enough_signed_pairs(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    from cfds_intel.news.select_model import main

    rows = [LABELS.splitlines()[0]]
    for k in range(20):  # 20 relevantes, pero todos "-": faltan los "+"
        rows.append(f"fed,press_all,i{k},T{k},https://f/{k},2026-11-20T19:00:00Z,US500,-,si")
    labels = tmp_path / "labels.csv"
    labels.write_text("\n".join(rows) + "\n")
    assert main(["x", "--labels", str(labels), "--model", MODEL]) == 2
    assert "signo +" in capsys.readouterr().err


def test_config_rejects_bad_text_and_relevance_settings() -> None:
    for old, new in [
        ('relevance_threshold = "0.5"', "relevance_threshold = 0.5"),
        ('relevance_threshold = "0.5"', 'relevance_threshold = "1.5"'),
        ("min_relevance_accuracy_pct = 70", "min_relevance_accuracy_pct = 101"),
        ('allowed_hosts = ["www.federalreserve.gov", "www.ecb.europa.eu"]', "allowed_hosts = []"),
    ]:
        with pytest.raises(OllamaError):
            load_config(REAL.replace(old, new))


def test_several_instruments_in_one_row_are_split_and_impact_is_ignored() -> None:
    text = (
        "source,feed,item_id,title,url,published_at,instrument_id,sign,relevant,impact\n"
        "fed,press_monetary,x,FOMC cuts,https://f/x,2025-12-10T19:00:00Z,US100;US500;US30,+,si,5\n"
        "fed,press_all,y,Bank approval,https://f/y,2025-12-11T19:00:00Z,,0,no,1\n"
    )
    sample = load_labels(text)
    assert [x.relevant for x in sample] == [
        (("US100", "+"), ("US500", "+"), ("US30", "+")),
        (),
    ]
    with pytest.raises(SelectionError):
        load_labels(text.replace("US100;US500;US30", "US100;US100"))
    with pytest.raises(SelectionError):
        load_labels(text.replace("US100;US500;US30", "US100;;US30"))
