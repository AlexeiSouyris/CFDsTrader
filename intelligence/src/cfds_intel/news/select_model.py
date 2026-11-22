"""Prueba de selección del modelo local (ADR 0008 §4, #129). Corre en el Mac mini, no gasta cupo.

```text
uv run python -m cfds_intel.news.select_model --labels tests/fixtures/news_labels.csv \\
    --model <candidato> [--model <otro>] [--out resultado.json]
```

Los criterios salen de `config/news_interpreter.toml` ([selection]) y se fijaron **antes** de
mirar resultados. Para cada candidato y cada titular etiquetado por el dueño, se interpreta dos
veces con `interpret()` (#122):

- **válidas:** las dos lecturas pasan las reglas y el resumen está en español;
- **deterministas:** las dos lecturas dan los mismos instrumentos y el mismo ajuste por noticia;
- **tiempo:** p95 de la duración de cada lectura;
- **memoria:** `size` del modelo cargado (`GET /api/ps`);
- **acierto de signo:** contra las etiquetas, solo en los pares relevantes, con su intervalo de
  Wilson del 95 %. Hacen falta al menos `min_signed_pairs_each` pares con "+" y otros tantos
  con "-"; si no, se amplía la muestra antes de correr;
- **detección de relevancia** (aparte): por cada titular e instrumento, el modelo marca
  relevante si su relevancia es ≥ `relevance_threshold`. Su acierto **balanceado** tiene que
  llegar a `min_relevance_accuracy_pct`.

El intérprete lee el texto oficial de cada comunicado (`fulltext.py`); las etiquetas también se
hacen leyendo ese texto.

Si ningún candidato pasa, no se elige ninguno: la decisión vuelve al dueño y los umbrales no se
relajan.
"""

import argparse
import csv
import io
import json
import math
import re
import sys
import time
from collections.abc import Sequence
from dataclasses import asdict, dataclass, field
from datetime import UTC, datetime
from decimal import ROUND_HALF_DOWN, Decimal
from pathlib import Path

from cfds_intel.news.feeds import NewsItem
from cfds_intel.news.interpret import Interpretation, InterpretationError, instrument_ids, interpret
from cfds_intel.news.ollama import (
    OllamaInterpreter,
    Selection,
    load_config,
    load_model,
    loaded_memory_bytes,
)

ROOT = Path(__file__).resolve().parents[4]
_WORD = re.compile(r"[a-záéíóúñü]+")
# Palabras muy frecuentes de cada idioma: un resumen está en español si tiene más de las primeras.
_ES = {"el", "la", "los", "las", "de", "del", "que", "en", "y", "por", "para", "con", "se", "su"}
_EN = {"the", "of", "and", "to", "in", "for", "with", "on", "is", "its", "by", "that"}
_Z95 = Decimal("1.959964")


class SelectionError(ValueError):
    """La muestra o los datos de la prueba no cumplen las reglas: no se corre nada."""


@dataclass(frozen=True)
class Labeled:
    item: NewsItem
    # (instrumento, signo) de los pares que el dueño marcó como relevantes.
    relevant: tuple[tuple[str, str], ...]


def load_labels(text: str) -> list[Labeled]:
    """Lee el CSV de titulares etiquetados (#161). Una fila por (titular, instrumento), o varios
    instrumentos con el mismo signo en una fila separados por ";". Un titular sin impacto va con
    `instrument_id` vacío y `relevant = no`. Toda fila va etiquetada. La columna `impact` (1 a 5)
    todavía no se usa: cómo compararla con la relevancia lo decide el dueño (#129)."""
    rows = list(csv.DictReader(io.StringIO(text)))
    if not rows:
        raise SelectionError("el CSV de etiquetas está vacío")
    by_item: dict[tuple[str, str], tuple[NewsItem, list[tuple[str, str]]]] = {}
    for n, r in enumerate(rows, start=2):
        relevant = (r.get("relevant") or "").strip().lower()
        if relevant not in {"si", "sí", "no"}:
            raise SelectionError(f"fila {n}: falta etiquetar 'relevant' (si o no)")
        key = (r["source"], r["item_id"])
        if key not in by_item:
            published = datetime.fromisoformat(r["published_at"].replace("Z", "+00:00"))
            item = NewsItem(
                r["source"], r["feed"], r["item_id"], r["title"], r["url"], published, published
            )
            by_item[key] = (item, [])
        if relevant != "no":
            sign = (r.get("sign") or "").strip()
            # Varios instrumentos con el mismo signo van en una fila, separados por ";".
            instruments = [i.strip() for i in (r.get("instrument_id") or "").split(";")]
            if sign not in {"-", "0", "+"} or not all(instruments):
                raise SelectionError(f"fila {n}: un par relevante necesita instrumento y signo")
            for instrument in instruments:
                if instrument in {i for i, _ in by_item[key][1]}:
                    raise SelectionError(f"fila {n}: {instrument} repetido en el mismo titular")
                by_item[key][1].append((instrument, sign))
    return [Labeled(item, tuple(pairs)) for item, pairs in by_item.values()]


def is_spanish(text: str) -> bool:
    words = _WORD.findall(text.lower())
    return sum(w in _ES for w in words) > sum(w in _EN for w in words)


def news_adjustment(sentiment: Decimal, relevance: Decimal) -> int:
    """Solo para comparar dos lecturas en esta prueba. El ajuste del sistema lo calcula Rust
    (`cfds_strategy::single_adjustment`, #130): redondear(2 * s * r), con ±0,5 hacia cero."""
    return int((2 * sentiment * relevance).quantize(Decimal(1), rounding=ROUND_HALF_DOWN))


def structure(i: Interpretation) -> frozenset[tuple[str, int]]:
    return frozenset(
        (x.instrument_id, news_adjustment(x.sentiment, x.relevance)) for x in i.impacts
    )


def sign_of(i: Interpretation | None, instrument: str) -> str:
    for x in i.impacts if i else ():
        if x.instrument_id == instrument:
            return "+" if x.sentiment > 0 else "-" if x.sentiment < 0 else "0"
    return "0"


def wilson(k: int, n: int) -> tuple[Decimal, Decimal]:
    """Intervalo de Wilson del 95 % para una proporción k/n."""
    if n == 0:
        return Decimal(0), Decimal(1)
    p, nn, z2 = Decimal(k) / n, Decimal(n), _Z95 * _Z95
    center = (p + z2 / (2 * nn)) / (1 + z2 / nn)
    half = _Z95 * ((p * (1 - p) / nn + z2 / (4 * nn * nn)).sqrt()) / (1 + z2 / nn)
    return max(Decimal(0), center - half), min(Decimal(1), center + half)


def p95(values: Sequence[Decimal]) -> Decimal:
    """Percentil 95 por rango más cercano."""
    if not values:
        return Decimal(0)
    ordered = sorted(values)
    return ordered[math.ceil(Decimal("0.95") * len(ordered)) - 1]


@dataclass
class Result:
    model: str
    items: int
    valid: int = 0
    deterministic: int = 0
    seconds: list[Decimal] = field(default_factory=list)
    memory_gb: Decimal = Decimal(0)
    sign_hits: int = 0
    sign_pairs: int = 0
    # Detección de relevancia por (titular, instrumento): verdaderos/falsos positivos/negativos.
    rel_tp: int = 0
    rel_fn: int = 0
    rel_tn: int = 0
    rel_fp: int = 0

    def pct(self, k: int) -> Decimal:
        return (
            (Decimal(100) * k / self.items).quantize(Decimal("0.1")) if self.items else Decimal(0)
        )

    def accuracy(self) -> tuple[Decimal, Decimal, Decimal]:
        low, high = wilson(self.sign_hits, self.sign_pairs)
        acc = Decimal(self.sign_hits) / self.sign_pairs if self.sign_pairs else Decimal(0)
        return acc * 100, low * 100, high * 100

    def relevance_accuracy(self) -> Decimal:
        """Acierto **balanceado** (promedio entre los relevantes y los no relevantes), en %: así
        un modelo que dice siempre "no relevante" no saca nota alta."""
        pos, neg = self.rel_tp + self.rel_fn, self.rel_tn + self.rel_fp
        if not pos or not neg:
            return Decimal(0)
        hit = Decimal(self.rel_tp) / pos + Decimal(self.rel_tn) / neg
        return (hit * 50).quantize(Decimal("0.1"))

    def failures(self, s: Selection) -> list[str]:
        out = []
        if self.relevance_accuracy() < s.min_relevance_accuracy_pct:
            out.append(
                f"relevancia {self.relevance_accuracy()} % < {s.min_relevance_accuracy_pct} %"
            )
        if self.pct(self.valid) < s.min_valid_pct:
            out.append(f"válidas {self.pct(self.valid)} % < {s.min_valid_pct} %")
        if self.deterministic < self.items:
            out.append(f"deterministas {self.pct(self.deterministic)} % < 100 %")
        if self.memory_gb > s.max_memory_gb:
            out.append(f"memoria {self.memory_gb} GB > {s.max_memory_gb} GB")
        if p95(self.seconds) > s.max_seconds_p95:
            out.append(f"p95 {p95(self.seconds)} s > {s.max_seconds_p95} s")
        return out


def score(
    model: str,
    sample: Sequence[Labeled],
    runs: Sequence[tuple[Interpretation | None, Interpretation | None]],
    seconds: Sequence[Decimal],
    memory_bytes: int,
    instruments: Sequence[str],
    relevance_threshold: Decimal,
) -> Result:
    """Resultado de un candidato. `runs[i]`: las dos lecturas del titular `i` (None = inválida)."""
    r = Result(model, len(sample), seconds=list(seconds))
    r.memory_gb = (Decimal(memory_bytes) / 2**30).quantize(Decimal("0.01"))
    for labeled, (a, b) in zip(sample, runs, strict=True):
        ok = [x for x in (a, b) if x is not None and is_spanish(x.summary_es)]
        r.valid += len(ok) == 2
        r.deterministic += len(ok) == 2 and structure(ok[0]) == structure(ok[1])
        for instrument, expected in labeled.relevant:
            r.sign_pairs += 1
            r.sign_hits += sign_of(a, instrument) == expected
        labeled_relevant = {i for i, _ in labeled.relevant}
        flagged = {
            x.instrument_id for x in (a.impacts if a else ()) if x.relevance >= relevance_threshold
        }
        for instrument in instruments:
            truth, said = instrument in labeled_relevant, instrument in flagged
            r.rel_tp += truth and said
            r.rel_fn += truth and not said
            r.rel_tn += not truth and not said
            r.rel_fp += not truth and said
    return r


def pick(results: Sequence[Result], s: Selection) -> tuple[Result | None, str]:
    """Ganador según el ADR 0008 §4, con el motivo."""
    passing = [r for r in results if not r.failures(s)]
    if not passing:
        return None, "ningún candidato pasa los criterios mínimos: decide el dueño"
    best = max(passing, key=lambda r: r.accuracy()[0])
    tied = [
        r
        for r in passing
        if best.accuracy()[0] - r.accuracy()[0] < s.tie_points
        or r.accuracy()[2] >= best.accuracy()[1]
    ]
    if len(tied) == 1:
        return best, "mayor acierto de signo"
    winner = min(tied, key=lambda r: r.memory_gb)
    return winner, f"empate de acierto entre {len(tied)}: gana el de menos memoria"


def run_model(
    model: str, sample: Sequence[Labeled], instruments: Sequence[str], cfg_text: str
) -> Result:
    cfg = load_config(cfg_text)
    interp = OllamaInterpreter(cfg, model=model)
    runs, seconds = [], []
    for labeled in sample:
        pair: list[Interpretation | None] = []
        for _ in range(2):
            start = time.perf_counter()
            try:
                pair.append(interpret(labeled.item, interp, instruments, datetime.now(UTC)))
            except (InterpretationError, OSError):
                pair.append(None)
            seconds.append(Decimal(str(round(time.perf_counter() - start, 3))))
        runs.append((pair[0], pair[1]))
    # Con keep_alive = 0 el modelo se descarga después de cada noticia: se carga a propósito
    # para medir su memoria (si no, /api/ps diría que no está cargado).
    load_model(cfg, model)
    memory = loaded_memory_bytes(cfg, model)
    threshold = cfg.selection.relevance_threshold
    return score(model, sample, runs, seconds, memory, instruments, threshold)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--labels", required=True, type=Path)
    ap.add_argument("--model", required=True, action="append")
    ap.add_argument("--config", type=Path, default=ROOT / "config" / "news_interpreter.toml")
    ap.add_argument("--out", type=Path)
    args = ap.parse_args(argv[1:])
    cfg_text = args.config.read_text()
    selection = load_config(cfg_text).selection
    sample = load_labels(args.labels.read_text())
    pairs = sum(len(x.relevant) for x in sample)
    if pairs < selection.min_relevant_pairs:
        print(
            f"solo {pairs} pares relevantes (mínimo {selection.min_relevant_pairs}): ampliar la "
            "muestra y etiquetarla antes de correr los modelos",
            file=sys.stderr,
        )
        return 2
    for sign in ("+", "-"):
        n = sum(1 for x in sample for _, sg in x.relevant if sg == sign)
        if n < selection.min_signed_pairs_each:
            print(
                f"solo {n} pares relevantes con signo {sign} (mínimo "
                f"{selection.min_signed_pairs_each}): ampliar la muestra antes de correr",
                file=sys.stderr,
            )
            return 2
    instruments = instrument_ids((ROOT / "config" / "instruments.toml").read_text())
    results = [run_model(m, sample, instruments, cfg_text) for m in args.model]
    for r in results:
        acc, low, high = r.accuracy()
        print(
            f"{r.model}: válidas {r.pct(r.valid)} %, deterministas {r.pct(r.deterministic)} %, "
            f"p95 {p95(r.seconds)} s, memoria {r.memory_gb} GB, acierto {acc:.1f} % "
            f"[{low:.1f}, {high:.1f}] en {r.sign_pairs} pares, "
            f"relevancia {r.relevance_accuracy()} % (balanceado)"
        )
        for f in r.failures(selection):
            print(f"  no pasa: {f}")
    winner, why = pick(results, selection)
    print(f"elegido: {winner.model if winner else 'ninguno'} ({why})")
    if args.out:
        doc = {
            "results": [asdict(r) for r in results],
            "winner": winner.model if winner else None,
            "why": why,
        }
        args.out.write_text(json.dumps(doc, default=str, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
