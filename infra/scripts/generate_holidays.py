"""Genera config/holidays/<BOLSA>.toml a partir de la librería abierta `exchange_calendars`.

Se corre a mano, solo cuando hay que extender el rango de años o actualizar la fuente:

    uv run --no-project --with exchange-calendars==4.13.3 python -I \
        infra/scripts/generate_holidays.py config/holidays

El motor (Rust) solo lee los TOML generados; no depende de Python en tiempo de ejecución.
Regla (CLAUDE.md §5b): no inventar datos. Cada archivo indica su fuente y su versión.
"""

import datetime as dt
import importlib.metadata
import sys
from pathlib import Path

import exchange_calendars as xc
import pandas as pd

EXCHANGES = ("XNYS", "XETR")
START, END = "2021-01-01", "2027-12-31"


def generate(code: str) -> str:
    cal = xc.get_calendar(code, start=START, end=END)
    tz = cal.tz
    regular_open = cal.open_times[-1][1]
    regular_close = cal.close_times[-1][1]
    sessions = set(cal.sessions.date)

    closed: list[str] = []
    early: dict[str, str] = {}
    for day in pd.bdate_range(START, END):  # lunes a viernes
        d = day.date()
        if d not in sessions:
            closed.append(d.isoformat())
            continue
        open_local = cal.session_open(day).tz_convert(tz).time()
        close_local = cal.session_close(day).tz_convert(tz).time()
        if open_local != regular_open:
            # Hoy ninguna de las dos bolsas tiene aperturas tardías; si aparecen, soportarlas.
            raise SystemExit(f"{code} {d}: apertura no estándar {open_local}, no soportada")
        if close_local != regular_close:
            early[d.isoformat()] = close_local.strftime("%H:%M")

    version = importlib.metadata.version("exchange_calendars")
    lines = [
        "# Generado por infra/scripts/generate_holidays.py. No editar a mano.",
        f'exchange = "{code}"',
        f'source = "exchange_calendars {version} (https://github.com/gerrymanoim/exchange_calendars)"',
        f'generated_on = "{dt.date.today().isoformat()}"',
        f'from = "{START}"',
        f'to = "{END}"',
        "",
        "# Días de lunes a viernes sin sesión (feriados y cierres extraordinarios).",
        "closed = [",
        *[f'  "{d}",' for d in closed],
        "]",
        "",
        "# Cierres anticipados, en hora local de la bolsa.",
        "[early_closes]",
        *[f'"{d}" = "{t}"' for d, t in early.items()],
        "",
    ]
    return "\n".join(lines)


def main() -> None:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for code in EXCHANGES:
        (out / f"{code}.toml").write_text(generate(code), encoding="utf-8")
        print(f"{code}: {out / f'{code}.toml'}")


if __name__ == "__main__":
    main()
