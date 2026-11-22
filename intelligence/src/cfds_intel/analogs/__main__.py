"""Análogos v1 de la última ventana de cada índice con datos (#148). Solo informativo.

```text
DATABASE_URL=... uv run python -m cfds_intel.analogs [INSTRUMENTO]
```
Lee las diarias y los indicadores de Rust (`daily_indicators`); antes hay que correr
`cargo run -p cfds-ingest --example daily_indicators`. No usa IG.
"""

import os
import sys
from pathlib import Path

import psycopg

from cfds_intel.analogs.config import load_config
from cfds_intel.analogs.data import benchmarks, daily_params, load_bars, load_rates
from cfds_intel.analogs.search import AnalogReport, Summary, analyze

CONFIG = Path(__file__).resolve().parents[4] / "config"


def _fmt_summary(title: str, s: Summary) -> list[str]:
    if s.count == 0:
        return [f"  {title}: sin vecinos"]
    head = f"  {title}: {s.count} vecinos" + (" — DATOS INSUFICIENTES" if s.insufficient else "")
    lines = [head]
    for h in s.horizons:
        pcts = " / ".join(f"p{p} {v * 100:+.2f} %" for p, v in h.percentiles.items())
        lines.append(f"    {h.horizon:>2} sesiones: {pcts} · positivos {h.positive_pct:.0f} %")
    with_r = sum(s.hits.values())
    if with_r:
        up = s.hits["up"] * 100 / with_r
        lines.append(
            f"    +2R antes que -1R: {up:.0f} % ({s.hits['up']} de {with_r};"
            f" ninguno {s.hits['none']}; sin ATR {s.without_atr})"
        )
    return lines


def render(r: AnalogReport) -> str:
    ctx = f"volatilidad {r.context.vol or 'sin dato'}, tasas {r.context.rates or 'sin dato'}"
    lines = [f"{r.instrument} — ventana al {r.day} ({ctx})"]
    lines += _fmt_summary("todos", r.all_neighbors)
    lines += _fmt_summary("mismo contexto", r.same_context)
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    cfg = load_config((CONFIG / "analogs.toml").read_text())
    atr_period, vol_window = daily_params((CONFIG / "indicators.toml").read_text())
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        series = load_bars(conn, atr_period, vol_window)
        rates = load_rates(conn, benchmarks((CONFIG / "instruments.toml").read_text()))
    wanted = argv[1:] or sorted(series)
    for inst in wanted:
        bars = series.get(inst)
        if not bars:
            print(f"{inst}: sin diarias con indicadores (correr daily_indicators)")
            continue
        print(render(analyze(series, rates, inst, len(bars) - 1, cfg)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
