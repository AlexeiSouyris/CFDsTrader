"""Genera config/holidays/<BOLSA>.toml a partir de librerías abiertas de calendarios.

- XNYS y XETR: `exchange_calendars`.
- CMEGLOBEX_METALS (oro de CME Globex, ADR 0013): `pandas_market_calendars`, calendario
  `CMEGlobex_GC`. Es una sesión que cruza la medianoche: la del día D abre el día anterior a
  las 17:00 de Chicago y cierra D a las 16:00 (18:00 a 17:00 en Nueva York).

Se corre a mano, solo cuando hay que extender el rango de años o actualizar la fuente:

    uv run --no-project --with exchange-calendars==4.13.3 \
        --with pandas-market-calendars==5.6.1 python -I \
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
import pandas_market_calendars as pmc

EXCHANGES = ("XNYS", "XETR")
START, END = "2011-01-01", "2027-12-31"


# Sesión que cruza la medianoche: código en config/sessions.toml → calendario de la librería.
OVERNIGHT = {"CMEGLOBEX_METALS": "CMEGlobex_GC"}


def render(code: str, source: str, closed: list[str], early: dict[str, str], note: str = "") -> str:
    lines = [
        "# Generado por infra/scripts/generate_holidays.py. No editar a mano.",
        *([f"# {note}"] if note else []),
        f'exchange = "{code}"',
        f'source = "{source}"',
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


def generate_overnight(code: str, name: str) -> str:
    """Una sesión por día hábil D: abre D-1 a la hora habitual y cierra D (o antes)."""
    cal = pmc.get_calendar(name)
    tz = cal.tz
    sched = cal.schedule(START, END)
    sessions = {d.date(): row for d, row in sched.iterrows()}
    regular_open = dt.time(17, 0)
    regular_close = dt.time(16, 0)
    closed: list[str] = []
    early: dict[str, str] = {}
    for day in pd.bdate_range(START, END):  # lunes a viernes
        d = day.date()
        if d not in sessions:
            closed.append(d.isoformat())
            continue
        row = sessions[d]
        o = row["market_open"].tz_convert(tz)
        c = row["market_close"].tz_convert(tz)
        if o.date() != d - dt.timedelta(days=1) or o.time() != regular_open:
            raise SystemExit(f"{code} {d}: apertura no estándar {o}, no soportada")
        if c.date() != d:
            raise SystemExit(f"{code} {d}: cierre en otro día {c}, no soportado")
        if c.time() != regular_close:
            early[d.isoformat()] = c.strftime("%H:%M")
    version = importlib.metadata.version("pandas_market_calendars")
    source = (
        f"pandas_market_calendars {version}, calendario {name} "
        "(https://github.com/rsheftel/pandas_market_calendars)"
    )
    note = "Sesión de D: abre D-1 17:00 y cierra D 16:00, hora de Chicago (config/sessions.toml)."
    return render(code, source, closed, early, note)


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
    source = f"exchange_calendars {version} (https://github.com/gerrymanoim/exchange_calendars)"
    return render(code, source, closed, early)


def main() -> None:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for code in EXCHANGES:
        (out / f"{code}.toml").write_text(generate(code), encoding="utf-8")
        print(f"{code}: {out / f'{code}.toml'}")
    for code, name in OVERNIGHT.items():
        (out / f"{code}.toml").write_text(generate_overnight(code, name), encoding="utf-8")
        print(f"{code}: {out / f'{code}.toml'}")


if __name__ == "__main__":
    main()
