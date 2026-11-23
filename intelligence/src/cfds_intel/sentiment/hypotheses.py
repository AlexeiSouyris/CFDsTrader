"""Arnés de las hipótesis H1-H7 de la spec §9.1 (#227): funciones puras, sin base ni reloj.

Criterio fijo, escrito antes de mirar los datos:
- "La media" es el retorno medio del mismo instrumento y horizonte en el mismo tramo de datos
  (todas las velas que recibe la prueba).
- Una hipótesis está **apoyada** solo si el intervalo de confianza del 95 % (bootstrap por
  percentiles) de la diferencia contra la media no incluye el 0 y está del lado esperado.
- Retornos de cierre a cierre, sin costos.
- Si un grupo tiene menos de `min_cases` casos: **datos insuficientes**, y no se calcula nada.

Cada evento se mide desde el cierre de la primera vela que termina en el momento del evento o
después (nunca antes de conocerse). Dos eventos que caen en la misma vela cuentan una vez.

H6 compara frecuencias (toca el objetivo antes que el stop) contra entradas aleatorias: los
resultados de cada entrada los simula el motor (Rust); acá solo se comparan. H7 queda pendiente
hasta tener volumen real.
"""

import bisect
import random
import tomllib
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal, InvalidOperation
from enum import StrEnum

SUPPORTED = "apoyada"
NOT_SUPPORTED = "no apoyada"
INSUFFICIENT = "datos insuficientes"
PENDING = "pendiente"


class Expect(StrEnum):
    HIGHER = "mayor"
    LOWER = "menor"


class HypothesisConfigError(ValueError):
    """`config/hypotheses.toml` no cumple las reglas."""


@dataclass(frozen=True)
class TestConfig:
    confidence: Decimal
    min_cases: int
    iterations: int
    seed: int


@dataclass(frozen=True)
class HypothesesConfig:
    test: TestConfig
    ig_majority_pct: Decimal
    h1_days: int
    h2_days: int
    h3_days: tuple[int, ...]
    h4_hours: int
    h4_days: int
    h5_days: int
    h7_days: int


@dataclass(frozen=True)
class Bar:
    """Vela cerrada: cuándo terminó y su cierre."""

    end: datetime
    close: Decimal


@dataclass(frozen=True)
class TestResult:
    name: str
    expect: Expect
    status: str
    cases: int
    population: int
    difference: Decimal | None = None
    ci_low: Decimal | None = None
    ci_high: Decimal | None = None


def load_config(text: str) -> HypothesesConfig:
    try:
        raw = tomllib.loads(text)
        t, c, h = raw["test"], raw["conditions"], raw["horizons"]
        for section, keys in (
            (raw, {"test", "conditions", "horizons"}),
            (t, {"confidence", "min_cases", "bootstrap_iterations", "seed"}),
            (c, {"ig_majority_pct"}),
            (h, {"h1_days", "h2_days", "h3_days", "h4_hours", "h4_days", "h5_days", "h7_days"}),
        ):
            if set(section) != keys:
                raise HypothesisConfigError(f"claves distintas de las esperadas: {sorted(keys)}")
        cfg = HypothesesConfig(
            test=TestConfig(
                Decimal(str(t["confidence"])),
                int(t["min_cases"]),
                int(t["bootstrap_iterations"]),
                int(t["seed"]),
            ),
            ig_majority_pct=Decimal(str(c["ig_majority_pct"])),
            h1_days=int(h["h1_days"]),
            h2_days=int(h["h2_days"]),
            h3_days=tuple(int(x) for x in h["h3_days"]),
            h4_hours=int(h["h4_hours"]),
            h4_days=int(h["h4_days"]),
            h5_days=int(h["h5_days"]),
            h7_days=int(h["h7_days"]),
        )
    except (KeyError, TypeError, ValueError, InvalidOperation, tomllib.TOMLDecodeError) as e:
        if isinstance(e, HypothesisConfigError):
            raise
        raise HypothesisConfigError(f"config/hypotheses.toml inválido: {e!r}") from e
    horizons = (cfg.h1_days, cfg.h2_days, *cfg.h3_days, cfg.h4_hours, cfg.h4_days, cfg.h5_days)
    if not 0 < cfg.test.confidence < 1 or cfg.test.min_cases < 2 or cfg.test.iterations < 100:
        raise HypothesisConfigError("test: confidence en (0, 1), min_cases ≥ 2, iteraciones ≥ 100")
    if not 50 < cfg.ig_majority_pct <= 100 or min(horizons) < 1 or cfg.h7_days < 1:
        raise HypothesisConfigError("ig_majority_pct en (50, 100] y horizontes ≥ 1")
    return cfg


def forward_returns(bars: Sequence[Bar], horizon: int) -> list[Decimal]:
    """Retorno de cierre a cierre a `horizon` velas de cada vela que lo tiene: la población."""
    return [bars[i + horizon].close / bars[i].close - 1 for i in range(len(bars) - horizon)]


def event_returns(bars: Sequence[Bar], events: Sequence[datetime], horizon: int) -> list[Decimal]:
    """Retornos a `horizon` velas desde la primera vela que termina en cada evento o después."""
    ends = [b.end for b in bars]
    starts = sorted({bisect.bisect_left(ends, e) for e in events})
    return [bars[i + horizon].close / bars[i].close - 1 for i in starts if i + horizon < len(bars)]


def mean_test(
    name: str,
    cases: Sequence[Decimal],
    population: Sequence[Decimal],
    expect: Expect,
    cfg: TestConfig,
) -> TestResult:
    """Media de los casos menos la media de la población, con su intervalo bootstrap."""
    return _bootstrap(name, list(cases), list(population), expect, cfg)


def frequency_test(
    name: str, hits: Sequence[bool], random_hits: Sequence[bool], cfg: TestConfig
) -> TestResult:
    """Frecuencia de aciertos de los casos menos la de las entradas aleatorias (H6)."""
    as_dec = [Decimal(int(h)) for h in hits]
    base = [Decimal(int(h)) for h in random_hits]
    return _bootstrap(name, as_dec, base, Expect.HIGHER, cfg)


def _bootstrap(
    name: str,
    cases: list[Decimal],
    population: list[Decimal],
    expect: Expect,
    cfg: TestConfig,
) -> TestResult:
    n, m = len(cases), len(population)
    if n < cfg.min_cases or m < cfg.min_cases:
        return TestResult(name, expect, INSUFFICIENT, n, m)
    diff = _mean(cases) - _mean(population)
    rng = random.Random(cfg.seed)  # noqa: S311 (estadística reproducible, no criptografía)
    diffs = sorted(
        _mean(rng.choices(cases, k=n)) - _mean(rng.choices(population, k=m))
        for _ in range(cfg.iterations)
    )
    tail = (1 - cfg.confidence) / 2
    low = diffs[int(tail * cfg.iterations)]
    high = diffs[min(cfg.iterations - 1, int((1 - tail) * cfg.iterations))]
    ok = low > 0 if expect is Expect.HIGHER else high < 0
    return TestResult(name, expect, SUPPORTED if ok else NOT_SUPPORTED, n, m, diff, low, high)


def _mean(xs: Sequence[Decimal]) -> Decimal:
    return sum(xs, Decimal(0)) / len(xs)


@dataclass(frozen=True)
class IndexPoint:
    """Un valor del índice de sentimiento y desde cuándo existe (sentiment.v1)."""

    published_at: datetime
    value: Decimal


@dataclass(frozen=True)
class IgPoint:
    """% de clientes de IG largos y desde cuándo existe (sentiment_observation.v1)."""

    published_at: datetime
    long_pct: Decimal


def _when(index: Sequence[IndexPoint], keep: Callable[[Decimal], bool]) -> list[datetime]:
    return [p.published_at for p in index if keep(p.value)]


def _known_index(index: Sequence[IndexPoint], at: datetime) -> Decimal | None:
    """Último valor del índice publicado hasta `at` (nunca uno posterior)."""
    known = [p for p in index if p.published_at <= at]
    return max(known, key=lambda p: p.published_at).value if known else None


def run_all(
    index: Sequence[IndexPoint],
    ig: Sequence[IgPoint],
    fomo: Sequence[datetime],
    climax: Sequence[datetime],
    daily: Sequence[Bar],
    hourly: Sequence[Bar],
    h6_hits: Sequence[bool],
    h6_random_hits: Sequence[bool],
    fear: Decimal,
    euphoria: Decimal,
    cfg: HypothesesConfig,
) -> list[TestResult]:
    """Todas las pruebas de H1-H7 de **un** instrumento y **un** tramo de datos.

    - `index`: el índice de sentimiento; `fear` y `euphoria`, los umbrales de
      `config/sentiment.toml` [states].
    - `ig`: % de clientes de IG largos (H3).
    - `fomo` y `climax`: fin de cada vela de 1h con ese estado (Rust, `cfds-regime`).
    - `daily` y `hourly`: velas cerradas del instrumento en el tramo, en orden.
    - `h6_hits` / `h6_random_hits`: si cada rechazo / entrada aleatoria tocó el objetivo antes
      que el stop (los simula el motor, con el mismo stop, objetivo y frecuencia).

    Cada hipótesis, horizonte e instrumento es una prueba: todas van al registro de experimentos
    y cuentan para el Sharpe deflactado (spec §9.1).
    """
    t = cfg.test

    def daily_test(name: str, events: Sequence[datetime], days: int, expect: Expect) -> TestResult:
        return mean_test(
            name, event_returns(daily, events, days), forward_returns(daily, days), expect, t
        )

    out = [
        daily_test("H1", _when(index, lambda v: v <= fear), cfg.h1_days, Expect.HIGHER),
        daily_test("H2", _when(index, lambda v: v >= euphoria), cfg.h2_days, Expect.LOWER),
    ]
    longs = [p.published_at for p in ig if p.long_pct >= cfg.ig_majority_pct]
    shorts = [p.published_at for p in ig if p.long_pct <= 100 - cfg.ig_majority_pct]
    for days in cfg.h3_days:
        out.append(daily_test(f"H3 largos {days}d", longs, days, Expect.LOWER))
        out.append(daily_test(f"H3 cortos {days}d", shorts, days, Expect.HIGHER))
    out.append(
        mean_test(
            "H4 continúa",
            event_returns(hourly, fomo, cfg.h4_hours),
            forward_returns(hourly, cfg.h4_hours),
            Expect.HIGHER,
            t,
        )
    )
    out.append(daily_test("H4 revierte", fomo, cfg.h4_days, Expect.LOWER))
    panic = [c for c in climax if (v := _known_index(index, c)) is not None and v <= fear]
    out.append(daily_test("H5", panic, cfg.h5_days, Expect.HIGHER))
    out.append(frequency_test("H6", h6_hits, h6_random_hits, t))
    out.append(TestResult("H7", Expect.LOWER, PENDING, 0, 0))
    return out
