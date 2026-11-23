"""Calcula y guarda el índice de sentimiento v1 (spec §9.1, #213) de US500, US100 y US30.

```text
DATABASE_URL=... uv run python -m cfds_intel.sentiment.compute_index
```
Lee los datos crudos de PostgreSQL (COT, volatilidad realizada y caída desde el máximo), arma el
índice cada vez que un componente publicó un dato y guarda lo nuevo en `sentiment_index`. Es
idempotente: lo ya guardado e idéntico cuenta como "ya estaban". Si un valor ya guardado diera
distinto, frena con error (sería repintado). Si no se llega al mínimo de componentes, lo dice.
No usa IG ni la red.
"""

import os
import sys
import tomllib
from pathlib import Path

import psycopg

from cfds_intel.sentiment.index import index_series, load_config
from cfds_intel.sentiment.index_store import insert_index, load_points

CONFIG = Path(__file__).resolve().parents[4] / "config"


def daily_windows(indicators_toml: str) -> tuple[int, int, int]:
    """(realized_vol_window, atr_period, drawdown_window) de `config/indicators.toml` [daily]."""
    daily = tomllib.loads(indicators_toml).get("daily", {})
    try:
        return (
            int(daily["realized_vol_window"]),
            int(daily["atr_period"]),
            int(daily["drawdown_window"]),
        )
    except (KeyError, TypeError, ValueError) as e:
        raise ValueError(f"config/indicators.toml [daily] incompleto: {e!r}") from e


def main() -> int:
    cfg = load_config((CONFIG / "sentiment.toml").read_text())
    vol_window, atr_period, dd_window = daily_windows((CONFIG / "indicators.toml").read_text())
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        for instrument in cfg.instruments:
            data = load_points(conn, instrument, vol_window, atr_period, dd_window)
            counts = ", ".join(f"{k} {len(v)}" for k, v in data.items())
            emitted, skipped = index_series(instrument, data, cfg)
            r = insert_index(conn, emitted)
            print(
                f"{instrument}: datos ({counts}) → {len(emitted)} valores del índice: "
                f"{r.inserted} nuevos, {r.unchanged} ya estaban"
            )
            if emitted:
                last = emitted[-1]
                print(f"  último: {last.value:.4f} ({last.published_at:%Y-%m-%d %H:%M} UTC)")
            if skipped and (not emitted or skipped[-1].at > emitted[-1].published_at):
                last_skip = skipped[-1]
                print(f"  sin índice desde {last_skip.at:%Y-%m-%d %H:%M} UTC: {last_skip.reason()}")
            elif not emitted and not skipped:
                print("  sin índice: no hay ningún dato")
    return 0


if __name__ == "__main__":
    sys.exit(main())
