"""Análogos v1 de la última ventana de cada índice con datos (#148). Solo informativo.

```text
DATABASE_URL=... uv run python -m cfds_intel.analogs [--fuente ig|futuros] [--guardar] [INSTRUMENTO]
```
Con `--guardar` (#294) también guarda cada informe como `analog_report.v1` en `analog_reports`
(solo agregar; un informe igual al último del mismo día no se repite). Así el gateway y la UI
lo leen sin recalcular nada.
Con `--fuente futuros` (#282) usa las diarias de los futuros (ids derivados, por ejemplo
`US500.FUT`); por omisión, las de IG. Las dos fuentes nunca se mezclan en la búsqueda.
Lee las diarias y los indicadores de Rust (`daily_indicators` y `daily_vol_terciles`); antes hay
que correr
`cargo run -p cfds-ingest --example daily_indicators`. No usa IG.
"""

import os
import sys
from datetime import UTC, datetime
from pathlib import Path

import psycopg

from cfds_intel.analogs.config import AnalogsConfig, load_config
from cfds_intel.analogs.contract import to_contract
from cfds_intel.analogs.data import (
    benchmarks,
    daily_params,
    futures_suffix,
    load_bars,
    load_rates,
    select_source,
)
from cfds_intel.analogs.search import AnalogReport, Bar, Rate, Summary, analyze
from cfds_intel.analogs.store import insert_report

CONFIG = Path(__file__).resolve().parents[4] / "config"
VOL_NAMES = {"low": "baja", "mid": "media", "high": "alta"}


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
    vol = VOL_NAMES[r.context.vol] if r.context.vol else "sin dato"
    ctx = f"volatilidad {vol}, tasas {r.context.rates or 'sin dato'}"
    lines = [f"{r.instrument} — ventana al {r.day} ({ctx})"]
    lines += _fmt_summary("todos", r.all_neighbors)
    lines += _fmt_summary("mismo contexto", r.same_context)
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    args = argv[1:]
    save = "--guardar" in args
    args = [a for a in args if a != "--guardar"]
    futures = False
    if args[:1] == ["--fuente"]:
        if len(args) < 2 or args[1] not in ("ig", "futuros", "futures"):
            print(
                "uso: python -m cfds_intel.analogs [--fuente ig|futuros] [--guardar] [INSTRUMENTO]"
            )
            return 2
        futures = args[1] != "ig"
        args = args[2:]
    suffix = futures_suffix((CONFIG / "candle_sources.toml").read_text()) if futures else None
    print("fuente de las diarias:", "futuros (Databento)" if futures else "IG CFD")
    cfg = load_config((CONFIG / "analogs.toml").read_text())
    atr_period, vol_window, years = daily_params((CONFIG / "indicators.toml").read_text())
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        series = load_bars(conn, atr_period, vol_window, years)
        rates = load_rates(conn, benchmarks((CONFIG / "instruments.toml").read_text()))
        series, rates = select_source(series, rates, suffix)
        return _report(conn, series, rates, args, suffix, cfg, save)


def _report(
    conn: psycopg.Connection,
    series: dict[str, list[Bar]],
    rates: dict[str, list[Rate]],
    args: list[str],
    suffix: str | None,
    cfg: AnalogsConfig,
    save: bool,
) -> int:
    wanted = [a + suffix if suffix and not a.endswith(suffix) else a for a in args]
    wanted = wanted or sorted(series)
    for inst in wanted:
        bars = series.get(inst)
        if not bars:
            print(f"{inst}: sin diarias con indicadores (correr daily_indicators)")
            continue
        report = analyze(series, rates, inst, len(bars) - 1, cfg)
        print(render(report))
        if save:
            doc = to_contract(report, series, "futures" if suffix else "ig", cfg, datetime.now(UTC))
            print("  guardado" if insert_report(conn, doc) else "  ya estaba guardado")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
