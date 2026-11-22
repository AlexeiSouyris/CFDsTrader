"""Lectura de los datos de los análogos desde PostgreSQL (#148).

- Velas diarias **con sus indicadores de Rust** (`daily_indicators`): el cruce deja solo los días
  con sesión, porque Rust solo calcula esos.
- El tipo de referencia de cada instrumento sale de `config/instruments.toml`
  (`funding_benchmark`), y sus valores, de `reference_rates`.
"""

import tomllib
from collections.abc import Mapping

import psycopg

from cfds_intel.analogs.search import Bar, Rate


def daily_params(indicators_toml: str) -> tuple[int, int]:
    """(atr_period, realized_vol_window) de `config/indicators.toml` [daily]."""
    daily = tomllib.loads(indicators_toml).get("daily", {})
    atr, vol = daily.get("atr_period"), daily.get("realized_vol_window")
    if not (isinstance(atr, int) and isinstance(vol, int)):
        raise ValueError("config/indicators.toml: falta [daily] atr_period o realized_vol_window")
    return atr, vol


def benchmarks(instruments_toml: str) -> dict[str, str]:
    """Instrumento → nombre de su tipo de referencia. Los pendientes no aparecen."""
    out: dict[str, str] = {}
    for inst, spec in tomllib.loads(instruments_toml).get("instruments", {}).items():
        fb = spec.get("funding_benchmark", {})
        if isinstance(fb, dict) and isinstance(fb.get("value"), str):
            out[inst] = fb["value"]
    return out


def load_bars(conn: psycopg.Connection, atr_period: int, vol_window: int) -> dict[str, list[Bar]]:
    rows = conn.execute(
        """
        SELECT d.instrument_id, d.day, i.published_at, d.high, d.low, d.close,
               i.atr, i.realized_vol
        FROM daily_candles d
        JOIN daily_indicators i ON i.instrument_id = d.instrument_id AND i.day = d.day
        WHERE i.atr_period = %s AND i.realized_vol_window = %s
        ORDER BY d.instrument_id, d.day
        """,
        (atr_period, vol_window),
    ).fetchall()
    series: dict[str, list[Bar]] = {}
    for inst, day, end, high, low, close, atr, vol in rows:
        series.setdefault(inst, []).append(Bar(day, end, high, low, close, atr, vol))
    return series


def load_rates(conn: psycopg.Connection, by_instrument: Mapping[str, str]) -> dict[str, list[Rate]]:
    out: dict[str, list[Rate]] = {}
    for inst, series in by_instrument.items():
        rows = conn.execute(
            "SELECT ref_date, published_at, rate_pct FROM reference_rates "
            "WHERE series = %s ORDER BY published_at, ref_date",
            (series,),
        ).fetchall()
        if rows:
            out[inst] = [Rate(d, p, v) for d, p, v in rows]
    return out
