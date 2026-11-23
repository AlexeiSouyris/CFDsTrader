"""Lectura de los datos de los análogos desde PostgreSQL (#148).

- Velas diarias **con sus indicadores de Rust** (`daily_indicators`): el cruce deja solo los días
  con sesión, porque Rust solo calcula esos. El tercio de la volatilidad también es de Rust
  (`daily_vol_terciles`); sin fila, queda sin dato.
- El tipo de referencia de cada instrumento sale de `config/instruments.toml`
  (`funding_benchmark`), y sus valores, de `reference_rates`.
- Fuente de las diarias (#282): las de IG (ids del catálogo) o las de los futuros (ids derivados,
  por ejemplo `US500.FUT`). Nunca se mezclan: serían el mismo mercado dos veces y cada ventana
  tendría de vecino a su gemela.
"""

import tomllib
from collections.abc import Mapping

import psycopg

from cfds_intel.analogs.search import Bar, Rate


def daily_params(indicators_toml: str) -> tuple[int, int, int]:
    """(atr_period, realized_vol_window, vol_tercile_years) de `config/indicators.toml` [daily]."""
    daily = tomllib.loads(indicators_toml).get("daily", {})
    keys = ("atr_period", "realized_vol_window", "vol_tercile_years")
    atr, vol, years = (daily.get(k) for k in keys)
    if not (isinstance(atr, int) and isinstance(vol, int) and isinstance(years, int)):
        raise ValueError(f"config/indicators.toml: falta alguno de [daily] {', '.join(keys)}")
    return atr, vol, years


def futures_suffix(candle_sources_toml: str) -> str:
    """`[futures] id_suffix` de `config/candle_sources.toml` (por ejemplo `.FUT`)."""
    suffix = tomllib.loads(candle_sources_toml).get("futures", {}).get("id_suffix")
    if not isinstance(suffix, str) or not suffix:
        raise ValueError("config/candle_sources.toml: falta [futures] id_suffix")
    return suffix


def select_source(
    series: Mapping[str, list[Bar]],
    rates: Mapping[str, list[Rate]],
    suffix: str | None,
) -> tuple[dict[str, list[Bar]], dict[str, list[Rate]]]:
    """Solo las series de una fuente: las de IG (`suffix` None, ids sin sufijo) o las de los
    futuros (ids que terminan en `suffix`). Los tipos de referencia son los del instrumento base."""
    if suffix is None:
        return {k: v for k, v in series.items() if "." not in k}, dict(rates)
    chosen = {k: v for k, v in series.items() if k.endswith(suffix)}
    by_id = {k: rates[k.removesuffix(suffix)] for k in chosen if k.removesuffix(suffix) in rates}
    return chosen, by_id


def benchmarks(instruments_toml: str) -> dict[str, str]:
    """Instrumento → nombre de su tipo de referencia. Los pendientes no aparecen."""
    out: dict[str, str] = {}
    for inst, spec in tomllib.loads(instruments_toml).get("instruments", {}).items():
        fb = spec.get("funding_benchmark", {})
        if isinstance(fb, dict) and isinstance(fb.get("value"), str):
            out[inst] = fb["value"]
    return out


def load_bars(
    conn: psycopg.Connection, atr_period: int, vol_window: int, tercile_years: int
) -> dict[str, list[Bar]]:
    rows = conn.execute(
        """
        SELECT d.instrument_id, d.day, i.published_at, d.high, d.low, d.close,
               i.atr, t.tercile
        FROM daily_candles d
        JOIN daily_indicators i ON i.instrument_id = d.instrument_id AND i.day = d.day
        LEFT JOIN daily_vol_terciles t
          ON t.instrument_id = d.instrument_id AND t.day = d.day
         AND t.realized_vol_window = i.realized_vol_window AND t.history_years = %s
        WHERE i.atr_period = %s AND i.realized_vol_window = %s
        ORDER BY d.instrument_id, d.day
        """,
        (tercile_years, atr_period, vol_window),
    ).fetchall()
    series: dict[str, list[Bar]] = {}
    for inst, day, end, high, low, close, atr, tercile in rows:
        if tercile not in (None, "low", "mid", "high"):
            raise ValueError(f"{inst} {day}: tercio fuera del contrato: {tercile!r}")
        series.setdefault(inst, []).append(Bar(day, end, high, low, close, atr, tercile))
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
