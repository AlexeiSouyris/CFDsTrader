"""Importador del histórico de 1 minuto de futuros desde Databento (ADR 0011, #247).

```text
uv run python -m cfds_intel.databento --estimate              # gratis: costo por símbolo y total
uv run python -m cfds_intel.databento --sample [AAAA-MM-DD]    # baja un día por símbolo y valida
CFDS_DATA_MACHINE=1 DATABASE_URL=... uv run python -m cfds_intel.databento --run
```
- La key sale de `DATABENTO_API_KEY` (la lee el SDK oficial) y nunca se imprime.
- `--estimate` solo usa metadata (gratis). Si el total pasa `budget.max_usd`
  (`config/databento.toml`), frena con código 2.
- `--sample` y `--run` vuelven a estimar y frenan igual antes de bajar nada.
- `--run` solo corre en la máquina de datos (`CFDS_DATA_MACHINE=1`): los datos quedan en su base
  local, nunca en el repo. Sigue desde el último tramo guardado, mes por mes, así que repetirlo
  no vuelve a pagar lo ya bajado.
"""

import os
import sys
from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from pathlib import Path
from typing import cast

from cfds_intel.databento.bars import (
    BudgetError,
    Client,
    DatabentoError,
    Estimate,
    check_budget,
    chunk_cost,
    estimate,
    fetch_bars,
    months,
    validate_day,
)
from cfds_intel.databento.config import DatabentoConfig, load_config

CONFIG = Path(__file__).resolve().parents[4] / "config" / "databento.toml"
KEY_ENV = "DATABENTO_API_KEY"
USAGE = "uso: python -m cfds_intel.databento (--estimate | --sample [AAAA-MM-DD] | --run)"


def client() -> Client:
    """Cliente oficial. La key la lee el SDK de `DATABENTO_API_KEY`: acá nunca se toca."""
    if not os.environ.get(KEY_ENV):
        raise DatabentoError(f"falta {KEY_ENV} en el .env")
    import databento

    return cast(Client, databento.Historical())


def today_utc() -> date:
    return datetime.now(UTC).date()


def print_estimates(estimates: list[Estimate]) -> None:
    for e in estimates:
        s = e.symbol
        if e.skipped:
            print(f"{s.symbol} ({s.dataset} → {s.instrument_id}): no se baja: {e.skipped}")
        else:
            print(
                f"{s.symbol} ({s.dataset} → {s.instrument_id}): {e.start} → {e.end}, "
                f"{e.records} barras, USD {e.cost_usd:.2f}"
            )


def guarded_estimate(
    c: Client, cfg: DatabentoConfig, end: date, loaded: dict[tuple[str, str], date]
) -> list[Estimate]:
    est = estimate(c, cfg, end, loaded)
    print_estimates(est)
    total = check_budget(est, cfg.max_usd)
    print(f"total estimado: USD {total:.2f} (tope USD {cfg.max_usd})")
    return est


def last_weekday_before(d: date) -> date:
    d -= timedelta(days=1)
    while d.weekday() >= 5:
        d -= timedelta(days=1)
    return d


def sample(c: Client, cfg: DatabentoConfig, day: date) -> int:
    end = day + timedelta(days=1)
    sample_cfg = replace(cfg, start=day, min_start=max(day, cfg.min_start))
    est = guarded_estimate(c, sample_cfg, end, {})
    failed = 0
    for sym in (e.symbol for e in est if not e.skipped):
        bars = fetch_bars(c, cfg, sym, day, end)
        r = validate_day(bars, sym, day, cfg.max_gap_minutes)
        span = f"{r.first:%H:%M}-{r.last:%H:%M} UTC" if r.first and r.last else "sin barras"
        print(
            f"{sym.symbol} {day}: {r.bars} barras ({r.hours:.1f} h, {span}), pausa más larga "
            f"{r.longest_gap_minutes} min, contratos {r.contracts}"
        )
        for p in r.problems:
            failed += 1
            print(f"  ⚠️  {p}", file=sys.stderr)
    return 1 if failed else 0


def run(c: Client, cfg: DatabentoConfig) -> int:
    if os.environ.get("CFDS_DATA_MACHINE") != "1":
        print("⚠️  --run solo corre en la máquina de datos (CFDS_DATA_MACHINE=1)", file=sys.stderr)
        return 2
    import psycopg

    from cfds_intel.databento.store import insert_chunk, loaded_until

    end = today_utc()
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        loaded = loaded_until(conn)
        for e in guarded_estimate(c, cfg, end, loaded):
            if e.skipped:
                continue
            for a, b in months(e.start, e.end):
                req_cost = chunk_cost(c, cfg, e.symbol, a, b)
                bars = fetch_bars(c, cfg, e.symbol, a, b)
                r = insert_chunk(conn, e.symbol, a, b, bars, req_cost)
                print(
                    f"{e.symbol.symbol} {a} → {b}: {r.inserted} nuevas, {r.unchanged} ya estaban "
                    f"(USD {req_cost:.2f})"
                )
    return 0


def main(argv: list[str]) -> int:
    args = argv[1:]
    if not args or args[0] not in {"--estimate", "--sample", "--run"}:
        print(USAGE, file=sys.stderr)
        return 2
    cfg = load_config(CONFIG.read_text())
    try:
        c = client()
        if args[0] == "--estimate":
            loaded: dict[tuple[str, str], date] = {}
            if os.environ.get("DATABASE_URL"):
                import psycopg

                from cfds_intel.databento.store import loaded_until

                with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
                    loaded = loaded_until(conn)
            guarded_estimate(c, cfg, today_utc(), loaded)
            return 0
        if args[0] == "--sample":
            day = date.fromisoformat(args[1]) if len(args) > 1 else last_weekday_before(today_utc())
            return sample(c, cfg, day)
        return run(c, cfg)
    except BudgetError as e:
        print(f"⛔ {e}: no se baja nada", file=sys.stderr)
        return 2
    except DatabentoError as e:
        print(f"⚠️  {e}", file=sys.stderr)
        return 1
    except Exception as e:  # errores del SDK (401, 422, 429...): sin la key, que nunca se imprime
        print(f"⚠️  {type(e).__name__}: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
