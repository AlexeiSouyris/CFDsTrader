"""Tasas de referencia en USD de la Reserva Federal de Nueva York (#467, ADR 0015).

EFFR (Effective Federal Funds Rate) y SOFR (Secured Overnight Financing Rate), desde la API
pública de la NY Fed (`/api/rates/<tipo>/<serie>/search.json`). Son la tasa que descuenta la base
de los futuros de índices de CME. Se guardan como `reference_rate.v1`, igual que las del BCE:

```text
ref_date     = effectiveDate (el día hábil al que corresponde)
published_at = fecha del valor siguiente de la serie, a la hora de publicación (config);
               el último valor, la hora de lectura
```
La NY Fed publica cada valor el día hábil siguiente a la mañana y puede revisarlo ese mismo día
a las 14:30 ET; se toma una hora posterior (config). Nunca se adelanta la publicación
(CLAUDE.md §5b).
"""

import json
import re
import tomllib
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import date, time
from decimal import Decimal
from zoneinfo import ZoneInfo

from cfds_intel.net import https_get
from cfds_intel.rates.ecb import Opener, RatesError

_SERIES = re.compile(r"^[A-Z][A-Z0-9_]{0,31}$")
_PATH = re.compile(r"^[a-z]+/[a-z]+$")


@dataclass(frozen=True)
class NyFedSeries:
    name: str
    # Ruta de la API, por ejemplo "secured/sofr".
    path: str
    # Valor de `type` en la respuesta ("SOFR", "EFFR").
    type: str
    start: date
    publication_time: time
    publication_tz: ZoneInfo


@dataclass(frozen=True)
class NyFedConfig:
    base_url: str
    user_agent: str
    timeout_secs: int
    series: dict[str, NyFedSeries]


def load_config(text: str) -> NyFedConfig:
    """La tabla `[nyfed]` de `config/reference_rates.toml`."""
    raw = tomllib.loads(text).get("nyfed")
    if not isinstance(raw, dict):
        raise RatesError("falta la tabla [nyfed]")
    url = raw.get("base_url")
    if not (isinstance(url, str) and url.startswith("https://")):
        raise RatesError("nyfed.base_url tiene que ser https")
    ua = raw.get("user_agent")
    if not (isinstance(ua, str) and ua.strip()):
        raise RatesError("nyfed.user_agent tiene que ser un texto")
    timeout = raw.get("timeout_secs")
    if not (isinstance(timeout, int) and not isinstance(timeout, bool) and timeout > 0):
        raise RatesError("nyfed.timeout_secs tiene que ser un entero > 0")
    series: dict[str, NyFedSeries] = {}
    for name, s in raw.get("series", {}).items():
        if not _SERIES.match(name):
            raise RatesError(f"nyfed.series: nombre inválido {name!r}")
        path, kind = s.get("path"), s.get("type")
        if not (isinstance(path, str) and _PATH.match(path)):
            raise RatesError(f"nyfed.series.{name}.path inválido: {path!r}")
        if not (isinstance(kind, str) and _SERIES.match(kind)):
            raise RatesError(f"nyfed.series.{name}.type inválido: {kind!r}")
        try:
            start = date.fromisoformat(str(s.get("start")))
            at = time.fromisoformat(str(s.get("publication_time")))
            tz = ZoneInfo(str(s.get("publication_timezone")))
        except (ValueError, KeyError) as e:
            raise RatesError(f"nyfed.series.{name}: start o publicación inválidos: {e}") from e
        series[name] = NyFedSeries(name, path, kind, start, at, tz)
    if not series:
        raise RatesError("nyfed: falta al menos una serie")
    return NyFedConfig(url.rstrip("/"), ua, timeout, series)


def parse_json(text: str, s: NyFedSeries) -> list[tuple[date, Decimal]]:
    """Respuesta de la API → (fecha, tasa) en orden de fecha. Cualquier dato raro rechaza todo.

    La API manda los días de más nuevo a más viejo; una fecha repetida es un error.
    """
    try:
        doc = json.loads(text, parse_float=Decimal, parse_int=Decimal)
    except json.JSONDecodeError as e:
        raise RatesError(f"{s.name}: la respuesta no es JSON") from e
    rows = doc.get("refRates") if isinstance(doc, dict) else None
    if not isinstance(rows, list):
        raise RatesError(f"{s.name}: falta refRates")
    out: dict[date, Decimal] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise RatesError(f"{s.name}: fila inválida {row!r}")
        if row.get("type") != s.type:
            raise RatesError(f"{s.name}: serie no pedida en la respuesta: {row.get('type')!r}")
        try:
            day = date.fromisoformat(str(row.get("effectiveDate")))
        except ValueError as e:
            raise RatesError(f"{s.name}: fecha inválida {row.get('effectiveDate')!r}") from e
        value = row.get("percentRate")
        if not (isinstance(value, Decimal) and value.is_finite()):
            raise RatesError(f"{s.name} {day}: {value!r} no es un número")
        if day in out:
            raise RatesError(f"{s.name}: fecha repetida {day}")
        out[day] = value
    return sorted(out.items())


def fetch_json(
    cfg: NyFedConfig, s: NyFedSeries, since: date, until: date, opener: Opener = https_get
) -> str:
    """La serie `s` de `since` a `until` (inclusive), en JSON."""
    query = urllib.parse.urlencode({"startDate": since.isoformat(), "endDate": until.isoformat()})
    req = urllib.request.Request(  # noqa: S310 (https, validado)
        f"{cfg.base_url}/{s.path}/search.json?{query}",
        headers={"User-Agent": cfg.user_agent, "Accept": "application/json"},
    )
    return opener(req, cfg.timeout_secs).decode("utf-8")
