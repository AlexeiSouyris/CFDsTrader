"""Universo del NASDAQ 100 sin sesgo de supervivencia (#10, ADR 0005).

Fuente: `nasdaq-100-ticker-history` (licencia MIT, https://github.com/jmccarrell/n100tickers),
fijada en `pyproject.toml` al commit 4ce8525 (versión 2026.17.1).

Para backtests y análogos se usa `members_between`: todas las empresas que estuvieron en el índice
en algún momento del período, **incluidas las que salieron** (fusiones, quiebras o caídas del
índice). Usar solo los componentes de hoy inflaría los resultados.
"""

from datetime import date, timedelta
from typing import cast

from nasdaq_100_ticker_history import tickers_as_of

# Cobertura de la versión fijada, según su README. No es un parámetro ajustable: es un dato
# de la fuente. Al actualizar la dependencia, se actualiza acá también.
COVERAGE_START = date(2003, 1, 1)
COVERAGE_END = date(2026, 11, 17)


class OutsideCoverageError(ValueError):
    """La fecha está fuera de lo que cubre la fuente: no se supone nada."""


def _check(day: date) -> None:
    if not COVERAGE_START <= day <= COVERAGE_END:
        raise OutsideCoverageError(
            f"{day} fuera de la cobertura de la fuente ({COVERAGE_START} a {COVERAGE_END})"
        )


def members_on(day: date) -> frozenset[str]:
    """Tickers del NASDAQ 100 ese día."""
    _check(day)
    tickers = cast(frozenset[str], tickers_as_of(day.year, day.month, day.day))
    return frozenset(str(t) for t in tickers)


def members_between(start: date, end: date) -> frozenset[str]:
    """Todas las empresas que estuvieron en el índice algún día de `[start, end]`."""
    if start > end:
        raise ValueError(f"rango invertido: {start} > {end}")
    _check(start)
    _check(end)
    universe: set[str] = set()
    day = start
    while day <= end:
        universe |= members_on(day)
        day += timedelta(days=1)
    return frozenset(universe)
