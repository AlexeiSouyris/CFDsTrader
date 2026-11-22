"""`config/ml.toml` (docs/spec/ml-v1.md): se valida al cargar."""

import re
import tomllib
from dataclasses import dataclass
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
class MlConfig:
    dataset: DatasetConfig


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
    return MlConfig(dataset=DatasetConfig(instruments=tuple(instruments)))


def load_default() -> MlConfig:
    return load_config(DEFAULT_PATH.read_text())
