"""Importador del histórico de 1 minuto de futuros desde Databento (ADR 0011, #247).

```text
uv run python -m cfds_intel.databento --estimate              # gratis: costo por símbolo y total
uv run python -m cfds_intel.databento --sample [AAAA-MM-DD]    # baja un día por símbolo y valida
CFDS_DATA_MACHINE=1 DATABASE_URL=... uv run python -m cfds_intel.databento --run
uv run python -m cfds_intel.databento --check-month AAAA-MM   # baja un mes, cuenta y no guarda
uv run python -m cfds_intel.databento --run --only FDAX.c.0   # cualquier modo, un solo símbolo
```
- La key sale de `DATABENTO_API_KEY` (la lee el SDK oficial) y nunca se imprime.
- `--estimate` solo usa metadata (gratis). Si el total pasa `budget.max_usd`
  (`config/databento.toml`), frena con código 2.
- `--sample` y `--run` vuelven a estimar y frenan igual antes de bajar nada.
- `--run` solo corre en la máquina de datos (`CFDS_DATA_MACHINE=1`): los datos quedan en su base
  local, nunca en el repo. Baja lo que falta antes del primer tramo guardado (si `start` de la
  config es anterior, del mes más nuevo al más viejo) y después del último, mes por mes, así que
  repetirlo no vuelve a pagar lo ya bajado. El tope de gasto es para todos los tramos juntos.
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
    Fetched,
    Stored,
    check_budget,
    chunk_cost,
    chunks,
    estimate,
    fetch_bars,
    months,
    validate_day,
)
from cfds_intel.databento.config import DatabentoConfig, SymbolConfig, load_config

CONFIG = Path(__file__).resolve().parents[4] / "config" / "databento.toml"
KEY_ENV = "DATABENTO_API_KEY"
USAGE = (
    "uso: python -m cfds_intel.databento (--estimate | --sample [AAAA-MM-DD] | --run"
    " | --check-month AAAA-MM) [--only SÍMBOLO]"
)
MODES = {"--estimate", "--sample", "--run", "--check-month"}


def client() -> Client:
    """Cliente oficial. La key la lee el SDK de `DATABENTO_API_KEY`: acá nunca se toca."""
    if not os.environ.get(KEY_ENV):
        raise DatabentoError(f"falta {KEY_ENV} en el .env")
    import databento

    return cast(Client, databento.Historical())


def today_utc() -> date:
    return datetime.now(UTC).date()


PARTS = {"anterior": "antes de lo guardado", "siguiente": "después de lo guardado"}


def print_estimates(estimates: list[Estimate]) -> None:
    for e in estimates:
        s = e.symbol
        if e.skipped:
            print(f"{s.symbol} ({s.dataset} → {s.instrument_id}): no se baja: {e.skipped}")
        else:
            part = f" [{PARTS[e.part]}]" if e.part else ""
            print(
                f"{s.symbol} ({s.dataset} → {s.instrument_id}){part}: {e.start} → {e.end}, "
                f"{e.records} barras, USD {e.cost_usd:.2f}"
                + (f" (el dataset empieza el {e.start})" if e.from_dataset_start else "")
            )


def guarded_estimate(
    c: Client, cfg: DatabentoConfig, end: date, loaded: dict[tuple[str, str], Stored]
) -> list[Estimate]:
    est = estimate(c, cfg, end, loaded)
    print_estimates(est)
    total = check_budget(est, cfg.max_usd)
    print(f"total estimado: USD {total:.2f} (tope USD {cfg.max_usd})")
    return est


def describe_dropped(f: Fetched, sym: SymbolConfig) -> str:
    """Cuántos registros de otros publishers se descartaron (en Eurex, los fuera de libro)."""
    if not f.dropped:
        return f"solo {sym.publisher}: nada descartado"
    detail = ", ".join(f"publisher {p}: {n}" for p, n in sorted(f.dropped.items()))
    return (
        f"se guarda solo {sym.publisher}; descartados {detail}; "
        f"{f.shared_minutes} minutos tenían registro de los dos"
    )


def check_month(c: Client, cfg: DatabentoConfig, month: str) -> int:
    """Baja un mes (con el mismo tope de gasto), cuenta los duplicados y no guarda nada."""
    first = date.fromisoformat(f"{month}-01")
    end = months(first, first + timedelta(days=32))[0][1]
    guarded_estimate(c, replace(cfg, start=first), end, {})
    for sym in cfg.symbols:
        f = fetch_bars(c, cfg, sym, first, end)
        print(f"{sym.symbol} {month}: {len(f.bars)} barras; {describe_dropped(f, sym)}")
    return 0


def last_weekday_before(d: date) -> date:
    d -= timedelta(days=1)
    while d.weekday() >= 5:
        d -= timedelta(days=1)
    return d


def sample(c: Client, cfg: DatabentoConfig, day: date) -> int:
    end = day + timedelta(days=1)
    sample_cfg = replace(cfg, start=day)
    est = guarded_estimate(c, sample_cfg, end, {})
    failed = 0
    for sym in (e.symbol for e in est if not e.skipped):
        f = fetch_bars(c, cfg, sym, day, end)
        r = validate_day(f.bars, sym, day, cfg.max_gap_minutes)
        span = f"{r.first:%H:%M}-{r.last:%H:%M} UTC" if r.first and r.last else "sin barras"
        print(
            f"{sym.symbol} {day}: {r.bars} barras ({r.hours:.1f} h, {span}), pausa más larga "
            f"{r.longest_gap_minutes} min, contratos {r.contracts}"
        )
        print(f"  {describe_dropped(f, sym)}")
        for p in r.problems:
            failed += 1
            print(f"  ⚠️  {p}", file=sys.stderr)
    return 1 if failed else 0


def run(c: Client, cfg: DatabentoConfig) -> int:
    if os.environ.get("CFDS_DATA_MACHINE") != "1":
        print("⚠️  --run solo corre en la máquina de datos (CFDS_DATA_MACHINE=1)", file=sys.stderr)
        return 2
    import psycopg

    from cfds_intel.databento.store import insert_chunk, loaded_spans

    end = today_utc()
    failed: list[str] = []
    with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as conn:
        loaded = loaded_spans(conn)
        for e in guarded_estimate(c, cfg, end, loaded):
            if e.skipped:
                continue
            try:
                for a, b in chunks(e):
                    req_cost = chunk_cost(c, cfg, e.symbol, a, b)
                    f = fetch_bars(c, cfg, e.symbol, a, b)
                    r = insert_chunk(conn, e.symbol, a, b, f.bars, req_cost)
                    print(
                        f"{e.symbol.symbol} {a} → {b}: {r.inserted} nuevas, {r.unchanged} ya "
                        f"estaban (USD {req_cost:.2f}); {describe_dropped(f, e.symbol)}"
                    )
            except (
                Exception
            ) as ex:  # errores del SDK o de los datos: sin la key, que nunca se imprime
                # Un símbolo que falla no corta los demás (#396). Lo ya guardado queda, y la
                # próxima corrida retoma desde ahí.
                failed.append(e.symbol.symbol)
                print(
                    f"⚠️  {e.symbol.symbol}: {type(ex).__name__}: {ex}; sigo con los demás",
                    file=sys.stderr,
                )
    if failed:
        print(f"⚠️  tramos con error: {', '.join(failed)}", file=sys.stderr)
        return 1
    return 0


def main(argv: list[str]) -> int:
    args = argv[1:]
    only = None
    if "--only" in args:
        i = args.index("--only")
        if i + 1 >= len(args):
            print(USAGE, file=sys.stderr)
            return 2
        only = args[i + 1]
        args = args[:i] + args[i + 2 :]
    if not args or args[0] not in MODES or (args[0] == "--check-month" and len(args) < 2):
        print(USAGE, file=sys.stderr)
        return 2
    cfg = load_config(CONFIG.read_text())
    if only is not None:
        symbols = tuple(s for s in cfg.symbols if s.symbol == only)
        if not symbols:
            print(f"⚠️  {only} no está en config/databento.toml", file=sys.stderr)
            return 2
        cfg = replace(cfg, symbols=symbols)
    try:
        c = client()
        if args[0] == "--estimate":
            loaded: dict[tuple[str, str], Stored] = {}
            if os.environ.get("DATABASE_URL"):
                import psycopg

                from cfds_intel.databento.store import loaded_spans

                with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as conn:
                    loaded = loaded_spans(conn)
            guarded_estimate(c, cfg, today_utc(), loaded)
            return 0
        if args[0] == "--sample":
            day = date.fromisoformat(args[1]) if len(args) > 1 else last_weekday_before(today_utc())
            return sample(c, cfg, day)
        if args[0] == "--check-month":
            return check_month(c, cfg, args[1])
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
