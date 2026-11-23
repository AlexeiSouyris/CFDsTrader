"""Lee el feed del calendario de Forex Factory y guarda los eventos nuevos (ADR 0010, #239).

```text
DATABASE_URL=... uv run python -m cfds_intel.calendar [ruta/a/config/calendar.toml]
```
Es idempotente: un evento ya guardado no se repite y conserva su primer `published_at`. Un
pedido por corrida, sin reintentos: si el feed responde 429 (límite de pedidos), se informa el
`Retry-After` y no se guarda nada. Correrlo cada pocas horas (el archivo cambia una vez por hora).
"""

import os
import sys
import urllib.error
from pathlib import Path

import psycopg

from cfds_intel.calendar.feed import CalendarError, fetch, load_config
from cfds_intel.calendar.store import insert_events

DEFAULT_CONFIG = Path(__file__).resolve().parents[4] / "config" / "calendar.toml"


def main(argv: list[str]) -> int:
    cfg = load_config(Path(argv[1] if len(argv) > 1 else DEFAULT_CONFIG).read_text())
    try:
        parsed = fetch(cfg)
    except urllib.error.HTTPError as e:
        wait = e.headers.get("Retry-After") if e.headers else None
        extra = f" (Retry-After: {wait} s)" if wait else ""
        print(
            f"⚠️  {cfg.source}: el feed respondió {e.code}{extra}. No se reintenta.", file=sys.stderr
        )
        return 1
    except (CalendarError, OSError) as e:
        print(f"⚠️  {cfg.source}: {e}", file=sys.stderr)
        return 1
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        r = insert_events(conn, parsed.events)
    skipped = ", ".join(f"{c}: {n}" for c, n in sorted(parsed.unknown_currencies.items()))
    print(
        f"{cfg.source}: {len(parsed.events)} eventos → {r.inserted} nuevos, "
        f"{r.unchanged} ya estaban; {parsed.holidays} feriados descartados"
        + (f"; monedas sin país en la config: {skipped}" if skipped else "")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
