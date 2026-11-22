"""Búsqueda de análogos v1 (#148, spec §13). Funciones puras: sin base ni reloj.

```text
ventana   = las últimas N velas diarias cerradas (solo días con sesión)
forma     = z-score de los cierres de la ventana (desvío muestral; 0 → no se usa)
distancia = DTW con banda de Sakoe-Chiba, costo |a - b|
vecinos   = los K más cercanos, de cualquier índice
```
- **Sin look-ahead:** un candidato vale solo si la última vela de su horizonte más largo cerró
  antes del cierre de la ventana consultada.
- **Sin repetir el episodio:** dos vecinos del mismo índice no pueden tener ventanas que se
  superpongan, y tampoco con la ventana consultada.
- La forma usa `float`: es una medida de parecido, no un precio. Retornos, R y tasas, que
  salen de precios, van en `Decimal` (CLAUDE.md §4).
- El ATR diario y la volatilidad realizada **no se calculan acá**: vienen de Rust
  (`daily_indicators`, CLAUDE.md §1).
"""

import math
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from datetime import date, datetime
from decimal import Decimal
from typing import Literal

from cfds_intel.analogs.config import AnalogsConfig

Hit = Literal["up", "down", "none"]
VolBand = Literal["baja", "media", "alta"]
RatesTrend = Literal["sube", "baja", "estable"]


@dataclass(frozen=True)
class Bar:
    """Vela diaria cerrada de un día con sesión, con sus indicadores de Rust."""

    day: date
    end: datetime  # fin de la vela (published_at de daily_indicator.v1)
    high: Decimal
    low: Decimal
    close: Decimal
    atr: Decimal | None
    realized_vol: Decimal | None


@dataclass(frozen=True)
class Rate:
    """Un valor publicado de un tipo de referencia (`reference_rate.v1`)."""

    ref_date: date
    published_at: datetime
    rate_pct: Decimal


@dataclass(frozen=True)
class Neighbor:
    instrument: str
    end_index: int
    distance: float


def zscore(closes: Sequence[Decimal]) -> list[float] | None:
    """Cierres normalizados; `None` si el desvío muestral es 0 (o hay menos de 2)."""
    xs = [float(c) for c in closes]
    if len(xs) < 2:
        return None
    mean = sum(xs) / len(xs)
    sd = math.sqrt(sum((x - mean) ** 2 for x in xs) / (len(xs) - 1))
    if sd == 0:
        return None
    return [(x - mean) / sd for x in xs]


def dtw(a: Sequence[float], b: Sequence[float], band: int) -> float:
    """DTW con banda de Sakoe-Chiba `band` y costo |a - b|."""
    n, m = len(a), len(b)
    inf = math.inf
    d = [[inf] * (m + 1) for _ in range(n + 1)]
    d[0][0] = 0.0
    for i in range(1, n + 1):
        for j in range(max(1, i - band), min(m, i + band) + 1):
            cost = abs(a[i - 1] - b[j - 1])
            d[i][j] = cost + min(d[i - 1][j], d[i][j - 1], d[i - 1][j - 1])
    return d[n][m]


def _window(bars: Sequence[Bar], end: int, size: int) -> list[Decimal] | None:
    first = end - size + 1
    if first < 0:
        return None
    return [b.close for b in bars[first : end + 1]]


def find_neighbors(
    series: Mapping[str, Sequence[Bar]],
    instrument: str,
    end: int,
    cfg: AnalogsConfig,
    accept: Callable[[str, int], bool] | None = None,
) -> list[Neighbor]:
    """Los K vecinos más cercanos de la ventana que termina en `series[instrument][end]`.

    `accept(instrumento, fin)` restringe los candidatos (por ejemplo, al mismo contexto).
    """
    query = series[instrument]
    closes = _window(query, end, cfg.window)
    qz = zscore(closes) if closes else None
    if qz is None:
        return []
    asked_at = query[end].end
    candidates: list[Neighbor] = []
    for inst, bars in series.items():
        for e in range(cfg.window - 1, len(bars) - cfg.max_horizon):
            # Su resultado completo tiene que existir al cierre de la consulta.
            if bars[e + cfg.max_horizon].end > asked_at:
                break  # las velas están en orden: las siguientes tampoco
            if inst == instrument and abs(e - end) < cfg.window:
                continue
            if accept is not None and not accept(inst, e):
                continue
            z = zscore(_window(bars, e, cfg.window) or [])
            if z is not None:
                candidates.append(Neighbor(inst, e, dtw(qz, z, cfg.dtw_band)))
    candidates.sort(key=lambda c: (c.distance, c.instrument, c.end_index))
    chosen: list[Neighbor] = []
    for c in candidates:
        if any(
            c.instrument == k.instrument and abs(c.end_index - k.end_index) < cfg.window
            for k in chosen
        ):
            continue
        chosen.append(c)
        if len(chosen) == cfg.neighbors:
            break
    return chosen


def forward_return(bars: Sequence[Bar], end: int, horizon: int) -> Decimal:
    return bars[end + horizon].close / bars[end].close - 1


def hit_first(bars: Sequence[Bar], end: int, cfg: AnalogsConfig) -> Hit | None:
    """Qué tocó primero: +r_up·R o -r_down·R. `None` sin ATR. Si una vela toca los dos, -R."""
    r = bars[end].atr
    if r is None or r <= 0:
        return None
    entry = bars[end].close
    up, down = entry + cfg.r_up * r, entry - cfg.r_down * r
    for b in bars[end + 1 : end + 1 + cfg.r_horizon]:
        if b.low <= down:
            return "down"
        if b.high >= up:
            return "up"
    return "none"


def nearest_rank(sorted_values: Sequence[Decimal], pct: int) -> Decimal:
    """Percentil por rango más cercano: el valor en la posición ⌈p/100 · n⌉."""
    rank = max(1, math.ceil(pct * len(sorted_values) / 100))
    return sorted_values[rank - 1]


@dataclass(frozen=True)
class HorizonSummary:
    horizon: int
    percentiles: dict[int, Decimal]
    positive_pct: Decimal


@dataclass(frozen=True)
class Summary:
    """Qué pasó después de un grupo de vecinos. `insufficient` con menos del mínimo."""

    count: int
    insufficient: bool
    horizons: list[HorizonSummary]
    hits: dict[Hit, int]
    without_atr: int


def summarize(
    series: Mapping[str, Sequence[Bar]], neighbors: Sequence[Neighbor], cfg: AnalogsConfig
) -> Summary:
    hits: dict[Hit, int] = {"up": 0, "down": 0, "none": 0}
    without_atr = 0
    for n in neighbors:
        hit = hit_first(series[n.instrument], n.end_index, cfg)
        if hit is None:
            without_atr += 1
        else:
            hits[hit] += 1
    horizons: list[HorizonSummary] = []
    if neighbors:
        for h in cfg.horizons:
            rets = sorted(forward_return(series[n.instrument], n.end_index, h) for n in neighbors)
            positive = Decimal(sum(1 for r in rets if r > 0)) / len(rets) * 100
            horizons.append(
                HorizonSummary(h, {p: nearest_rank(rets, p) for p in cfg.percentiles}, positive)
            )
    return Summary(len(neighbors), len(neighbors) < cfg.min_neighbors, horizons, hits, without_atr)


def vol_band(bars: Sequence[Bar], end: int, years: int) -> VolBand | None:
    """Tercio de la volatilidad realizada frente a la de los `years` años anteriores."""
    current = bars[end].realized_vol
    if current is None:
        return None
    day = bars[end].day
    try:
        since = day.replace(year=day.year - years)
    except ValueError:  # 29 de febrero
        since = day.replace(year=day.year - years, day=28)
    with_vol = [b for b in bars[:end] if b.realized_vol is not None]
    if not with_vol or with_vol[0].day > since:
        return None  # sin años completos de historia
    history = [b.realized_vol for b in with_vol if b.day >= since and b.realized_vol is not None]
    if not history:
        return None  # ningún valor en esos años
    rank = Decimal(sum(1 for v in history if v < current)) / len(history)
    if rank < Decimal(1) / 3:
        return "baja"
    if rank < Decimal(2) / 3:
        return "media"
    return "alta"


def rate_at(rates: Sequence[Rate], t: datetime) -> Decimal | None:
    """Valor vigente en `t`: el del día más reciente ya publicado (su última corrección)."""
    published = [r for r in rates if r.published_at <= t]
    if not published:
        return None
    return max(published, key=lambda r: (r.ref_date, r.published_at)).rate_pct


def rates_trend(
    bars: Sequence[Bar], end: int, rates: Sequence[Rate] | None, cfg: AnalogsConfig
) -> RatesTrend | None:
    """Variación del tipo de referencia en las últimas `rates_lookback` sesiones."""
    if not rates or end < cfg.rates_lookback:
        return None
    now = rate_at(rates, bars[end].end)
    before = rate_at(rates, bars[end - cfg.rates_lookback].end)
    if now is None or before is None:
        return None
    change = now - before
    if change > cfg.rates_threshold_pp:
        return "sube"
    if change < -cfg.rates_threshold_pp:
        return "baja"
    return "estable"


@dataclass(frozen=True)
class Context:
    vol: VolBand | None
    rates: RatesTrend | None


@dataclass(frozen=True)
class AnalogReport:
    instrument: str
    day: date
    context: Context
    neighbors: list[Neighbor]
    all_neighbors: Summary
    same_context: Summary


def analyze(
    series: Mapping[str, Sequence[Bar]],
    rates: Mapping[str, Sequence[Rate]],
    instrument: str,
    end: int,
    cfg: AnalogsConfig,
) -> AnalogReport:
    """Análogos de la ventana que termina en `series[instrument][end]`, con su contexto.

    `rates` va de instrumento a la serie de su tipo de referencia; sin serie, "sin dato".
    """

    cache: dict[tuple[str, int], Context] = {}

    def context(inst: str, e: int) -> Context:
        if (inst, e) not in cache:
            bars = series[inst]
            cache[(inst, e)] = Context(
                vol_band(bars, e, cfg.vol_history_years),
                rates_trend(bars, e, rates.get(inst), cfg),
            )
        return cache[(inst, e)]

    neighbors = find_neighbors(series, instrument, end, cfg)
    ctx = context(instrument, end)
    # Segunda búsqueda: los K más cercanos entre los candidatos con el mismo contexto. Con algún
    # dato del contexto faltante en la consulta no hay con qué comparar: grupo vacío.
    same = (
        find_neighbors(series, instrument, end, cfg, lambda i, e: context(i, e) == ctx)
        if ctx.vol is not None and ctx.rates is not None
        else []
    )
    return AnalogReport(
        instrument=instrument,
        day=series[instrument][end].day,
        context=ctx,
        neighbors=neighbors,
        all_neighbors=summarize(series, neighbors, cfg),
        same_context=summarize(series, same, cfg),
    )
