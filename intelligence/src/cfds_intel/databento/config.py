"""`config/databento.toml` (1 minuto) y `config/databento_daily.toml` (diarias, #522), validados
al cargar (ADR 0011, #247). Los dos archivos tienen la misma forma; lo que cambia según el
esquema:

- `ohlcv-1m`: lleva `[sample]` y cada símbolo su `min_session_hours` (validación de `--sample`).
- `ohlcv-1d`: no lleva ninguno de los dos (no hay `--sample` de diarias). La vela diaria de
  Databento cubre un día UTC (medianoche a medianoche UTC), no la sesión de la bolsa.
"""

import re
import tomllib
from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation

_INSTRUMENT = re.compile(r"^[A-Z0-9][A-Z0-9._-]{0,31}$")
_DATASET = re.compile(r"^[A-Z0-9]+\.[A-Z0-9]+$")
_CONTINUOUS = re.compile(r"^[A-Z0-9]{1,10}\.[cnv]\.\d{1,2}$")
_SECTIONS = {"download", "budget", "sample", "symbols"}

# Esquemas OHLCV oficiales que se bajan (docs: schemas-and-data-formats/ohlcv).
MINUTE = "ohlcv-1m"
DAILY = "ohlcv-1d"
SCHEMAS = (MINUTE, DAILY)


class ConfigError(ValueError):
    """`config/databento.toml` no cumple las reglas."""


@dataclass(frozen=True)
class SymbolConfig:
    dataset: str
    symbol: str
    instrument_id: str
    # Solo en `ohlcv-1m` (validación de `--sample`); `None` en las diarias.
    min_session_hours: int | None
    # Publisher del libro central (el único que se guarda) y su id numérico.
    publisher: str
    publisher_id: int
    # Inicio propio del símbolo (#410), cuando el futuro empezó a cotizar en el dataset después
    # del `start` global (RTY en CME desde 2017). Solo puede ser posterior al global, así que el
    # inicio efectivo es siempre el mayor de los dos (`effective_start`).
    start: date | None = None


@dataclass(frozen=True)
class DatabentoConfig:
    schema: str
    stype_in: str
    start: date
    max_usd: Decimal
    # Solo en `ohlcv-1m` (`[sample]`); `None` en las diarias.
    max_gap_minutes: int | None
    symbols: tuple[SymbolConfig, ...]


def effective_start(cfg: DatabentoConfig, sym: SymbolConfig) -> date:
    """Desde cuándo se baja `sym`: su `start` propio gana sobre el global."""
    return max(cfg.start, sym.start) if sym.start else cfg.start


def _positive_int(v: object, name: str) -> int:
    if not isinstance(v, int) or isinstance(v, bool) or v <= 0:
        raise ConfigError(f"{name} tiene que ser un entero mayor que 0")
    return v


def _date(v: object, name: str) -> date:
    if not isinstance(v, str):
        raise ConfigError(f"{name} tiene que ser AAAA-MM-DD")
    try:
        return date.fromisoformat(v)
    except ValueError as e:
        raise ConfigError(f"{name}: {v!r} no es AAAA-MM-DD") from e


def _publisher_id(name: object, dataset: str, sym: str) -> int:
    """Id numérico del publisher según la lista del SDK oficial (`databento.Publisher`)."""
    import databento

    if not isinstance(name, str):
        raise ConfigError(f"{sym}: falta publisher")
    try:
        p = databento.Publisher(name)
    except ValueError as e:
        raise ConfigError(f"{sym}: publisher desconocido {name!r}") from e
    if str(p.dataset) != dataset:
        raise ConfigError(f"{sym}: el publisher {name} no es del dataset {dataset}")
    return int(p.to_int())


def load_config(text: str) -> DatabentoConfig:
    raw = tomllib.loads(text)
    unknown = set(raw) - _SECTIONS
    if unknown:
        raise ConfigError(f"secciones desconocidas: {sorted(unknown)}")
    d, b, s = raw.get("download", {}), raw.get("budget", {}), raw.get("sample", {})
    schema = d.get("schema")
    if schema not in SCHEMAS:
        raise ConfigError(f"download.schema tiene que ser uno de {list(SCHEMAS)}")
    minute = schema == MINUTE
    if not minute and "sample" in raw:
        raise ConfigError(f"[sample] es solo para {MINUTE}")
    if d.get("stype_in") != "continuous":
        raise ConfigError("download.stype_in tiene que ser continuous")
    start = _date(d.get("start"), "download.start")
    extra = set(d) - {"schema", "stype_in", "start"}
    if extra:
        raise ConfigError(f"download: claves desconocidas {sorted(extra)}")
    try:
        max_usd = Decimal(str(b.get("max_usd")))
    except InvalidOperation as e:
        raise ConfigError("budget.max_usd tiene que ser un decimal como texto") from e
    if not isinstance(b.get("max_usd"), str) or not max_usd.is_finite() or max_usd <= 0:
        raise ConfigError("budget.max_usd tiene que ser un decimal como texto, mayor que 0")
    symbols: list[SymbolConfig] = []
    for x in raw.get("symbols", []):
        ds, sym, inst = x.get("dataset"), x.get("symbol"), x.get("instrument_id")
        if not (isinstance(ds, str) and _DATASET.match(ds)):
            raise ConfigError(f"symbols.dataset inválido: {ds!r}")
        if not (isinstance(sym, str) and _CONTINUOUS.match(sym)):
            raise ConfigError(f"symbols.symbol no es un contrato continuo: {sym!r}")
        if not (isinstance(inst, str) and _INSTRUMENT.match(inst)):
            raise ConfigError(f"symbols.instrument_id inválido: {inst!r}")
        hours: int | None = None
        if minute:
            hours = _positive_int(x.get("min_session_hours"), f"{sym}: min_session_hours")
            if hours > 24:
                raise ConfigError(f"{sym}: min_session_hours no puede pasar de 24")
        allowed = {"dataset", "symbol", "instrument_id", "publisher", "start"}
        if minute:
            allowed.add("min_session_hours")
        if set(x) - allowed:
            raise ConfigError(f"{sym}: claves desconocidas")
        own_start = None
        if "start" in x:
            own_start = _date(x["start"], f"{sym}: start")
            if own_start <= start:
                raise ConfigError(
                    f"{sym}: start ({own_start}) tiene que ser posterior a download.start ({start})"
                )
        pub = x.get("publisher")
        pub_id = _publisher_id(pub, ds, sym)
        symbols.append(SymbolConfig(ds, sym, inst, hours, str(pub), pub_id, own_start))
    if not symbols:
        raise ConfigError("no hay símbolos configurados")
    if len({(x.dataset, x.symbol) for x in symbols}) != len(symbols):
        raise ConfigError("símbolo repetido")
    if len({x.instrument_id for x in symbols}) != len(symbols):
        raise ConfigError("dos futuros para el mismo instrumento")
    return DatabentoConfig(
        schema=str(schema),
        stype_in=d["stype_in"],
        start=start,
        max_usd=max_usd,
        max_gap_minutes=(
            _positive_int(s.get("max_gap_minutes"), "sample.max_gap_minutes") if minute else None
        ),
        symbols=tuple(symbols),
    )
