"""Configuración de los análogos (`config/analogs.toml`), validada al cargar."""

import tomllib
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation


class AnalogsConfigError(ValueError):
    """Configuración inválida: no se calcula nada."""


@dataclass(frozen=True)
class AnalogsConfig:
    window: int
    dtw_band: int
    neighbors: int
    min_neighbors: int
    horizons: tuple[int, ...]
    r_up: Decimal
    r_down: Decimal
    r_horizon: int
    percentiles: tuple[int, ...]
    rates_lookback: int
    rates_threshold_pp: Decimal

    @property
    def max_horizon(self) -> int:
        return max(*self.horizons, self.r_horizon)


def _pos_int(raw: dict[str, object], key: str, minimum: int = 1) -> int:
    v = raw.get(key)
    if not (isinstance(v, int) and not isinstance(v, bool) and v >= minimum):
        raise AnalogsConfigError(f"{key} tiene que ser un entero ≥ {minimum}")
    return v


def load_config(text: str) -> AnalogsConfig:
    raw = tomllib.loads(text)
    shape, out, ctx = raw.get("shape", {}), raw.get("outcome", {}), raw.get("context", {})
    horizons = out.get("horizons")
    pcts = out.get("percentiles")
    if not (
        isinstance(horizons, list)
        and horizons
        and all(isinstance(h, int) and not isinstance(h, bool) and h >= 1 for h in horizons)
    ):
        raise AnalogsConfigError("outcome.horizons: lista de enteros ≥ 1")
    if not (
        isinstance(pcts, list)
        and pcts
        and all(isinstance(p, int) and not isinstance(p, bool) and 0 < p < 100 for p in pcts)
    ):
        raise AnalogsConfigError("outcome.percentiles: lista de enteros entre 1 y 99")
    try:
        threshold = Decimal(str(ctx.get("rates_threshold_pp")))
        r_up, r_down = Decimal(str(out.get("r_up"))), Decimal(str(out.get("r_down")))
    except InvalidOperation as e:
        raise AnalogsConfigError(f"número inválido: {e}") from e
    if not (threshold >= 0 and r_up > 0 and r_down > 0):
        raise AnalogsConfigError("r_up, r_down > 0 y rates_threshold_pp ≥ 0")
    cfg = AnalogsConfig(
        window=_pos_int(shape, "window", 3),
        dtw_band=_pos_int(shape, "dtw_band", 0),
        neighbors=_pos_int(shape, "neighbors"),
        min_neighbors=_pos_int(shape, "min_neighbors"),
        horizons=tuple(horizons),
        r_up=r_up,
        r_down=r_down,
        r_horizon=_pos_int(out, "r_horizon"),
        percentiles=tuple(pcts),
        rates_lookback=_pos_int(ctx, "rates_lookback"),
        rates_threshold_pp=threshold,
    )
    if cfg.min_neighbors > cfg.neighbors:
        raise AnalogsConfigError("min_neighbors no puede ser mayor que neighbors")
    return cfg
