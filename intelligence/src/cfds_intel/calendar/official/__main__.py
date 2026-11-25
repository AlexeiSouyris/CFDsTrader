"""Carga el calendario histórico oficial (ADR 0016): FOMC (#520) o CPI y empleo del BLS (#608).

FOMC (por defecto, `--source fed`), comunicados 2011-2025 de federalreserve.gov:

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

BLS (`--source bls`), CPI y empleo 2011-2026, **sin robot**: lee solo los HTML que el dueño
guardó a mano en `[bls] raw_dir` (o `--bls-dir`); nunca pide bls.gov (ADR 0016 §1.3 y §1.4).

```text
uv run python -m cfds_intel.calendar.official --source bls --dry-run
DATABASE_URL=... uv run python -m cfds_intel.calendar.official --source bls
```
Las filas que la regla del ADR 0016 §4 deja fuera se listan con su motivo y no se guardan (no es
un error: es la regla). Una estructura inesperada en las páginas hace fallar la carga sin
guardar nada.
"""

import argparse
import os
import sys
from datetime import UTC, datetime
from pathlib import Path

import psycopg

from cfds_intel.calendar.official.bls import load_bls
from cfds_intel.calendar.official.fed import load_fed
from cfds_intel.calendar.official.model import (
    OfficialCalendarConfig,
    OfficialCalendarError,
    OfficialRelease,
    load_config,
    utc_z,
)
from cfds_intel.calendar.official.pages import PoliteFetcher
from cfds_intel.calendar.official.store import insert_releases

REPO_ROOT = Path(__file__).resolve().parents[5]
DEFAULT_CONFIG = REPO_ROOT / "config" / "official_calendar.toml"


def _save(releases: list[OfficialRelease], label: str) -> int:
    try:
        with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
            rep = insert_releases(conn, releases)
    except OfficialCalendarError as e:
        print(f"⚠️  {label}: {e}", file=sys.stderr)
        return 1
    print(f"{label}: {rep.inserted} nuevos, {rep.unchanged} ya estaban")
    return 0


def _print_rows(releases: list[OfficialRelease]) -> None:
    for r in releases:
        at = utc_z(r.scheduled_at) if r.scheduled_at else "-"
        print(
            f"  {r.series:<20} {r.reference_period} {r.status:<11} {at}"
            f" desde {utc_z(r.published_at)} {r.time_rule}"
            f"{' +conferencia' if r.with_press_conference else ''}"
        )


def main_bls(cfg: OfficialCalendarConfig, bls_dir: Path | None, dry_run: bool) -> int:
    if cfg.bls is None:
        print("⚠️  bls: falta la sección [bls] en la config", file=sys.stderr)
        return 1
    raw_dir = bls_dir or Path(cfg.bls.raw_dir)
    if not raw_dir.is_absolute():
        raw_dir = REPO_ROOT / raw_dir
    try:
        result = load_bls(cfg.bls, raw_dir, datetime.now(UTC))
    except OfficialCalendarError as e:
        print(f"⚠️  bls: {e}", file=sys.stderr)
        return 1
    for line in result.excluded:
        print(f"  fuera (ADR 0016 §4): {line}")
    for series in ("cpi", "employment_situation"):
        per_year = " ".join(
            f"{y}:{n}" for (s, y), n in sorted(result.counts.items()) if s == series
        )
        print(f"bls {series}, publicaciones por año en las páginas: {per_year}")
    by_status: dict[str, int] = {}
    for r in result.releases:
        k = f"{r.series}/{r.status}"
        by_status[k] = by_status.get(k, 0) + 1
    summary = ", ".join(f"{s}: {n}" for s, n in sorted(by_status.items()))
    print(f"bls: {len(result.releases)} filas ({summary}), {len(result.excluded)} fuera")
    if dry_run:
        _print_rows(result.releases)
        return 0
    return _save(result.releases, "bls")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="python -m cfds_intel.calendar.official")
    ap.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    ap.add_argument("--dry-run", action="store_true", help="no guarda: muestra las filas")
    ap.add_argument(
        "--allow-excluded",
        action="store_true",
        help="guarda aunque haya reuniones marcadas (quedan fuera); solo con el ok del dueño",
    )
    ap.add_argument("--source", choices=("fed", "bls"), default="fed")
    ap.add_argument(
        "--bls-dir", type=Path, default=None, help="carpeta de los HTML del BLS guardados a mano"
    )
    args = ap.parse_args(argv[1:])
    cfg = load_config(args.config.read_text())
    if args.source == "bls":
        return main_bls(cfg, args.bls_dir, args.dry_run)
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
        _print_rows(result.releases)
        return 0
    if result.excluded and not args.allow_excluded:
        print(
            "⚠️  no se guarda nada: hay reuniones marcadas. Preguntá al dueño (ADR 0016 §6) y,"
            " con su ok, corré con --allow-excluded.",
            file=sys.stderr,
        )
        return 1
    return _save(result.releases, "fed_fomc")


if __name__ == "__main__":
    sys.exit(main(sys.argv))
