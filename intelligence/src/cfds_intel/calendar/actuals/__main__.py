"""Lee de la API pública v1 del BLS el dato real del CPI y del empleo (ADR 0018 §2, #697).

```text
DATABASE_URL=... uv run python -m cfds_intel.calendar.actuals [--dry-run] [ruta/config.toml]
```
Para correr una vez por día hábil (cron del Mac mini). Busca en `official_releases` las
publicaciones del BLS de los últimos `lookback_days` que ya pasaron al día hábil siguiente y no
tienen su primer dato, y hace **una consulta por publicación**. Si no hay ninguna, no pide nada.
Ante un 403 o un 429 frena todo, sin reintentar (ADR 0016 §1.4). `--dry-run` solo dice qué
consultaría, sin pedir nada ni guardar.
"""

import os
import sys
import urllib.error
from datetime import UTC, datetime
from pathlib import Path

import psycopg

from cfds_intel.calendar.actuals import (
    ActualsError,
    build_actuals,
    due_releases,
    effective_releases,
    load_config,
    query,
)
from cfds_intel.calendar.actuals.store import insert_actuals, periods_with_first, release_rows

DEFAULT_CONFIG = Path(__file__).resolve().parents[5] / "config" / "calendar_surprise.toml"


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if a != "--dry-run"]
    dry_run = "--dry-run" in argv[1:]
    cfg = load_config(Path(args[0] if args else DEFAULT_CONFIG).read_text())
    now = datetime.now(UTC)
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        releases = effective_releases(release_rows(conn), now)
        done_series = periods_with_first(conn, [s.series_id for s in cfg.series])
        done = {
            (r.series, r.reference_period)
            for r in releases
            if all(
                (s.series_id, r.reference_period) in done_series for s in cfg.series_of(r.series)
            )
        }
        due = due_releases(releases, now, cfg.lookback_days, done)
        if not due:
            print("bls: ninguna publicación pendiente; no se consulta")
            return 0
        for rel in due:
            label = f"{rel.series} {rel.reference_period}"
            if dry_run:
                print(f"bls: consultaría {label} (publicado {rel.released_at:%Y-%m-%d %H:%M}Z)")
                continue
            try:
                values = query(cfg, rel)
            except urllib.error.HTTPError as e:
                print(f"⚠️  bls: la API respondió {e.code} en {label}. Se frena.", file=sys.stderr)
                return 1
            except (ActualsError, OSError) as e:
                print(f"⚠️  bls: {label}: {e}", file=sys.stderr)
                return 1
            prev = [
                r
                for r in releases
                if r.series == rel.series and r.next_released_at == rel.released_at
            ]
            rows = build_actuals(rel, prev[0] if prev else None, cfg, values, datetime.now(UTC))
            if not any(r.reference_period == rel.reference_period for r in rows):
                print(f"bls: {label} todavía no está en la API; se vuelve a mirar otro día")
                continue
            rep = insert_actuals(conn, rows)
            print(
                f"bls: {label} → {rep.inserted} filas nuevas, {rep.unchanged} ya estaban "
                f"(fuente: U.S. Bureau of Labor Statistics, leído {datetime.now(UTC):%Y-%m-%d})"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
