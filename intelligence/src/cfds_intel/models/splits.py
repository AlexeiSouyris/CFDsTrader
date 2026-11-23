"""Cortes del tiempo del modelo de ML v1 (docs/spec/ml-v1.md §5 y §8.3, #158).

- **Plan de ventanas:** el mismo algoritmo que `cfds_market_data::split::plan` (Rust), con
  `config/data_split.toml` sin cambiar sus números. Un fixture compartido
  (`intelligence/tests/fixtures/data_split_plan.json`) lo prueban los dos lenguajes, así no
  divergen. La spec §9 pone los cortes en Python.
- **Fuera de muestra:** nunca se entrega. Sale cualquier propuesta que se decide en ese tramo o
  cuyo resultado se conoce en él (su etiqueta usaría precios de ese tramo).
- **Purga:** sale del entrenamiento una propuesta cuyo resultado se conoce en el inicio de su
  ventana de validación o después.
- **Embargo:** sale del entrenamiento una propuesta decidida en las `time_exit_bars` velas de 15m
  que siguen al fin de la validación (`config/backtest.toml`).
- **Corte interno** para hiperparámetros: el último 20 % del entrenamiento, con la misma purga.
"""

import tomllib
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, date, datetime, time, timedelta

from cfds_intel.models.dataset import Example

M15 = timedelta(minutes=15)


class SplitError(ValueError):
    """Config inválida o datos que no alcanzan para una ventana."""


@dataclass(frozen=True)
class SplitConfig:
    out_of_sample_months: int
    train_months: int
    validation_months: int
    step_months: int


@dataclass(frozen=True)
class Period:
    """Tramo `[start, end)` con cortes a las 00:00 UTC."""

    start: datetime
    end: datetime

    def __contains__(self, t: datetime) -> bool:
        return self.start <= t < self.end


@dataclass(frozen=True)
class Window:
    train: Period
    validation: Period


@dataclass(frozen=True)
class DataSplit:
    windows: tuple[Window, ...]
    out_of_sample: Period


@dataclass(frozen=True)
class Fold:
    window: Window
    train: tuple[Example, ...]
    validation: tuple[Example, ...]
    purged: int
    embargoed: int


def _positive_int(table: object, key: str) -> int:
    v = table.get(key) if isinstance(table, dict) else None
    if not isinstance(v, int) or isinstance(v, bool) or v <= 0:
        raise SplitError(f"{key} tiene que ser un entero mayor que 0")
    return v


def load_split_config(text: str) -> SplitConfig:
    """`config/data_split.toml`."""
    raw = tomllib.loads(text)
    wf = raw.get("walk_forward")
    return SplitConfig(
        out_of_sample_months=_positive_int(raw.get("out_of_sample"), "months"),
        train_months=_positive_int(wf, "train_months"),
        validation_months=_positive_int(wf, "validation_months"),
        step_months=_positive_int(wf, "step_months"),
    )


def load_embargo(backtest_toml: str) -> timedelta:
    """`exits.time_exit_bars` de `config/backtest.toml`, en velas de 15m."""
    return _positive_int(tomllib.loads(backtest_toml).get("exits"), "time_exit_bars") * M15


def _add_months(d: date, n: int) -> date:
    m = d.month - 1 + n
    return date(d.year + m // 12, m % 12 + 1, 1)


def _midnight(d: date) -> datetime:
    return datetime.combine(d, time.min, UTC)


def plan(first: datetime, last: datetime, cfg: SplitConfig) -> DataSplit:
    """Igual que `cfds_market_data::split::plan`: ventanas desde el primer mes completo."""
    window_months = cfg.train_months + cfg.validation_months
    last_month = last.date().replace(day=1)
    oos_start = _add_months(last_month, -cfg.out_of_sample_months)
    first_day = first.date()
    s = (
        first_day
        if first_day.day == 1 and first.time() == time.min
        else _add_months(first_day.replace(day=1), 1)
    )
    windows: list[Window] = []
    while _add_months(s, window_months) <= oos_start:
        train_end = _add_months(s, cfg.train_months)
        val_end = _add_months(s, window_months)
        windows.append(
            Window(
                Period(_midnight(s), _midnight(train_end)),
                Period(_midnight(train_end), _midnight(val_end)),
            )
        )
        s = _add_months(s, cfg.step_months)
    if not windows:
        raise SplitError(
            f"no alcanzan los datos: hace falta al menos una ventana de {window_months} meses "
            "antes del tramo fuera de muestra"
        )
    oos = Period(_midnight(oos_start), _midnight(_add_months(last_month, 1)))
    return DataSplit(tuple(windows), oos)


def research(examples: Sequence[Example], split: DataSplit) -> tuple[Example, ...]:
    """Lo único que se puede usar para investigar: nada decidido ni resuelto fuera de muestra."""
    start = split.out_of_sample.start
    return tuple(e for e in examples if e.signal_at < start and e.known_at < start)


def folds(examples: Sequence[Example], split: DataSplit, embargo: timedelta) -> list[Fold]:
    """Entrenamiento y validación de cada ventana, sobre `research(examples)`."""
    usable = research(examples, split)
    result: list[Fold] = []
    for w in split.windows:
        val = tuple(e for e in usable if e.signal_at in w.validation)
        candidates = [e for e in usable if e.signal_at in w.train]
        embargo_period = Period(w.validation.end, w.validation.end + embargo)
        embargoed = [e for e in candidates if e.signal_at in embargo_period]
        kept = [e for e in candidates if e.signal_at not in embargo_period]
        train = tuple(e for e in kept if e.known_at < w.validation.start)
        result.append(Fold(w, train, val, len(kept) - len(train), len(embargoed)))
    return result


def inner_split(
    train: Sequence[Example], tune_fraction: float
) -> tuple[tuple[Example, ...], tuple[Example, ...]]:
    """Corte temporal interno para elegir hiperparámetros: el último `tune_fraction` del
    entrenamiento (por orden de `signal_at`) ajusta; el resto entrena, con la misma purga."""
    if not 0 < tune_fraction < 1:
        raise SplitError("la fracción del corte interno tiene que estar entre 0 y 1")
    ordered = sorted(train, key=lambda e: (e.signal_at, e.proposal_id))
    n_tune = max(1, round(len(ordered) * tune_fraction)) if ordered else 0
    tune = tuple(ordered[len(ordered) - n_tune :])
    if not tune:
        return tuple(ordered), ()
    start = tune[0].signal_at
    fit = tuple(e for e in ordered[: len(ordered) - n_tune] if e.known_at < start)
    return fit, tune
