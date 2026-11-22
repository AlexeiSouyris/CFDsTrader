"""Carga los tipos de referencia diarios del BCE y los guarda (#79).

```text
DATABASE_URL=... uv run python -m cfds_intel.rates [desde AAAA-MM-DD]
```
Sin fecha, carga cada serie desde su `start` (`config/reference_rates.toml`). No gasta cupo de IG.
"""

import os
import sys
from datetime import UTC, date, datetime
from pathlib import Path

import psycopg

from cfds_intel.rates.ecb import fetch_csv, load_config, parse_csv, to_rates
from cfds_intel.rates.store import insert_rates

CONFIG = Path(__file__).resolve().parents[4] / "config" / "reference_rates.toml"


def main(argv: list[str]) -> int:
    cfg = load_config(CONFIG.read_text())
    since_arg = date.fromisoformat(argv[1]) if len(argv) > 1 else None
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        for s in cfg.series.values():
            since = max(since_arg, s.start) if since_arg else s.start
            values = parse_csv(fetch_csv(cfg, s, since), s)
            now = datetime.now(UTC)
            r = insert_rates(conn, to_rates(values, s, now), now)
            span = f"{values[0][0]} → {values[-1][0]}" if values else "sin datos"
            print(
                f"{s.name}: {len(values)} días ({span}) → "
                f"{r.inserted} nuevos, {r.unchanged} ya estaban, {r.corrections} correcciones"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
