"""Carga la curva par diaria del Tesoro de EE. UU. y la guarda (ADR 0012).

```text
DATABASE_URL=... uv run python -m cfds_intel.treasury [desde_año]
```
Sin año, carga desde `[load] start_year` (`config/treasury.toml`) hasta el año en curso. Es
idempotente: lo ya guardado con el mismo valor no se repite. No gasta cupo de IG.
"""

import os
import sys
from datetime import UTC, datetime
from pathlib import Path

import psycopg

from cfds_intel.treasury.store import insert_yields
from cfds_intel.treasury.ust import fetch_rows, load_config, to_yields

CONFIG = Path(__file__).resolve().parents[4] / "config" / "treasury.toml"


def main(argv: list[str]) -> int:
    cfg = load_config(CONFIG.read_text())
    now = datetime.now(UTC)
    first = int(argv[1]) if len(argv) > 1 else cfg.start_year
    if not cfg.start_year <= first <= now.year:
        print(f"el año tiene que estar entre {cfg.start_year} y {now.year}", file=sys.stderr)
        return 2
    rows = fetch_rows(cfg, first, now.year)
    now = datetime.now(UTC)
    yields = to_yields(rows, cfg, now)
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        r = insert_yields(conn, yields, now)
    span = f"{rows[0][0]} → {rows[-1][0]}" if rows else "sin datos"
    print(
        f"Tesoro: {len(rows)} días ({span}), {len(yields)} valores → "
        f"{r.inserted} nuevos, {r.unchanged} ya estaban, {r.corrections} correcciones"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
