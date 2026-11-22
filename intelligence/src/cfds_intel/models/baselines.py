"""Comparadores de ML v1 (docs/spec/ml-v1.md §4): B0 y B1.

- **B0:** la proporción de `y = 1` en el entrenamiento.
- **B1:** la tabla de la spec §13, la que se muestra hoy y el rival a vencer. Frecuencia por gatillo
  (el de mayor preferencia) por rango de puntaje, con el mínimo de casos de
  `config/probability.toml`; si el grupo no llega, cae a B0. Los grupos son los mismos que
  `cfds_journal::probability::ProbabilityConfig::group` (Rust), con la misma config.
"""

import tomllib
from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass
from itertools import pairwise

from cfds_intel.models.dataset import TRIGGERS, Example


class BaselineError(ValueError):
    """Config inválida o entrenamiento vacío."""


@dataclass(frozen=True)
class ProbabilityConfig:
    min_cases: int
    score_bands: tuple[int, ...]
    trigger_priority: tuple[str, ...]

    def group(self, e: Example) -> tuple[str, int] | None:
        trigger = next((t for t in self.trigger_priority if t in e.triggers), None)
        bands = [i for i, lo in enumerate(self.score_bands) if e.score >= lo]
        if trigger is None or not bands:
            return None
        return trigger, bands[-1]


def load_probability_config(text: str) -> ProbabilityConfig:
    """`config/probability.toml` (la misma que lee Rust)."""
    raw = tomllib.loads(text)
    bands, priority, min_cases = (
        raw.get("score_bands"),
        raw.get("trigger_priority"),
        raw.get("min_cases"),
    )
    if not isinstance(min_cases, int) or min_cases <= 0:
        raise BaselineError("min_cases tiene que ser un entero mayor que 0")
    if (
        not isinstance(bands, list)
        or not bands
        or not all(isinstance(b, int) for b in bands)
        or any(a >= b for a, b in pairwise(bands))
    ):
        raise BaselineError("score_bands: enteros estrictamente crecientes")
    if not isinstance(priority, list) or sorted(priority) != sorted(TRIGGERS):
        raise BaselineError("trigger_priority: los cuatro gatillos, una vez cada uno")
    return ProbabilityConfig(min_cases, tuple(bands), tuple(priority))


def _rate(examples: Sequence[Example]) -> float:
    if not examples:
        raise BaselineError("sin ejemplos de entrenamiento")
    return sum(e.y for e in examples) / len(examples)


@dataclass(frozen=True)
class B0:
    p: float

    @classmethod
    def fit(cls, train: Sequence[Example]) -> "B0":
        return cls(_rate(train))

    def predict(self, examples: Sequence[Example]) -> list[float]:
        return [self.p for _ in examples]


@dataclass(frozen=True)
class B1:
    fallback: B0
    table: dict[tuple[str, int], float]
    cfg: ProbabilityConfig

    @classmethod
    def fit(cls, train: Sequence[Example], cfg: ProbabilityConfig) -> "B1":
        counts: Counter[tuple[str, int]] = Counter()
        hits: Counter[tuple[str, int]] = Counter()
        for e in train:
            g = cfg.group(e)
            if g is not None:
                counts[g] += 1
                hits[g] += e.y
        table = {g: hits[g] / n for g, n in counts.items() if n >= cfg.min_cases}
        return cls(B0.fit(train), table, cfg)

    def predict(self, examples: Sequence[Example]) -> list[float]:
        out = []
        for e in examples:
            g = self.cfg.group(e)
            out.append(self.table.get(g, self.fallback.p) if g is not None else self.fallback.p)
        return out
