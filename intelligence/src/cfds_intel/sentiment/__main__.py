"""Carga el COT desde la API pública de la CFTC y lo guarda (#113).

```text
DATABASE_URL=... uv run python -m cfds_intel.sentiment [desde AAAA-MM-DD]
```
Sin fecha, carga desde el 2010-06-15 (inicio de las series consolidadas).
"""

import os
import sys
from datetime import UTC, date, datetime
from pathlib import Path

import psycopg

from cfds_intel.sentiment.cot import fetch_rows, load_config, to_observations
from cfds_intel.sentiment.store import insert_observations

CONFIG = Path(__file__).resolve().parents[4] / "config" / "cot.toml"


def main(argv: list[str]) -> int:
    cfg = load_config(CONFIG.read_text())
    since = date.fromisoformat(argv[1]) if len(argv) > 1 else date(2010, 6, 15)
    rows = list(fetch_rows(cfg, since))
    now = datetime.now(UTC)
    obs = to_observations(rows, cfg, now)
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        r = insert_observations(conn, obs, now)
    reports = {(r_["cftc_contract_market_code"], r_["report_date_as_yyyy_mm_dd"]) for r_ in rows}
    print(
        f"COT: {len(reports)} informes, {len(obs)} observaciones → "
        f"{r.inserted} nuevas, {r.unchanged} ya estaban, {r.corrections} correcciones"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
