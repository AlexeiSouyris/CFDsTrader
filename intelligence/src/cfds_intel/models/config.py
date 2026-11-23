"""`config/ml.toml` (docs/spec/ml-v1.md): se valida al cargar."""

import re
import tomllib
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
DEFAULT_PATH = ROOT / "config" / "ml.toml"

_INSTRUMENT = re.compile(r"^[A-Z0-9][A-Z0-9._-]{0,31}$")


class MlConfigError(ValueError):
    """`config/ml.toml` no es válido."""


@dataclass(frozen=True)
class DatasetConfig:
    # Instrumentos que entran al dataset, en el orden de su codificación one-hot.
    instruments: tuple[str, ...]


@dataclass(frozen=True)
class TrainingConfig:
    min_examples: int
    min_per_class: int
    inner_tune_fraction: float
    l2_grid: tuple[float, ...]
    max_iterations: int
    seed: int


@dataclass(frozen=True)
class EvaluationConfig:
    bootstrap_resamples: int
    confidence: float


@dataclass(frozen=True)
class IntervalConfig:
    models: int
    low_percentile: int
    high_percentile: int


@dataclass(frozen=True)
class MlConfig:
    dataset: DatasetConfig
    training: TrainingConfig
    evaluation: EvaluationConfig
    interval: IntervalConfig


def _evaluation(t: object) -> EvaluationConfig:
    if not isinstance(t, dict):
        raise MlConfigError("falta [evaluation]")
    return EvaluationConfig(
        bootstrap_resamples=_int(t, "bootstrap_resamples", 1),
        confidence=_fraction(t.get("confidence"), "confidence"),
    )


def _interval(t: object) -> IntervalConfig:
    if not isinstance(t, dict):
        raise MlConfigError("falta [interval]")
    low, high = _int(t, "low_percentile", 0), _int(t, "high_percentile", 0)
    if not low < high <= 100:
        raise MlConfigError("interval: low_percentile < high_percentile <= 100")
    return IntervalConfig(models=_int(t, "models", 1), low_percentile=low, high_percentile=high)


def _int(table: dict[str, object], key: str, low: int) -> int:
    v = table.get(key)
    if not isinstance(v, int) or isinstance(v, bool) or v < low:
        raise MlConfigError(f"{key} tiene que ser un entero >= {low}")
    return v


def _fraction(text: object, key: str, *, open_low: bool = True) -> float:
    try:
        v = Decimal(str(text)) if isinstance(text, str) else None
    except InvalidOperation:
        v = None
    if v is None or not (0 < v < 1 if open_low else 0 < v):
        raise MlConfigError(f'{key}: decimal en texto, como "0.2"')
    return float(v)


def _training(t: object) -> TrainingConfig:
    if not isinstance(t, dict):
        raise MlConfigError("falta [training]")
    grid = t.get("l2_grid")
    if not isinstance(grid, list) or not grid:
        raise MlConfigError("training.l2_grid: lista no vacía de decimales en texto")
    return TrainingConfig(
        min_examples=_int(t, "min_examples", 1),
        min_per_class=_int(t, "min_per_class", 1),
        inner_tune_fraction=_fraction(t.get("inner_tune_fraction"), "inner_tune_fraction"),
        l2_grid=tuple(_fraction(g, "l2_grid", open_low=False) for g in grid),
        max_iterations=_int(t, "max_iterations", 1),
        seed=_int(t, "seed", 0),
    )


def load_config(text: str) -> MlConfig:
    raw = tomllib.loads(text)
    d = raw.get("dataset")
    if not isinstance(d, dict):
        raise MlConfigError("falta [dataset]")
    instruments = d.get("instruments")
    if (
        not isinstance(instruments, list)
        or not instruments
        or not all(isinstance(i, str) and _INSTRUMENT.match(i) for i in instruments)
        or len(set(instruments)) != len(instruments)
    ):
        raise MlConfigError("dataset.instruments: lista no vacía de instrumentos sin repetir")
    return MlConfig(
        dataset=DatasetConfig(instruments=tuple(instruments)),
        training=_training(raw.get("training")),
        evaluation=_evaluation(raw.get("evaluation")),
        interval=_interval(raw.get("interval")),
    )


def load_default() -> MlConfig:
    return load_config(DEFAULT_PATH.read_text())
