"""Carga los comunicados del FOMC 2011-2025 de federalreserve.gov (ADR 0016, #520).

```text
# 1. Sin base: pide las páginas y muestra las filas y lo que quedaría fuera.
uv run python -m cfds_intel.calendar.official --dry-run
# 2. La carga real (una sola vez, en el Mac mini):
DATABASE_URL=... uv run python -m cfds_intel.calendar.official
```
Son unas 160 páginas, de a una y con `pause_secs` entre pedidos (`config/official_calendar.toml`):
tarda unos 4 minutos. No gasta cupo de IG. Es idempotente: una segunda corrida no agrega nada, y
una fila guardada con otros datos es un error (no se pisa).

Si una reunión programada no figura con sus fechas exactas en un anuncio del calendario
anterior a ella, se marca y la carga **no guarda nada** (ADR 0016 §6: se marca y se pregunta).
`--allow-excluded` guarda el resto y deja esas reuniones fuera: usarlo solo con el visto bueno
del dueño. Si la Fed responde un error (403, 404, 429), la carga frena sin reintentar.
"""

import argparse
import os
import sys
from pathlib import Path

import psycopg

from cfds_intel.calendar.official.fed import load_fed
from cfds_intel.calendar.official.model import OfficialCalendarError, load_config, utc_z
from cfds_intel.calendar.official.pages import PoliteFetcher
from cfds_intel.calendar.official.store import insert_releases

DEFAULT_CONFIG = Path(__file__).resolve().parents[5] / "config" / "official_calendar.toml"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="python -m cfds_intel.calendar.official")
    ap.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    ap.add_argument("--dry-run", action="store_true", help="no guarda: muestra las filas")
    ap.add_argument(
        "--allow-excluded",
        action="store_true",
        help="guarda aunque haya reuniones marcadas (quedan fuera); solo con el ok del dueño",
    )
    args = ap.parse_args(argv[1:])
    cfg = load_config(args.config.read_text())
    fed = cfg.fed
    fetcher = PoliteFetcher(
        cfg.user_agent,
        cfg.timeout_secs,
        cfg.pause_secs,
        (fed.historical_url.split("{year}")[0], fed.calendars_url, fed.release_url_prefix),
    )
    try:
        result = load_fed(fed, fetcher)
    except OfficialCalendarError as e:
        print(f"⚠️  fed_fomc: {e}", file=sys.stderr)
        return 1
    finally:
        print(f"fed_fomc: {fetcher.requests} páginas pedidas", file=sys.stderr)
    for line in result.skipped:
        print(f"  sin comunicado: {line}")
    for line in result.excluded:
        print(f"  ⚠️  MARCADA, queda fuera: {line}", file=sys.stderr)
    by_status: dict[str, int] = {}
    for r in result.releases:
        by_status[r.status] = by_status.get(r.status, 0) + 1
    summary = ", ".join(f"{s}: {n}" for s, n in sorted(by_status.items()))
    print(
        f"fed_fomc: {len(result.releases)} comunicados ({summary}), {len(result.excluded)} marcados"
    )
    if args.dry_run:
        for r in result.releases:
            at = utc_z(r.scheduled_at) if r.scheduled_at else "-"
            print(
                f"  {r.reference_period} {r.status:<11} {at}"
                f" desde {utc_z(r.published_at)} {r.time_rule}"
                f"{' +conferencia' if r.with_press_conference else ''}"
            )
        return 0
    if result.excluded and not args.allow_excluded:
        print(
            "⚠️  no se guarda nada: hay reuniones marcadas. Preguntá al dueño (ADR 0016 §6) y,"
            " con su ok, corré con --allow-excluded.",
            file=sys.stderr,
        )
        return 1
    try:
        with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
            rep = insert_releases(conn, result.releases)
    except OfficialCalendarError as e:
        print(f"⚠️  fed_fomc: {e}", file=sys.stderr)
        return 1
    print(f"fed_fomc: {rep.inserted} nuevos, {rep.unchanged} ya estaban")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
