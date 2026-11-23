"""Métricas de ML v1 (docs/spec/ml-v1.md §6): log loss (la principal), Brier, AUC y calibración
por deciles con intervalo de Wilson. Probabilidades en `float`: no son dinero."""

import math
from collections.abc import Sequence
from dataclasses import dataclass

# Las predicciones se recortan a [EPS, 1 - EPS] para que un 0 o un 1 exacto (por ejemplo, B0 con
# una ventana sin aciertos) no dé una pérdida infinita.
EPS = 1e-15


def _check(y: Sequence[int], p: Sequence[float]) -> None:
    if len(y) != len(p) or not y:
        raise ValueError("hacen falta tantas etiquetas como predicciones, y al menos una")
    if any(v not in (0, 1) for v in y) or any(not 0 <= q <= 1 for q in p):
        raise ValueError("etiquetas 0/1 y probabilidades en [0, 1]")


def log_losses(y: Sequence[int], p: Sequence[float]) -> list[float]:
    """Pérdida de cada predicción (para promediar o remuestrear por bloques)."""
    _check(y, p)
    out = []
    for v, q in zip(y, p, strict=True):
        q = min(max(q, EPS), 1 - EPS)
        out.append(-math.log(q) if v == 1 else -math.log(1 - q))
    return out


def log_loss(y: Sequence[int], p: Sequence[float]) -> float:
    losses = log_losses(y, p)
    return sum(losses) / len(losses)


def brier(y: Sequence[int], p: Sequence[float]) -> float:
    _check(y, p)
    return sum((q - v) ** 2 for v, q in zip(y, p, strict=True)) / len(y)


def auc(y: Sequence[int], p: Sequence[float]) -> float | None:
    """Probabilidad de que un positivo tenga más puntaje que un negativo (empates: ½). `None`
    si falta una de las dos clases."""
    _check(y, p)
    pos = [q for v, q in zip(y, p, strict=True) if v == 1]
    neg = [q for v, q in zip(y, p, strict=True) if v == 0]
    if not pos or not neg:
        return None
    wins = sum((a > b) + 0.5 * (a == b) for a in pos for b in neg)
    return wins / (len(pos) * len(neg))


def wilson(hits: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n == 0:
        raise ValueError("sin casos")
    f = hits / n
    den = 1 + z * z / n
    center = (f + z * z / (2 * n)) / den
    half = z * math.sqrt(f * (1 - f) / n + z * z / (4 * n * n)) / den
    return max(0.0, center - half), min(1.0, center + half)


@dataclass(frozen=True)
class CalibrationBin:
    mean_predicted: float
    observed: float
    low: float
    high: float
    n: int


def calibration(y: Sequence[int], p: Sequence[float], bins: int = 10) -> list[CalibrationBin]:
    """Deciles de la predicción (por orden), con la frecuencia observada y su intervalo."""
    _check(y, p)
    pairs = sorted(zip(p, y, strict=True))
    out = []
    for i in range(bins):
        chunk = pairs[i * len(pairs) // bins : (i + 1) * len(pairs) // bins]
        if not chunk:
            continue
        hits = sum(v for _, v in chunk)
        low, high = wilson(hits, len(chunk))
        out.append(
            CalibrationBin(
                sum(q for q, _ in chunk) / len(chunk), hits / len(chunk), low, high, len(chunk)
            )
        )
    return out
