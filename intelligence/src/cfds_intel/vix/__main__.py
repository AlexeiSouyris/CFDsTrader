"""Carga el VIX diario del CSV oficial de Cboe y lo guarda (ADR 0012 §2).

```text
DATABASE_URL=... uv run python -m cfds_intel.vix
```
Una sola lectura del CSV; guarda desde `[load] start` (`config/vix.toml`). Es idempotente. Si Cboe
responde 403 o algo que no es el CSV, sale con código 2 sin guardar nada y sin reintentar.
Los datos quedan solo en la base: nunca se copian al repo.
"""

import os
import sys
from datetime import UTC, datetime
from pathlib import Path

import psycopg

from cfds_intel.vix.cboe import VixError, fetch_csv, load_config, parse_csv, to_days
from cfds_intel.vix.store import insert_days

CONFIG = Path(__file__).resolve().parents[4] / "config" / "vix.toml"


def main() -> int:
    cfg = load_config(CONFIG.read_text())
    try:
        rows = parse_csv(fetch_csv(cfg), cfg.start)
    except VixError as e:
        print(f"VIX: {e}", file=sys.stderr)
        return 2
    now = datetime.now(UTC)
    days = to_days(rows, cfg, now)
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        r = insert_days(conn, days, now)
    span = f"{days[0].ref_date} → {days[-1].ref_date}" if days else "sin datos"
    print(
        f"VIX: {len(days)} días ({span}) → "
        f"{r.inserted} nuevos, {r.unchanged} ya estaban, {r.corrections} correcciones"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
