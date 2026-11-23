"""Dataset del modelo de ML v1 (docs/spec/ml-v1.md §1 y §3, #158).

Una fila por propuesta de una corrida de backtest (`backtest_runs`), con su resultado simulado
(`outcomes`) y sus variables (`ml_features`, que arma Rust). Python **no recalcula** indicadores
(CLAUDE.md §1): solo lee y codifica.

- **Etiqueta:** `y = 1` si tocó TP1 (`hit_tp1`), si no `0`. Las salidas por tiempo y los cierres
  forzados cuentan como `0` (decisión 1 de la spec §11).
- **Solo información al cierre de `t`:** las variables son de la propuesta y de `ml_features`, con
  `as_of` igual a `signal_at`. Si no coincide, la fila es un error y no entra.
- **Calentamiento:** una propuesta sin variables no entra y se cuenta aparte; no se rellena.
- Los cálculos de distancias usan `Decimal` y se pasan a `float` al final: son razones, no dinero.
"""

from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal
from typing import Any

import psycopg

from cfds_intel.models.config import DatasetConfig

TRIGGERS = ("candle_pattern", "double_break", "level_rejection", "sweep")
COMPONENTS = (
    "trend",
    "sma_side",
    "candle_pattern",
    "double_break",
    "level_rejection",
    "sweep",
    "fvg_or_order_block",
    "recent_choch",
    "news",
)
TERCILES = ("low", "mid", "high")
BANDS = ("open", "mid", "close")


def feature_names(cfg: DatasetConfig) -> tuple[str, ...]:
    """Nombres de las columnas de X, en orden (spec §3)."""
    return (
        "side_long",
        *(f"instrument_{i}" for i in cfg.instruments),
        *(f"trigger_{t}" for t in TRIGGERS),
        *(f"points_{c}" for c in COMPONENTS),
        "score",
        "stop_atr",
        "tp1_r",
        *(f"vol_{t}" for t in TERCILES),
        *(f"session_{b}" for b in BANDS),
    )


@dataclass(frozen=True)
class Row:
    """Una propuesta con su resultado y sus variables, tal como salen de PostgreSQL."""

    proposal_id: str
    instrument_id: str
    side: str
    signal_at: datetime
    reference: Decimal
    stop: Decimal
    tp1: Decimal
    score: int
    breakdown: Sequence[dict[str, Any]]
    triggers: Sequence[str]
    hit_tp1: bool | None
    exits: Sequence[dict[str, Any]] | None
    as_of: datetime | None
    atr14_m15: Decimal | None
    realized_vol_tercile: str | None
    session_band: str | None


@dataclass(frozen=True)
class Example:
    proposal_id: str
    signal_at: datetime
    # Cuándo se conoció el resultado (la última salida): lo usa la purga (spec §5).
    known_at: datetime
    x: tuple[float, ...]
    y: int
    # Para la tabla de §13 (B1): gatillos y puntaje de la propuesta.
    triggers: tuple[str, ...]
    score: int


@dataclass(frozen=True)
class Dataset:
    examples: tuple[Example, ...]
    feature_names: tuple[str, ...]
    # Lo que no entró, contado aparte (spec §3: nunca se rellena).
    other_instrument: int
    without_outcome: int
    without_features: int


class DatasetError(ValueError):
    """Una fila contradice el contrato (por ejemplo, variables con otra hora que la señal)."""


def _parse_utc(text: str) -> datetime:
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def _one_hot(value: str, values: Sequence[str]) -> list[float]:
    return [1.0 if value == v else 0.0 for v in values]


def encode(row: Row, cfg: DatasetConfig) -> Example:
    """Una propuesta completa → su ejemplo. Supone resultado y variables presentes."""
    if row.hit_tp1 is None or not row.exits:
        raise DatasetError(f"{row.proposal_id}: sin resultado")
    if (
        row.as_of is None
        or row.atr14_m15 is None
        or row.realized_vol_tercile is None
        or row.session_band is None
    ):
        raise DatasetError(f"{row.proposal_id}: sin variables")
    if row.as_of != row.signal_at:
        raise DatasetError(
            f"{row.proposal_id}: variables al {row.as_of}, señal al {row.signal_at} (§3)"
        )
    if row.realized_vol_tercile not in TERCILES or row.session_band not in BANDS:
        raise DatasetError(f"{row.proposal_id}: tercio o franja fuera del contrato")
    risk = abs(row.reference - row.stop)
    if risk == 0 or row.atr14_m15 <= 0:
        raise DatasetError(f"{row.proposal_id}: riesgo o ATR en cero")
    points = {str(b["component"]): int(b["points"]) for b in row.breakdown}
    x = [
        1.0 if row.side == "long" else 0.0,
        *_one_hot(row.instrument_id, cfg.instruments),
        *(1.0 if t in row.triggers else 0.0 for t in TRIGGERS),
        *(float(points.get(c, 0)) for c in COMPONENTS),
        float(row.score),
        float(risk / row.atr14_m15),
        float(abs(row.tp1 - row.reference) / risk),
        *_one_hot(row.realized_vol_tercile, TERCILES),
        *_one_hot(row.session_band, BANDS),
    ]
    known_at = max(_parse_utc(str(e["at"])) for e in row.exits)
    return Example(
        proposal_id=row.proposal_id,
        signal_at=row.signal_at,
        known_at=known_at,
        x=tuple(x),
        y=1 if row.hit_tp1 else 0,
        triggers=tuple(row.triggers),
        score=row.score,
    )


def build(rows: Iterable[Row], cfg: DatasetConfig) -> Dataset:
    """Arma el dataset en orden de `signal_at` (y de id, para que sea reproducible)."""
    examples: list[Example] = []
    other = no_outcome = no_features = 0
    for row in sorted(rows, key=lambda r: (r.signal_at, r.proposal_id)):
        if row.instrument_id not in cfg.instruments:
            other += 1
        elif row.hit_tp1 is None or not row.exits:
            no_outcome += 1
        elif row.as_of is None:
            no_features += 1
        else:
            examples.append(encode(row, cfg))
    return Dataset(tuple(examples), feature_names(cfg), other, no_outcome, no_features)


_QUERY = """
SELECT p.id, p.instrument_id, p.side, p.signal_at, p.reference, p.stop, p.tp1, p.score,
       p.breakdown, p.triggers, o.hit_tp1, o.exits,
       f.as_of, f.atr14_m15, f.realized_vol_tercile, f.session_band
FROM proposals p
LEFT JOIN outcomes o ON o.proposal_id = p.id AND o.kind = 'simulated'
LEFT JOIN ml_features f ON f.proposal_id = p.id
WHERE p.run_id = %s AND p.source = 'backtest'
"""


def load(conn: psycopg.Connection, run_id: str, cfg: DatasetConfig) -> Dataset:
    """Lee una corrida guardada en `backtest_runs` (#200). Sin esa corrida, es un error."""
    with conn.cursor() as cur:
        cur.execute("SELECT 1 FROM backtest_runs WHERE run_id = %s", (run_id,))
        if cur.fetchone() is None:
            raise DatasetError(f"la corrida {run_id!r} no está en backtest_runs")
        cur.execute(_QUERY, (run_id,))
        rows = [Row(*r) for r in cur.fetchall()]
    return build(rows, cfg)
