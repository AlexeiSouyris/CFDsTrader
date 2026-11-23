"""Índice de sentimiento v1 (spec §9.1, #213): funciones puras, sin base ni reloj.

```text
ventana_i    = valores de la fuente i publicados en los 3 años anteriores al dato actual
               (sin incluirlo; solo lo que estaba publicado en ese momento)
percentil_i  = (valores de la ventana menores que el actual + 0,5 * iguales) / tamaño de la ventana
componente_i = 2 * percentil_i - 1          (o 1 - 2 * percentil_i si la fuente se invierte)
índice       = Σ peso_i * componente_i / Σ peso_i      (solo con componentes válidos)
```

- **Dato actual** de un componente en el momento `t`: el de `observed_for` más reciente entre los
  publicados hasta `t`. Si la fuente lo corrigió, vale la última versión publicada hasta `t`.
- **Válido:** tiene al menos `warmup_years` de historia publicada antes del dato actual
  (calentamiento) y el dato no está vencido (`t - observed_for` < `expiry_days`).
- Con menos de `min_components` válidos el índice **no se emite**, y se dice por qué.
- **Sin repintado:** el valor en `t` solo usa lo publicado hasta `t`; un dato publicado después
  nunca cambia un valor ya emitido (hay test).
- Los valores crudos vienen de Rust (volatilidad realizada y caída desde el máximo) y del COT
  guardado por Python. Acá solo se combinan: ningún indicador de precio se recalcula (CLAUDE.md §1).
"""

import re
import tomllib
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime, timedelta
from decimal import Decimal, InvalidOperation, localcontext
from typing import Any

_NAME = re.compile(r"^[a-z][a-z0-9_]{0,31}$")
_INSTRUMENT = re.compile(r"^[A-Z0-9][A-Z0-9._-]{0,31}$")
_PRECISION = 28  # dígitos significativos de Decimal: fijo para que el cálculo sea reproducible

# Series del COT (config/cot.toml [series]) con las que se arma su valor crudo.
COT_LONG = "tff_lev_money_long"
COT_SHORT = "tff_lev_money_short"
COT_OPEN_INTEREST = "tff_open_interest"


class SentimentConfigError(ValueError):
    """`config/sentiment.toml` no cumple las reglas: no se calcula nada."""


@dataclass(frozen=True)
class Component:
    name: str
    weight: Decimal
    inverted: bool
    expiry: timedelta
    pending: bool
    only: frozenset[str] | None

    def applies_to(self, instrument: str) -> bool:
        return self.only is None or instrument in self.only


@dataclass(frozen=True)
class IndexConfig:
    instruments: tuple[str, ...]
    warmup_years: int
    min_components: int
    components: tuple[Component, ...]


@dataclass(frozen=True)
class Point:
    """Un dato crudo de un componente: a qué momento corresponde, cuándo se publicó y su valor."""

    observed_for: datetime
    published_at: datetime
    value: Decimal


@dataclass(frozen=True)
class ComponentState:
    name: str
    status: str  # valid, warming_up, expired, missing o pending_data
    weight: Decimal
    current: Point | None
    percentile: Decimal | None
    component: Decimal | None


@dataclass(frozen=True)
class SentimentIndex:
    instrument_id: str
    as_of: datetime
    published_at: datetime
    value: Decimal
    warmup_complete: bool
    min_components: int
    components: tuple[ComponentState, ...]

    def to_contract(self) -> dict[str, Any]:
        def utc(t: datetime | None) -> str | None:
            return None if t is None else t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")

        def dec(d: Decimal | None) -> str | None:
            return None if d is None else _plain(d)

        return {
            "instrument_id": self.instrument_id,
            "as_of": utc(self.as_of),
            "published_at": utc(self.published_at),
            "value": _plain(self.value),
            "warmup_complete": self.warmup_complete,
            "min_components": self.min_components,
            "components": [
                {
                    "name": c.name,
                    "status": c.status,
                    "weight": _plain(c.weight),
                    "observed_for": utc(c.current.observed_for) if c.current else None,
                    "published_at": utc(c.current.published_at) if c.current else None,
                    "raw": dec(c.current.value) if c.current else None,
                    "percentile": dec(c.percentile),
                    "component": dec(c.component),
                }
                for c in self.components
            ],
        }


@dataclass(frozen=True)
class NotEmitted:
    """Menos componentes válidos que el mínimo: no hay índice en ese momento."""

    instrument_id: str
    at: datetime
    valid: int
    min_components: int
    components: tuple[ComponentState, ...]

    def reason(self) -> str:
        states = ", ".join(f"{c.name}: {c.status}" for c in self.components)
        return (
            f"{self.instrument_id}: {self.valid} componentes válidos de {self.min_components} "
            f"necesarios ({states})"
        )


def load_config(text: str) -> IndexConfig:
    try:
        raw = tomllib.loads(text)
        idx = raw["index"]
        instruments = tuple(idx["instruments"])
        warmup_years = int(idx["warmup_years"])
        min_components = int(idx["min_components"])
        comps = []
        for name, c in raw["components"].items():
            only = c.get("only")
            comps.append(
                Component(
                    name=name,
                    weight=Decimal(str(c["weight"])),
                    inverted=bool(c["inverted"]),
                    expiry=timedelta(days=int(c["expiry_days"])),
                    pending=bool(c.get("pending", False)),
                    only=None if only is None else frozenset(only),
                )
            )
            extra = set(c) - {"weight", "inverted", "expiry_days", "pending", "only"}
            if extra:
                raise SentimentConfigError(f"components.{name}: claves desconocidas {extra}")
        if set(raw) - {"index", "components"} or set(idx) - {
            "instruments",
            "warmup_years",
            "min_components",
        }:
            raise SentimentConfigError("claves desconocidas en config/sentiment.toml")
    except (KeyError, TypeError, ValueError, InvalidOperation, tomllib.TOMLDecodeError) as e:
        if isinstance(e, SentimentConfigError):
            raise
        raise SentimentConfigError(f"config/sentiment.toml inválido: {e!r}") from e
    if not instruments or any(not _INSTRUMENT.match(i) for i in instruments):
        raise SentimentConfigError("index.instruments: lista vacía o instrumento inválido")
    if warmup_years < 1 or min_components < 1:
        raise SentimentConfigError("index.warmup_years y index.min_components tienen que ser ≥ 1")
    for c in comps:
        if not _NAME.match(c.name) or not c.weight.is_finite() or c.weight <= 0:
            raise SentimentConfigError(f"components.{c.name}: nombre o peso inválido")
        if c.expiry <= timedelta(0):
            raise SentimentConfigError(f"components.{c.name}: expiry_days tiene que ser > 0")
    if min_components > sum(1 for c in comps if not c.pending):
        raise SentimentConfigError("index.min_components supera los componentes con datos")
    return IndexConfig(instruments, warmup_years, min_components, tuple(comps))


def cot_points(rows: Iterable[tuple[str, datetime, datetime, Decimal]]) -> list[Point]:
    """Valor crudo del COT: (largos - cortos de Leveraged Funds) / interés abierto.

    `rows` = (serie, observed_for, published_at, valor) de `sentiment_observations` de un
    instrumento. Cada informe da un punto por cada momento en que cambió alguna de sus tres
    series (una corrección de la fuente es un punto nuevo, publicado cuando se leyó). El punto
    vale desde que estuvieron publicadas las tres. Interés abierto en 0: sin punto.
    """
    by_report: dict[datetime, dict[str, list[tuple[datetime, Decimal]]]] = {}
    for series, observed_for, published_at, value in rows:
        if series in (COT_LONG, COT_SHORT, COT_OPEN_INTEREST):
            by_report.setdefault(observed_for, {}).setdefault(series, []).append(
                (published_at, value)
            )
    points = []
    for observed_for, by_series in by_report.items():
        if len(by_series) < 3:
            continue
        moments = sorted({p for versions in by_series.values() for p, _ in versions})
        for at in moments:
            latest: dict[str, Decimal] = {}
            for name, versions in by_series.items():
                known = [(p, v) for p, v in versions if p <= at]
                if known:
                    latest[name] = max(known)[1]
            if len(latest) < 3 or latest[COT_OPEN_INTEREST] == 0:
                continue
            with localcontext() as ctx:
                ctx.prec = _PRECISION
                value = (latest[COT_LONG] - latest[COT_SHORT]) / latest[COT_OPEN_INTEREST]
            points.append(Point(observed_for, at, value))
    return sorted(points, key=lambda p: (p.observed_for, p.published_at))


def index_at(
    instrument: str,
    data: Mapping[str, Sequence[Point]],
    cfg: IndexConfig,
    at: datetime,
) -> SentimentIndex | NotEmitted:
    """El índice de `instrument` con lo publicado hasta `at` (incluido)."""
    states = tuple(
        _state(c, data.get(c.name, ()), cfg.warmup_years, at)
        for c in cfg.components
        if c.applies_to(instrument)
    )
    valid = [s for s in states if s.status == "valid"]
    if len(valid) < cfg.min_components:
        return NotEmitted(instrument, at, len(valid), cfg.min_components, states)
    with localcontext() as ctx:
        ctx.prec = _PRECISION
        total = sum((s.weight for s in valid), Decimal(0))
        value = sum((s.weight * s.component for s in valid if s.component is not None), Decimal(0))
        value /= total
    used = [s.current for s in valid if s.current is not None]
    with_data = [s for s in states if s.status != "pending_data"]
    return SentimentIndex(
        instrument_id=instrument,
        as_of=max(p.observed_for for p in used),
        published_at=max(p.published_at for p in used),
        value=value,
        warmup_complete=all(s.status != "warming_up" for s in with_data),
        min_components=cfg.min_components,
        components=states,
    )


def index_series(
    instrument: str, data: Mapping[str, Sequence[Point]], cfg: IndexConfig
) -> tuple[list[SentimentIndex], list[NotEmitted]]:
    """El índice cada vez que un componente publica un dato (spec: "cuándo se recalcula").

    Un momento que no cambia el índice (por ejemplo, un dato de un componente todavía en
    calentamiento) no emite otro registro igual.
    """
    moments = sorted({p.published_at for points in data.values() for p in points})
    emitted: list[SentimentIndex] = []
    skipped: list[NotEmitted] = []
    for at in moments:
        r = index_at(instrument, data, cfg, at)
        if isinstance(r, NotEmitted):
            skipped.append(r)
        elif not emitted or (r.published_at, r.value) != (
            emitted[-1].published_at,
            emitted[-1].value,
        ):
            emitted.append(r)
    return emitted, skipped


def _state(
    c: Component, points: Sequence[Point], warmup_years: int, at: datetime
) -> ComponentState:
    def state(status: str, cur: Point | None = None) -> ComponentState:
        return ComponentState(c.name, status, c.weight, cur, None, None)

    if c.pending:
        return state("pending_data")
    known = [p for p in points if p.published_at <= at]
    if not known:
        return state("missing")
    cur = max(known, key=lambda p: (p.observed_for, p.published_at))
    if min(p.published_at for p in known) > _years_before(cur.published_at, warmup_years):
        return state("warming_up", cur)
    if at - cur.observed_for >= c.expiry:
        return state("expired", cur)
    window = _window(known, cur, warmup_years)
    if not window:
        return state("warming_up", cur)
    with localcontext() as ctx:
        ctx.prec = _PRECISION
        less = sum(1 for v in window if v < cur.value)
        equal = sum(1 for v in window if v == cur.value)
        pct = (Decimal(less) + Decimal("0.5") * equal) / len(window)
        comp = 1 - 2 * pct if c.inverted else 2 * pct - 1
    return ComponentState(c.name, "valid", c.weight, cur, pct, comp)


def _window(known: Sequence[Point], cur: Point, years: int) -> list[Decimal]:
    """Valores publicados en los `years` años anteriores al dato actual, sin incluirlo.

    De cada momento observado vale la última versión publicada antes que el dato actual.
    """
    start = _years_before(cur.published_at, years)
    latest: dict[datetime, Point] = {}
    for p in known:
        if p.observed_for == cur.observed_for or not start <= p.published_at < cur.published_at:
            continue
        prev = latest.get(p.observed_for)
        if prev is None or p.published_at > prev.published_at:
            latest[p.observed_for] = p
    return [p.value for p in latest.values()]


def _years_before(t: datetime, years: int) -> datetime:
    try:
        return t.replace(year=t.year - years)
    except ValueError:  # 29 de febrero
        return t.replace(year=t.year - years, day=28)


def _plain(d: Decimal) -> str:
    """Decimal del contrato: sin exponente y sin "-0"."""
    text = format(d.normalize() if d != 0 else Decimal(0), "f")
    return text
