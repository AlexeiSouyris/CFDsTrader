"""Carga los tipos de referencia diarios del BCE (#79) y de la NY Fed (#467) y los guarda.

```text
DATABASE_URL=... uv run python -m cfds_intel.rates [desde AAAA-MM-DD]
```
Sin fecha, carga cada serie desde su `start` (`config/reference_rates.toml`). No gasta cupo de IG.
"""

import os
import sys
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path

import psycopg

from cfds_intel.rates import nyfed
from cfds_intel.rates.ecb import Rate, fetch_csv, load_config, parse_csv, to_rates
from cfds_intel.rates.store import StoreReport, insert_rates

CONFIG = Path(__file__).resolve().parents[4] / "config" / "reference_rates.toml"


def _report(name: str, values: list[tuple[date, Decimal]], r: StoreReport) -> None:
    span = f"{values[0][0]} → {values[-1][0]}" if values else "sin datos"
    print(
        f"{name}: {len(values)} días ({span}) → "
        f"{r.inserted} nuevos, {r.unchanged} ya estaban, {r.corrections} correcciones"
    )


def main(argv: list[str]) -> int:
    text = CONFIG.read_text()
    cfg = load_config(text)
    fed = nyfed.load_config(text)
    since_arg = date.fromisoformat(argv[1]) if len(argv) > 1 else None
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        for s in cfg.series.values():
            since = max(since_arg, s.start) if since_arg else s.start
            values = parse_csv(fetch_csv(cfg, s, since), s)
            now = datetime.now(UTC)
            rates: list[Rate] = to_rates(values, s, now)
            _report(s.name, values, insert_rates(conn, rates, now))
        for f in fed.series.values():
            since = max(since_arg, f.start) if since_arg else f.start
            now = datetime.now(UTC)
            values = nyfed.parse_json(nyfed.fetch_json(fed, f, since, now.date()), f)
            _report(f.name, values, insert_rates(conn, to_rates(values, f, now), now))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
