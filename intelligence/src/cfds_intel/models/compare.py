"""Cuándo M1 "mejora" a B1 (docs/spec/ml-v1.md §6, fijado antes de mirar resultados).

M1 mejora si su log loss, sobre todas las validaciones juntas, es menor que el de B1 **y** el
intervalo de confianza (95 %) de la diferencia queda entero por debajo de 0. El intervalo sale de
un bootstrap **por bloques de una semana** (semana ISO de `signal_at`, en UTC), porque las
propuestas de una misma semana no son independientes (§12). Si no, se queda B1.
"""

import math
import random
from collections import defaultdict
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime

from cfds_intel.models.dataset import Example


def week_of(t: datetime) -> tuple[int, int]:
    iso = t.isocalendar()
    return iso.year, iso.week


def percentile(sorted_values: Sequence[float], pct: float) -> float:
    """Percentil por rango más cercano (determinista): el valor en la posición ⌈pct/100 · n⌉."""
    if not sorted_values:
        raise ValueError("sin valores")
    k = max(1, math.ceil(pct / 100 * len(sorted_values)))
    return sorted_values[min(k, len(sorted_values)) - 1]


def weekly_blocks(examples: Sequence[Example]) -> list[list[int]]:
    """Índices de los ejemplos agrupados por semana, en orden de semana."""
    blocks: dict[tuple[int, int], list[int]] = defaultdict(list)
    for i, e in enumerate(examples):
        blocks[week_of(e.signal_at)].append(i)
    return [blocks[w] for w in sorted(blocks)]


@dataclass(frozen=True)
class Comparison:
    # Diferencia media de log loss, M1 - B1 (negativa: M1 pierde menos).
    difference: float
    low: float
    high: float
    blocks: int

    @property
    def improves(self) -> bool:
        return self.difference < 0 and self.high < 0


def compare(
    examples: Sequence[Example],
    loss_m1: Sequence[float],
    loss_b1: Sequence[float],
    resamples: int,
    confidence: float,
    seed: int,
) -> Comparison:
    """`loss_*`: la pérdida de cada ejemplo de validación (`metrics.log_losses`), en el mismo
    orden que `examples`."""
    if not examples or not len(examples) == len(loss_m1) == len(loss_b1):
        raise ValueError("hacen falta ejemplos y una pérdida de cada modelo por ejemplo")
    d = [a - b for a, b in zip(loss_m1, loss_b1, strict=True)]
    blocks = weekly_blocks(examples)
    sums = [sum(d[i] for i in b) for b in blocks]
    sizes = [len(b) for b in blocks]
    rnd = random.Random(seed)  # noqa: S311 (remuestreo reproducible, no criptografía)
    means = []
    for _ in range(resamples):
        pick = [rnd.randrange(len(blocks)) for _ in blocks]
        means.append(sum(sums[k] for k in pick) / sum(sizes[k] for k in pick))
    means.sort()
    tail = (1 - confidence) / 2 * 100
    return Comparison(
        difference=sum(d) / len(d),
        low=percentile(means, tail),
        high=percentile(means, 100 - tail),
        blocks=len(blocks),
    )
