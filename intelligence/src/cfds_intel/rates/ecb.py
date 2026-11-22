"""Series diarias del BCE desde su API de datos (#79), en formato `csvdata`.

Cada fila es el valor de un día hábil. Se guarda como `reference_rate.v1`:

```text
ref_date     = TIME_PERIOD (el día hábil al que corresponde)
published_at = día hábil siguiente de la serie, a la hora de publicación (config);
               el último valor, la hora de lectura (todavía no se sabe cuál es el siguiente)
```
El "día hábil siguiente" es la fecha del valor siguiente de la misma serie: la serie solo tiene
días hábiles de TARGET2, así que no hace falta un calendario aparte. Nunca se adelanta la
publicación (CLAUDE.md §5b).
"""

import csv
import io
import re
import tomllib
import urllib.parse
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, date, datetime, time, timedelta
from decimal import Decimal, InvalidOperation
from zoneinfo import ZoneInfo

from cfds_intel.net import https_get

_SERIES = re.compile(r"^[A-Z][A-Z0-9_]{0,31}$")
_FLOW = re.compile(r"^[A-Z0-9_]{1,16}$")
_KEY = re.compile(r"^[A-Z0-9_]+(\.[A-Z0-9_]+)*$")


class RatesError(ValueError):
    """Respuesta o configuración que no cumple las reglas: no se guarda nada."""


@dataclass(frozen=True)
class SeriesConfig:
    name: str
    flow: str
    key: str
    start: date
    publication_time: time
    publication_tz: ZoneInfo


@dataclass(frozen=True)
class RatesConfig:
    base_url: str
    user_agent: str
    timeout_secs: int
    series: dict[str, SeriesConfig]


def load_config(text: str) -> RatesConfig:
    raw = tomllib.loads(text)
    api = raw.get("api", {})
    url = api.get("base_url")
    if not (isinstance(url, str) and url.startswith("https://")):
        raise RatesError("api.base_url tiene que ser https")
    ua = api.get("user_agent")
    if not (isinstance(ua, str) and ua.strip()):
        raise RatesError("api.user_agent tiene que ser un texto")
    timeout = api.get("timeout_secs")
    if not (isinstance(timeout, int) and not isinstance(timeout, bool) and timeout > 0):
        raise RatesError("api.timeout_secs tiene que ser un entero > 0")
    series: dict[str, SeriesConfig] = {}
    for name, s in raw.get("series", {}).items():
        if not _SERIES.match(name):
            raise RatesError(f"series: nombre inválido {name!r}")
        flow, key = s.get("flow"), s.get("key")
        if not (isinstance(flow, str) and _FLOW.match(flow)):
            raise RatesError(f"series.{name}.flow inválido: {flow!r}")
        if not (isinstance(key, str) and _KEY.match(key)):
            raise RatesError(f"series.{name}.key inválida: {key!r}")
        try:
            start = date.fromisoformat(str(s.get("start")))
            at = time.fromisoformat(str(s.get("publication_time")))
            tz = ZoneInfo(str(s.get("publication_timezone")))
        except (ValueError, KeyError) as e:
            raise RatesError(f"series.{name}: start o publicación inválidos: {e}") from e
        series[name] = SeriesConfig(name, flow, key, start, at, tz)
    if not series:
        raise RatesError("falta al menos una serie")
    return RatesConfig(url.rstrip("/"), ua, timeout, series)


@dataclass(frozen=True)
class Rate:
    """Un documento de `reference_rate.v1`."""

    series: str
    ref_date: date
    published_at: datetime
    rate_pct: Decimal

    def to_contract(self) -> dict[str, str]:
        return {
            "series": self.series,
            "ref_date": self.ref_date.isoformat(),
            "published_at": self.published_at.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "rate_pct": str(self.rate_pct),
        }


def parse_csv(text: str, s: SeriesConfig) -> list[tuple[date, Decimal]]:
    """Filas de la API → (fecha, tasa) en orden. Cualquier fila rara rechaza todo."""
    expected_key = f"{s.flow}.{s.key}"
    out: list[tuple[date, Decimal]] = []
    reader = csv.DictReader(io.StringIO(text))
    missing = {"KEY", "TIME_PERIOD", "OBS_VALUE"} - set(reader.fieldnames or [])
    if missing:
        raise RatesError(f"{s.name}: faltan columnas {sorted(missing)}")
    for row in reader:
        if row["KEY"] != expected_key:
            raise RatesError(f"{s.name}: serie no pedida en la respuesta: {row['KEY']!r}")
        try:
            day = date.fromisoformat(row["TIME_PERIOD"])
        except ValueError as e:
            raise RatesError(f"{s.name}: fecha inválida {row['TIME_PERIOD']!r}") from e
        raw = row["OBS_VALUE"]
        try:
            value = Decimal(raw)
        except InvalidOperation as e:
            raise RatesError(f"{s.name} {day}: {raw!r} no es un número") from e
        if not value.is_finite():
            raise RatesError(f"{s.name} {day}: {raw!r} no es un número")
        if out and day <= out[-1][0]:
            raise RatesError(f"{s.name}: fechas desordenadas o repetidas en {day}")
        out.append((day, value))
    return out


def to_rates(
    values: list[tuple[date, Decimal]], s: SeriesConfig, retrieved_at: datetime
) -> list[Rate]:
    """(fecha, tasa) en orden → registros con su hora de publicación, sin adelantarla nunca."""
    if retrieved_at.tzinfo is None:
        raise RatesError("retrieved_at tiene que tener zona horaria")
    read = retrieved_at.astimezone(UTC)
    out: list[Rate] = []
    for i, (day, value) in enumerate(values):
        if i + 1 < len(values):
            nxt = datetime.combine(values[i + 1][0], s.publication_time, s.publication_tz)
            published = nxt.astimezone(UTC)
        else:
            published = read
        end_of_day = datetime.combine(day + timedelta(days=1), time(), s.publication_tz)
        if published < end_of_day.astimezone(UTC):
            raise RatesError(f"{s.name} {day}: leído antes de que terminara ese día")
        out.append(Rate(s.name, day, published, value))
    return out


Opener = Callable[[urllib.request.Request, int], bytes]


def fetch_csv(cfg: RatesConfig, s: SeriesConfig, since: date, opener: Opener = https_get) -> str:
    """La serie `s` desde `since`, en CSV."""
    query = urllib.parse.urlencode({"startPeriod": since.isoformat(), "format": "csvdata"})
    req = urllib.request.Request(  # noqa: S310 (https, validado)
        f"{cfg.base_url}/{s.flow}/{s.key}?{query}",
        headers={"User-Agent": cfg.user_agent, "Accept": "text/csv"},
    )
    return opener(req, cfg.timeout_secs).decode("utf-8")
