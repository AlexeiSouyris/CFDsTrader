"""`config/databento.toml`, validado al cargar (ADR 0011, #247)."""

import re
import tomllib
from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation

_INSTRUMENT = re.compile(r"^[A-Z0-9][A-Z0-9._-]{0,31}$")
_DATASET = re.compile(r"^[A-Z0-9]+\.[A-Z0-9]+$")
_CONTINUOUS = re.compile(r"^[A-Z0-9]{1,10}\.[cnv]\.\d{1,2}$")
_SECTIONS = {"download", "budget", "sample", "symbols"}


class ConfigError(ValueError):
    """`config/databento.toml` no cumple las reglas."""


@dataclass(frozen=True)
class SymbolConfig:
    dataset: str
    symbol: str
    instrument_id: str
    min_session_hours: int


@dataclass(frozen=True)
class DatabentoConfig:
    schema: str
    stype_in: str
    start: date
    min_start: date
    max_usd: Decimal
    max_gap_minutes: int
    symbols: tuple[SymbolConfig, ...]


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


def load_config(text: str) -> DatabentoConfig:
    raw = tomllib.loads(text)
    unknown = set(raw) - _SECTIONS
    if unknown:
        raise ConfigError(f"secciones desconocidas: {sorted(unknown)}")
    d, b, s = raw.get("download", {}), raw.get("budget", {}), raw.get("sample", {})
    if d.get("schema") != "ohlcv-1m":
        raise ConfigError("download.schema tiene que ser ohlcv-1m")
    if d.get("stype_in") != "continuous":
        raise ConfigError("download.stype_in tiene que ser continuous")
    start, min_start = (
        _date(d.get("start"), "download.start"),
        _date(d.get("min_start"), "download.min_start"),
    )
    if start > min_start:
        raise ConfigError("download.start tiene que ser anterior o igual a min_start")
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
        hours = _positive_int(x.get("min_session_hours"), f"{sym}: min_session_hours")
        if hours > 24:
            raise ConfigError(f"{sym}: min_session_hours no puede pasar de 24")
        if set(x) - {"dataset", "symbol", "instrument_id", "min_session_hours"}:
            raise ConfigError(f"{sym}: claves desconocidas")
        symbols.append(SymbolConfig(ds, sym, inst, hours))
    if not symbols:
        raise ConfigError("no hay símbolos configurados")
    if len({(x.dataset, x.symbol) for x in symbols}) != len(symbols):
        raise ConfigError("símbolo repetido")
    if len({x.instrument_id for x in symbols}) != len(symbols):
        raise ConfigError("dos futuros para el mismo instrumento")
    return DatabentoConfig(
        schema=d["schema"],
        stype_in=d["stype_in"],
        start=start,
        min_start=min_start,
        max_usd=max_usd,
        max_gap_minutes=_positive_int(s.get("max_gap_minutes"), "sample.max_gap_minutes"),
        symbols=tuple(symbols),
    )
