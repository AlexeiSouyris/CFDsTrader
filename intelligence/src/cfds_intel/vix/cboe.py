"""VIX diario desde el CSV histórico oficial de Cboe (ADR 0012 §2).

Se usa por decisión del dueño: solo personal y solo en la base del Mac. Si Cboe responde 403 o
algo que no es el CSV (por ejemplo, un desafío anti-bots), se frena con error y no se reintenta.

```text
ref_date     = DATE (MM/DD/YYYY)
published_at = día hábil siguiente de la serie a la hora de [publication] (09:30 ET);
               el último valor, la hora de lectura. Nunca antes del cierre (16:15 ET).
```
"""

import csv
import io
import tomllib
import urllib.error
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, date, datetime, time
from decimal import Decimal, InvalidOperation
from zoneinfo import ZoneInfo

from cfds_intel.daily_publication import PublicationError, published_times
from cfds_intel.net import https_get

HEADER = ["DATE", "OPEN", "HIGH", "LOW", "CLOSE"]
#: Cierre del VIX: antes de esta hora (ET) el valor del día no existe.
CLOSE_AT = time(16, 15)
NEW_YORK = ZoneInfo("America/New_York")


class VixError(ValueError):
    """Respuesta o configuración que no cumple las reglas: no se guarda nada."""


@dataclass(frozen=True)
class VixConfig:
    csv_url: str
    user_agent: str
    timeout_secs: int
    start: date
    publication_time: time
    publication_tz: ZoneInfo


def load_config(text: str) -> VixConfig:
    raw = tomllib.loads(text)
    api = raw.get("api", {})
    url = api.get("csv_url")
    if not (isinstance(url, str) and url.startswith("https://cdn.cboe.com/")):
        raise VixError("api.csv_url tiene que ser el CSV oficial de https://cdn.cboe.com/")
    ua = api.get("user_agent")
    if not (isinstance(ua, str) and ua.strip()):
        raise VixError("api.user_agent tiene que ser un texto")
    timeout = api.get("timeout_secs")
    if not (isinstance(timeout, int) and not isinstance(timeout, bool) and timeout > 0):
        raise VixError("api.timeout_secs tiene que ser un entero > 0")
    pub = raw.get("publication", {})
    try:
        start = date.fromisoformat(str(raw.get("load", {}).get("start")))
        at = time.fromisoformat(str(pub.get("time")))
        tz = ZoneInfo(str(pub.get("timezone")))
    except (ValueError, KeyError) as e:
        raise VixError(f"load.start o publication inválidos: {e}") from e
    return VixConfig(url, ua, timeout, start, at, tz)


@dataclass(frozen=True)
class VixDay:
    """Un documento de `vix_daily.v1`."""

    ref_date: date
    published_at: datetime
    open: Decimal
    high: Decimal
    low: Decimal
    close: Decimal

    def to_contract(self) -> dict[str, str]:
        return {
            "ref_date": self.ref_date.isoformat(),
            "published_at": self.published_at.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "open": str(self.open),
            "high": str(self.high),
            "low": str(self.low),
            "close": str(self.close),
        }


Row = tuple[date, Decimal, Decimal, Decimal, Decimal]


def _level(day: date, name: str, cell: str) -> Decimal:
    try:
        v = Decimal(cell.strip())
    except InvalidOperation as e:
        raise VixError(f"{day} {name}: {cell!r} no es un número") from e
    if not v.is_finite() or v <= 0:
        raise VixError(f"{day} {name}: {cell!r} no es un nivel válido")
    return v


def parse_csv(text: str, start: date) -> list[Row]:
    """CSV de Cboe → filas desde `start` (fecha, apertura, máximo, mínimo, cierre), en orden.

    Las filas anteriores a `start` no se validan ni se usan: el CSV trae desde 1990, y en los
    primeros años hay días con valores que no son coherentes.
    """
    if text.lstrip().startswith("<"):
        raise VixError("Cboe no devolvió el CSV (¿página HTML o desafío anti-bots?): se frena")
    reader = csv.reader(io.StringIO(text.lstrip("﻿")))
    header = next(reader, None)
    if header != HEADER:
        raise VixError(f"encabezado inesperado: {header!r}")
    out: list[Row] = []
    for line in reader:
        if not line:
            continue
        if len(line) != len(HEADER):
            raise VixError(f"fila con {len(line)} celdas en vez de {len(HEADER)}: {line!r}")
        try:
            day = datetime.strptime(line[0], "%m/%d/%Y").date()
        except ValueError as e:
            raise VixError(f"fecha inválida {line[0]!r}") from e
        if day < start:
            continue
        o, h, lo, c = (_level(day, n, v) for n, v in zip(HEADER[1:], line[1:], strict=True))
        if not (lo <= o <= h and lo <= c <= h):
            raise VixError(f"{day}: apertura, máximo, mínimo y cierre no son coherentes")
        out.append((day, o, h, lo, c))
    out.sort(key=lambda r: r[0])
    return out


def to_days(rows: list[Row], cfg: VixConfig, retrieved_at: datetime) -> list[VixDay]:
    """Filas en orden → registros con su hora de publicación, sin adelantarla nunca."""
    try:
        times = published_times(
            [r[0] for r in rows],
            cfg.publication_time,
            cfg.publication_tz,
            retrieved_at,
            CLOSE_AT,
            NEW_YORK,
        )
    except PublicationError as e:
        raise VixError(str(e)) from e
    return [
        VixDay(day, published, o, h, lo, c)
        for (day, o, h, lo, c), published in zip(rows, times, strict=True)
    ]


Opener = Callable[[urllib.request.Request, int], bytes]


def fetch_csv(cfg: VixConfig, opener: Opener = https_get) -> str:
    """El CSV oficial, una sola vez. Un 403 u otro error HTTP frena todo: no se reintenta."""
    req = urllib.request.Request(  # noqa: S310 (https, validado en load_config)
        cfg.csv_url, headers={"User-Agent": cfg.user_agent, "Accept": "text/csv"}
    )
    try:
        return opener(req, cfg.timeout_secs).decode("utf-8")
    except urllib.error.HTTPError as e:
        raise VixError(f"Cboe respondió {e.code}: se frena sin reintentar (ADR 0012 §2)") from e
