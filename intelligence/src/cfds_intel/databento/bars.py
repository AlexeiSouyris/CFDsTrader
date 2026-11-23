"""Barras de 1 minuto de Databento: estimación del costo, conversión y validación (ADR 0011, #247).

Reglas:
- Solo el SDK oficial (`databento`) y sus métodos documentados. La metadata (`get_cost`,
  `get_record_count`, `get_dataset_range`) no se cobra: "Access to metadata, symbology, and
  account management is free" (docs: api-reference-historical/basics).
- `ts_event` es el **inicio** de la barra, en nanosegundos UTC. Tiene que caer justo en un minuto.
- Los precios llegan como enteros con escala 1e-9: se pasan a `Decimal` exacto, nunca a float.
- Un minuto sin operaciones no trae barra (docs: schemas-and-data-formats/ohlcv). Por eso un
  hueco no es un error de la fuente, pero `--sample` lo informa.
- Cualquier barra que no cumple las reglas hace fallar el pedido entero: no se guarda nada.
"""

from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from itertools import pairwise
from typing import Any, Protocol

from cfds_intel.databento.config import DatabentoConfig, SymbolConfig

SOURCE = "databento"
_SCALE = Decimal(1_000_000_000)
_UNDEF_PRICE = 9_223_372_036_854_775_807  # databento_dbn.UNDEF_PRICE (INT64_MAX)
_NS_PER_MINUTE = 60_000_000_000
_EPOCH = datetime(1970, 1, 1, tzinfo=UTC)


class DatabentoError(ValueError):
    """Una respuesta o una barra no cumple las reglas: no se guarda nada."""


class BudgetError(DatabentoError):
    """La estimación pasa `budget.max_usd`: no se baja nada."""


class Metadata(Protocol):
    def get_cost(self, **kwargs: Any) -> float: ...
    def get_record_count(self, **kwargs: Any) -> int: ...
    def get_dataset_range(self, dataset: str) -> Mapping[str, Any]: ...


class Timeseries(Protocol):
    def get_range(self, **kwargs: Any) -> Iterable[Any]: ...


class Client(Protocol):
    """Lo que se usa de `databento.Historical` (se reemplaza en los tests, sin red)."""

    @property
    def metadata(self) -> Metadata: ...
    @property
    def timeseries(self) -> Timeseries: ...


@dataclass(frozen=True)
class Bar:
    """Una barra de `contracts/futures_bar.v1`."""

    dataset: str
    symbol: str
    instrument_id: str
    contract_id: int
    start: datetime
    open: Decimal
    high: Decimal
    low: Decimal
    close: Decimal
    volume: int

    def to_contract(self) -> dict[str, object]:
        return {
            "source": SOURCE,
            "dataset": self.dataset,
            "symbol": self.symbol,
            "instrument_id": self.instrument_id,
            "contract_id": self.contract_id,
            "start": self.start.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "open": str(self.open),
            "high": str(self.high),
            "low": str(self.low),
            "close": str(self.close),
            "volume": self.volume,
        }


def _price(raw: object, where: str) -> Decimal:
    if not isinstance(raw, int) or isinstance(raw, bool) or raw == _UNDEF_PRICE or raw <= 0:
        raise DatabentoError(f"{where}: precio inválido {raw!r}")
    return Decimal(raw) / _SCALE


def to_bar(record: Any, sym: SymbolConfig) -> Bar:
    """Convierte un registro OHLCV del SDK (atributos enteros) en una [`Bar`]."""
    ts = getattr(record, "ts_event", None)
    where = f"{sym.symbol} ts_event={ts!r}"
    if not isinstance(ts, int) or ts < 0 or ts % _NS_PER_MINUTE != 0:
        raise DatabentoError(f"{where}: no es el inicio de un minuto en UTC")
    contract, volume = getattr(record, "instrument_id", None), getattr(record, "volume", None)
    if not isinstance(contract, int) or contract <= 0:
        raise DatabentoError(f"{where}: instrument_id inválido {contract!r}")
    if not isinstance(volume, int) or volume < 0:
        raise DatabentoError(f"{where}: volumen inválido {volume!r}")
    o, h, lo, c = (
        _price(getattr(record, k, None), where) for k in ("open", "high", "low", "close")
    )
    if not (lo <= min(o, c) and h >= max(o, c)):
        raise DatabentoError(f"{where}: OHLC incoherente")
    return Bar(
        dataset=sym.dataset,
        symbol=sym.symbol,
        instrument_id=sym.instrument_id,
        contract_id=contract,
        start=_EPOCH + timedelta(microseconds=ts // 1000),
        open=o,
        high=h,
        low=lo,
        close=c,
        volume=volume,
    )


def to_bars(records: Iterable[Any], sym: SymbolConfig, start: datetime, end: datetime) -> list[Bar]:
    """Barras de `[start, end)`, en orden estricto y sin repetir."""
    bars = [to_bar(r, sym) for r in records]
    for prev, cur in pairwise(bars):
        if cur.start <= prev.start:
            raise DatabentoError(f"{sym.symbol}: barras fuera de orden en {cur.start}")
    if bars and (bars[0].start < start or bars[-1].start >= end):
        raise DatabentoError(f"{sym.symbol}: barras fuera del período pedido")
    return bars


def _midnight(d: date) -> datetime:
    return datetime(d.year, d.month, d.day, tzinfo=UTC)


def _request(cfg: DatabentoConfig, sym: SymbolConfig, start: date, end: date) -> dict[str, Any]:
    return {
        "dataset": sym.dataset,
        "symbols": [sym.symbol],
        "schema": cfg.schema,
        "stype_in": cfg.stype_in,
        "start": start.isoformat(),
        "end": end.isoformat(),
    }


@dataclass(frozen=True)
class Estimate:
    symbol: SymbolConfig
    start: date
    end: date
    cost_usd: Decimal
    records: int
    skipped: str | None = None


def dataset_start(client: Client, dataset: str, schema: str) -> date:
    """Primer día con datos del esquema en el dataset (`get_dataset_range`, gratis)."""
    r = client.metadata.get_dataset_range(dataset)
    per_schema = r.get("schema")
    raw = r.get("start")
    if isinstance(per_schema, Mapping) and isinstance(per_schema.get(schema), Mapping):
        raw = per_schema[schema].get("start", raw)
    if not isinstance(raw, str):
        if isinstance(per_schema, Mapping) and schema not in per_schema:
            raise DatabentoError(f"{dataset} no tiene el esquema {schema}")
        raise DatabentoError(f"{dataset}: get_dataset_range sin start")
    try:
        return date.fromisoformat(raw[:10])
    except ValueError as e:
        raise DatabentoError(f"{dataset}: start inválido {raw!r}") from e


def chunk_cost(
    client: Client, cfg: DatabentoConfig, sym: SymbolConfig, start: date, end: date
) -> Decimal:
    """Costo en USD de `[start, end)` según `metadata.get_cost` (gratis)."""
    return Decimal(repr(float(client.metadata.get_cost(**_request(cfg, sym, start, end)))))


def estimate(
    client: Client,
    cfg: DatabentoConfig,
    end: date,
    loaded_until: Mapping[tuple[str, str], date],
) -> list[Estimate]:
    """Costo y cantidad de barras de lo que **falta** bajar de cada símbolo, hasta `end`."""
    starts: dict[str, date] = {}
    out: list[Estimate] = []
    for sym in cfg.symbols:
        if sym.dataset not in starts:
            starts[sym.dataset] = dataset_start(client, sym.dataset, cfg.schema)
        # Si el dataset empieza después de `start` (Eurex), se baja desde su inicio.
        available = starts[sym.dataset]
        start = max(cfg.start, available, loaded_until.get((sym.dataset, sym.symbol), cfg.start))
        if start >= end:
            out.append(Estimate(sym, start, end, Decimal(0), 0, "ya está todo guardado"))
            continue
        cost = chunk_cost(client, cfg, sym, start, end)
        records = int(client.metadata.get_record_count(**_request(cfg, sym, start, end)))
        if cost < 0 or records < 0:
            raise DatabentoError(f"{sym.symbol}: estimación negativa")
        out.append(Estimate(sym, start, end, cost, records))
    return out


def check_budget(estimates: Iterable[Estimate], max_usd: Decimal) -> Decimal:
    """Total estimado. Si pasa `max_usd`, [`BudgetError`] y no se baja nada."""
    total = sum((e.cost_usd for e in estimates), Decimal(0))
    if total > max_usd:
        raise BudgetError(f"la estimación (USD {total}) pasa el tope de USD {max_usd}")
    return total


def months(start: date, end: date) -> list[tuple[date, date]]:
    """`[start, end)` partido en meses de calendario, para bajar y registrar de a uno."""
    out: list[tuple[date, date]] = []
    a = start
    while a < end:
        nxt = date(a.year + (a.month == 12), a.month % 12 + 1, 1)
        out.append((a, min(nxt, end)))
        a = nxt
    return out


def fetch_bars(
    client: Client, cfg: DatabentoConfig, sym: SymbolConfig, start: date, end: date
) -> list[Bar]:
    """Baja las barras de `[start, end)` (esto **sí** consume crédito)."""
    records = client.timeseries.get_range(**_request(cfg, sym, start, end))
    return to_bars(records, sym, _midnight(start), _midnight(end))


@dataclass(frozen=True)
class SampleReport:
    symbol: str
    day: date
    bars: int
    hours: Decimal
    first: datetime | None
    last: datetime | None
    longest_gap_minutes: int
    gaps: list[tuple[datetime, int]]
    contracts: list[int]
    problems: list[str]


def validate_day(bars: list[Bar], sym: SymbolConfig, day: date, max_gap: int) -> SampleReport:
    """Validación de un día UTC: sesión de al menos `min_session_hours` con barras y sin huecos
    de más de `max_gap` minutos, salvo el más largo (la pausa diaria del mercado)."""
    problems: list[str] = []
    gaps = [
        (prev.start, int((cur.start - prev.start).total_seconds() // 60) - 1)
        for prev, cur in pairwise(bars)
    ]
    gaps = [g for g in gaps if g[1] > 0]
    longest = max((g[1] for g in gaps), default=0)
    others = sorted(gaps, key=lambda g: g[1], reverse=True)[1:]
    big = [g for g in others if g[1] > max_gap]
    hours = Decimal(len(bars)) / 60
    if hours < sym.min_session_hours:
        problems.append(f"solo {hours:.1f} h con barras (mínimo {sym.min_session_hours} h)")
    if big:
        problems.append(f"{len(big)} huecos de más de {max_gap} min fuera de la pausa diaria")
    if any(b.start.date() != day for b in bars):
        problems.append("barras fuera del día pedido")
    return SampleReport(
        symbol=sym.symbol,
        day=day,
        bars=len(bars),
        hours=hours,
        first=bars[0].start if bars else None,
        last=bars[-1].start if bars else None,
        longest_gap_minutes=longest,
        gaps=big,
        contracts=sorted({b.contract_id for b in bars}),
        problems=problems,
    )
