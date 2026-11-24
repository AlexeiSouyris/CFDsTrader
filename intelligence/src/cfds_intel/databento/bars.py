"""Barras de 1 minuto de Databento: estimación del costo, conversión y validación (ADR 0011, #247).

Reglas:
- Solo el SDK oficial (`databento`) y sus métodos documentados. La metadata (`get_cost`,
  `get_record_count`, `get_dataset_range`) no se cobra: "Access to metadata, symbology, and
  account management is free" (docs: api-reference-historical/basics).
- `ts_event` es el **inicio** de la barra, en nanosegundos UTC. Tiene que caer justo en un minuto.
- Los precios llegan como enteros con escala 1e-9: se pasan a `Decimal` exacto, nunca a float.
- Un minuto sin operaciones no trae barra (docs: schemas-and-data-formats/ohlcv). Por eso un
  hueco no es un error de la fuente, pero `--sample` lo informa.
- Solo se guardan las barras del publisher del libro central (`publisher` en la config). En
  Eurex, un minuto con operaciones dentro y fuera del libro trae dos registros, uno por publisher
  (`XEUR.EOBI.XEUR` y `XEUR.EOBI.XOFF`, docs: datasets/xeur-eobi): el de fuera del libro se
  descarta y se cuenta. Dos registros del mismo minuto y del mismo publisher son un error.
- Cualquier barra que no cumple las reglas hace fallar el pedido entero: no se guarda nada.
"""

from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from itertools import pairwise
from typing import Any, Protocol

from cfds_intel.databento.config import DatabentoConfig, SymbolConfig, effective_start

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


@dataclass(frozen=True)
class Fetched:
    """Barras del libro central y lo descartado de otros publishers."""

    bars: list[Bar]
    # Registros descartados por publisher_id (por ejemplo 103 = XEUR.EOBI.XOFF).
    dropped: dict[int, int]
    # Minutos en los que hubo un registro del libro central y otro de otro publisher.
    shared_minutes: int


def to_bars(records: Iterable[Any], sym: SymbolConfig, start: datetime, end: datetime) -> Fetched:
    """Barras de `[start, end)` del publisher `sym.publisher_id`, en orden estricto y sin
    repetir. Los registros de otros publishers se descartan y se cuentan."""
    kept: list[Any] = []
    dropped: dict[int, int] = {}
    other_minutes: set[int] = set()
    for r in records:
        pub = getattr(r, "publisher_id", None)
        if not isinstance(pub, int):
            raise DatabentoError(f"{sym.symbol}: registro sin publisher_id")
        if pub == sym.publisher_id:
            kept.append(r)
        else:
            dropped[pub] = dropped.get(pub, 0) + 1
            other_minutes.add(getattr(r, "ts_event", -1))
    bars = [to_bar(r, sym) for r in kept]
    for prev, cur in pairwise(bars):
        if cur.start == prev.start:
            raise DatabentoError(
                f"{sym.symbol}: dos barras del mismo publisher en {cur.start}: no se elige a ojo"
            )
        if cur.start < prev.start:
            raise DatabentoError(f"{sym.symbol}: barras fuera de orden en {cur.start}")
    if bars and (bars[0].start < start or bars[-1].start >= end):
        raise DatabentoError(f"{sym.symbol}: barras fuera del período pedido")
    shared = len(other_minutes & {r.ts_event for r in kept})
    return Fetched(bars, dropped, shared)


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
class Stored:
    """Lo ya guardado de un símbolo según `futures_bar_loads`: desde `first` (el primer día del
    tramo más viejo) hasta `until` (exclusivo, el fin del tramo más nuevo)."""

    first: date
    until: date


BACKWARD = "anterior"
FORWARD = "siguiente"


@dataclass(frozen=True)
class Estimate:
    symbol: SymbolConfig
    start: date
    end: date
    cost_usd: Decimal
    records: int
    skipped: str | None = None
    # `BACKWARD`: el tramo que falta antes de lo guardado; `FORWARD`: el que falta después.
    # `None`: no hay nada guardado del símbolo.
    part: str | None = None
    # El tramo arranca en el inicio del dataset porque `start` de la config es anterior.
    from_dataset_start: bool = False


def _range_day(dataset: str, r: Mapping[str, Any], schema: str, key: str) -> date:
    per_schema = r.get("schema")
    raw = r.get(key)
    if isinstance(per_schema, Mapping) and isinstance(per_schema.get(schema), Mapping):
        raw = per_schema[schema].get(key, raw)
    if not isinstance(raw, str):
        if isinstance(per_schema, Mapping) and schema not in per_schema:
            raise DatabentoError(f"{dataset} no tiene el esquema {schema}")
        raise DatabentoError(f"{dataset}: get_dataset_range sin {key}")
    try:
        return date.fromisoformat(raw[:10])
    except ValueError as e:
        raise DatabentoError(f"{dataset}: {key} inválido {raw!r}") from e


def dataset_range(client: Client, dataset: str, schema: str) -> tuple[date, date]:
    """`[primer día, último día completo + 1)` con datos del esquema en el dataset
    (`get_dataset_range`, gratis). El fin que da Databento es un instante (por ejemplo
    "2026-11-23T18:10:06Z"): se corta a la medianoche UTC anterior, así nunca se pide más allá de
    lo publicado (422 `dataset_unavailable_range`, #396) ni se guarda un día a medias."""
    r = client.metadata.get_dataset_range(dataset)
    return _range_day(dataset, r, schema, "start"), _range_day(dataset, r, schema, "end")


def chunk_cost(
    client: Client, cfg: DatabentoConfig, sym: SymbolConfig, start: date, end: date
) -> Decimal:
    """Costo en USD de `[start, end)` según `metadata.get_cost` (gratis)."""
    return Decimal(repr(float(client.metadata.get_cost(**_request(cfg, sym, start, end)))))


def _missing(
    wanted: date, available: date, end: date, stored: Stored | None
) -> list[tuple[str | None, date, date]]:
    """Tramos que faltan de un símbolo: `[desde, hasta)` con su parte. Lo que hay entre el primer
    y el último tramo guardado no se vuelve a pedir: los tramos se guardan sin huecos."""
    first = max(wanted, available)
    if stored is None:
        return [(None, first, end)]
    return [
        (BACKWARD, first, min(stored.first, end)),
        (FORWARD, max(first, stored.until), end),
    ]


def estimate(
    client: Client,
    cfg: DatabentoConfig,
    end: date,
    stored: Mapping[tuple[str, str], Stored],
) -> list[Estimate]:
    """Costo y cantidad de barras de lo que **falta** bajar de cada símbolo, hasta `end`: el
    tramo anterior a lo ya guardado (si `start` es anterior) y el posterior. Un símbolo puede
    tener entonces dos tramos; el tope de gasto se aplica al total de todos. El fin de cada tramo
    es el menor entre `end` y el fin publicado del dataset (#396)."""
    ranges: dict[str, tuple[date, date]] = {}
    out: list[Estimate] = []
    for sym in cfg.symbols:
        if sym.dataset not in ranges:
            ranges[sym.dataset] = dataset_range(client, sym.dataset, cfg.schema)
        # Si el dataset empieza después de `start` (Eurex), se baja desde su inicio.
        available, published = ranges[sym.dataset]
        # El inicio propio del símbolo gana sobre el global (#410).
        wanted = effective_start(cfg, sym)
        until = min(end, published)
        parts = [
            (part, a, b)
            for part, a, b in _missing(
                wanted, available, until, stored.get((sym.dataset, sym.symbol))
            )
            if a < b
        ]
        if not parts:
            key = (sym.dataset, sym.symbol)
            why = "ya está todo guardado" if key in stored else "no hay datos en el período pedido"
            out.append(Estimate(sym, until, until, Decimal(0), 0, why))
            continue
        for part, a, b in parts:
            cost = chunk_cost(client, cfg, sym, a, b)
            records = int(client.metadata.get_record_count(**_request(cfg, sym, a, b)))
            if cost < 0 or records < 0:
                raise DatabentoError(f"{sym.symbol}: estimación negativa")
            from_dataset = a == available and available > wanted
            out.append(Estimate(sym, a, b, cost, records, None, part, from_dataset))
    return out


def chunks(e: Estimate) -> list[tuple[date, date]]:
    """Los meses de un tramo en el orden de bajada. El tramo anterior va del mes más nuevo al
    más viejo: si la corrida se corta, lo guardado sigue sin huecos y la próxima retoma desde
    el primer día guardado."""
    out = months(e.start, e.end)
    return out[::-1] if e.part == BACKWARD else out


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
) -> Fetched:
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
