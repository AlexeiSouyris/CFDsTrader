"""Intervalo de cada predicción de M1 (docs/spec/ml-v1.md §6).

El rango entre los percentiles 5 y 95 de las predicciones de N modelos M1 (200), entrenados
sobre remuestreos bootstrap del entrenamiento, con la misma L2 elegida. El remuestreo es **por
bloques de una semana**, igual que la regla de mejora: con propuestas correlacionadas da un
intervalo más ancho, que es lo conservador.

Si en el entrenamiento hay menos de `min_cases` propuestas parecidas (mismo gatillo y rango de
puntaje, los grupos de la tabla §13), la predicción se muestra como "datos insuficientes" (`None`),
igual que en §13.
"""

import random
from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass

from cfds_intel.models.baselines import ProbabilityConfig
from cfds_intel.models.compare import percentile, weekly_blocks
from cfds_intel.models.config import IntervalConfig, TrainingConfig
from cfds_intel.models.dataset import Example
from cfds_intel.models.logistic import M1


@dataclass(frozen=True)
class Prediction:
    probability: float
    low: float
    high: float


def predict_with_intervals(
    model: M1,
    train: Sequence[Example],
    examples: Sequence[Example],
    probability: ProbabilityConfig,
    cfg: IntervalConfig,
    training: TrainingConfig,
) -> list[Prediction | None]:
    """Una predicción con intervalo por ejemplo, o `None` ("datos insuficientes")."""
    groups = Counter(probability.group(e) for e in train)
    enough = [
        (g := probability.group(e)) is not None and groups[g] >= probability.min_cases
        for e in examples
    ]
    if not any(enough):
        return [None for _ in examples]
    blocks = weekly_blocks(train)
    rnd = random.Random(training.seed)  # noqa: S311 (remuestreo reproducible)
    draws: list[list[float]] = [[] for _ in examples]
    for k in range(cfg.models):
        sample = [train[i] for _ in blocks for i in blocks[rnd.randrange(len(blocks))]]
        if len({e.y for e in sample}) < 2:
            continue  # un remuestreo de una sola clase no define un modelo
        m = M1.fit(sample, model.l2, training.seed + k, training.max_iterations)
        for j, q in enumerate(m.predict(examples)):
            draws[j].append(q)
    out: list[Prediction | None] = []
    for e, ok, ps in zip(examples, enough, draws, strict=True):
        if not ok or not ps:
            out.append(None)
            continue
        ps.sort()
        out.append(
            Prediction(
                model.predict([e])[0],
                percentile(ps, cfg.low_percentile),
                percentile(ps, cfg.high_percentile),
            )
        )
    return out
