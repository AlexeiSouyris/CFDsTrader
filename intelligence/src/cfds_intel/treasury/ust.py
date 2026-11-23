"""Curva par del Tesoro de EE. UU. desde su CSV oficial por año (ADR 0012).

Cada fila es un día hábil del Tesoro, con una columna por plazo. Se guarda como
`treasury_yield.v1`, un registro por día y plazo:

```text
ref_date     = Date (MM/DD/YYYY)
published_at = día hábil siguiente de la serie a la hora de [publication] (09:30 ET);
               el último valor, la hora de lectura (todavía no se sabe cuál es el siguiente)
```
El "día hábil siguiente" es la fecha de la fila siguiente: la serie solo tiene días hábiles del
Tesoro, así que no hace falta un calendario aparte. Una celda vacía (un plazo que ese día no
existía) no se guarda. Nunca se adelanta la publicación (CLAUDE.md §5b).
"""

import csv
import io
import tomllib
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, date, datetime, time
from decimal import Decimal, InvalidOperation
from itertools import pairwise
from zoneinfo import ZoneInfo

from cfds_intel.daily_publication import PublicationError, published_times
from cfds_intel.net import https_get

#: Columna del CSV del Tesoro → plazo de `treasury_yield.v1`. Una columna nueva rechaza todo:
#: se agrega acá (y al contrato) a propósito, nunca en silencio.
COLUMNS: dict[str, str] = {
    "1 Mo": "1M",
    "1.5 Month": "1_5M",
    "2 Mo": "2M",
    "3 Mo": "3M",
    "4 Mo": "4M",
    "6 Mo": "6M",
    "1 Yr": "1Y",
    "2 Yr": "2Y",
    "3 Yr": "3Y",
    "5 Yr": "5Y",
    "7 Yr": "7Y",
    "10 Yr": "10Y",
    "20 Yr": "20Y",
    "30 Yr": "30Y",
}

#: Hora (ET) en que la Fed de Nueva York toma los precios: antes, el valor no existe.
INPUTS_AT = time(15, 30)
NEW_YORK = ZoneInfo("America/New_York")


class TreasuryError(ValueError):
    """Respuesta o configuración que no cumple las reglas: no se guarda nada."""


@dataclass(frozen=True)
class TreasuryConfig:
    csv_url: str
    user_agent: str
    timeout_secs: int
    start_year: int
    publication_time: time
    publication_tz: ZoneInfo


def load_config(text: str) -> TreasuryConfig:
    raw = tomllib.loads(text)
    api = raw.get("api", {})
    url = api.get("csv_url")
    if not (
        isinstance(url, str) and url.startswith("https://home.treasury.gov/") and "{year}" in url
    ):
        raise TreasuryError("api.csv_url tiene que ser https://home.treasury.gov/... con {year}")
    ua = api.get("user_agent")
    if not (isinstance(ua, str) and ua.strip()):
        raise TreasuryError("api.user_agent tiene que ser un texto")
    timeout = api.get("timeout_secs")
    if not (isinstance(timeout, int) and not isinstance(timeout, bool) and timeout > 0):
        raise TreasuryError("api.timeout_secs tiene que ser un entero > 0")
    start = raw.get("load", {}).get("start_year")
    if not (isinstance(start, int) and not isinstance(start, bool) and 1990 <= start <= 2100):
        raise TreasuryError("load.start_year tiene que ser un año")
    pub = raw.get("publication", {})
    try:
        at = time.fromisoformat(str(pub.get("time")))
        tz = ZoneInfo(str(pub.get("timezone")))
    except (ValueError, KeyError) as e:
        raise TreasuryError(f"publication: hora o zona inválidas: {e}") from e
    return TreasuryConfig(url, ua, timeout, start, at, tz)


@dataclass(frozen=True)
class TreasuryYield:
    """Un documento de `treasury_yield.v1`."""

    tenor: str
    ref_date: date
    published_at: datetime
    yield_pct: Decimal

    def to_contract(self) -> dict[str, str]:
        return {
            "tenor": self.tenor,
            "ref_date": self.ref_date.isoformat(),
            "published_at": self.published_at.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "yield_pct": str(self.yield_pct),
        }


Row = tuple[date, dict[str, Decimal]]


def parse_csv(text: str) -> list[Row]:
    """CSV de un año → filas (fecha, {plazo: rendimiento}) en orden ascendente.

    El Tesoro las publica de la más nueva a la más vieja. Cualquier fila rara rechaza todo.
    """
    reader = csv.reader(io.StringIO(text.lstrip("﻿")))
    header = next(reader, None)
    if not header or header[0] != "Date":
        raise TreasuryError(f"encabezado inesperado: {header!r}")
    unknown = [c for c in header[1:] if c not in COLUMNS]
    if unknown:
        raise TreasuryError(f"columnas desconocidas del Tesoro: {unknown}")
    tenors = [COLUMNS[c] for c in header[1:]]
    out: list[Row] = []
    for line in reader:
        if not line:
            continue
        if len(line) != len(header):
            raise TreasuryError(f"fila con {len(line)} celdas en vez de {len(header)}: {line!r}")
        try:
            day = datetime.strptime(line[0], "%m/%d/%Y").date()
        except ValueError as e:
            raise TreasuryError(f"fecha inválida {line[0]!r}") from e
        values: dict[str, Decimal] = {}
        for tenor, cell in zip(tenors, line[1:], strict=True):
            cell = cell.strip()
            if cell in ("", "N/A"):
                continue
            try:
                v = Decimal(cell)
            except InvalidOperation as e:
                raise TreasuryError(f"{day} {tenor}: {cell!r} no es un número") from e
            if not v.is_finite():
                raise TreasuryError(f"{day} {tenor}: {cell!r} no es un número")
            values[tenor] = v
        out.append((day, values))
    out.sort(key=lambda r: r[0])
    for a, b in pairwise(out):
        if a[0] == b[0]:
            raise TreasuryError(f"fecha repetida {a[0]}")
    return out


def to_yields(rows: list[Row], cfg: TreasuryConfig, retrieved_at: datetime) -> list[TreasuryYield]:
    """Filas en orden → registros con su hora de publicación, sin adelantarla nunca."""
    try:
        times = published_times(
            [day for day, _ in rows],
            cfg.publication_time,
            cfg.publication_tz,
            retrieved_at,
            INPUTS_AT,
            NEW_YORK,
        )
    except PublicationError as e:
        raise TreasuryError(str(e)) from e
    return [
        TreasuryYield(t, day, published, v)
        for (day, values), published in zip(rows, times, strict=True)
        for t, v in sorted(values.items())
    ]


Opener = Callable[[urllib.request.Request, int], bytes]


def fetch_year(cfg: TreasuryConfig, year: int, opener: Opener = https_get) -> str:
    """El CSV oficial de un año."""
    req = urllib.request.Request(  # noqa: S310 (https, validado en load_config)
        cfg.csv_url.replace("{year}", str(year)),
        headers={"User-Agent": cfg.user_agent, "Accept": "text/csv"},
    )
    return opener(req, cfg.timeout_secs).decode("utf-8")


def fetch_rows(
    cfg: TreasuryConfig, first_year: int, last_year: int, opener: Opener = https_get
) -> list[Row]:
    """Las filas de varios años, juntas y en orden (el siguiente de un 31-12 está en otro año)."""
    rows: list[Row] = []
    for year in range(first_year, last_year + 1):
        rows.extend(parse_csv(fetch_year(cfg, year, opener)))
    rows.sort(key=lambda r: r[0])
    return rows
