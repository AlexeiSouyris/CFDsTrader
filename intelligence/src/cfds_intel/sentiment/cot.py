"""COT de la CFTC, "TFF - Futures Only", desde la API pública oficial (#113, ADR 0004).

Cada fila de la API es un informe semanal de un mercado. Se guardan las posiciones crudas (en
contratos) como observaciones de `sentiment_observation.v1`:

```text
observed_for = fecha del informe (el martes), 00:00 UTC
published_at = min(martes + lag_days a la hora de publicación del Este, hora de lectura)
```
Lo segundo es conservador: un dato nunca existe para el sistema antes de que pudiera conocerse.
"""

import json
import re
import tomllib
import urllib.parse
import urllib.request
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from datetime import UTC, date, datetime, time, timedelta
from decimal import Decimal, InvalidOperation
from zoneinfo import ZoneInfo

from cfds_intel.net import https_get

SOURCE = "cftc_cot"
_LABEL = re.compile(r"^[a-z][a-z0-9_]{0,31}$")
_INSTRUMENT = re.compile(r"^[A-Z0-9][A-Z0-9._-]{0,31}$")
_COLUMN = re.compile(r"^[a-z][a-z0-9_]{0,63}$")
_MARKET = re.compile(r"^[A-Z0-9+]{1,10}$")


class CotError(ValueError):
    """Respuesta o configuración que no cumple las reglas: no se guarda nada."""


@dataclass(frozen=True)
class CotConfig:
    base_url: str
    user_agent: str
    timeout_secs: int
    page_size: int
    lag: timedelta
    publication_time: time
    publication_tz: ZoneInfo
    markets: dict[str, str]
    series: dict[str, str]


def load_config(text: str) -> CotConfig:
    raw = tomllib.loads(text)
    api, pub = raw.get("api", {}), raw.get("publication", {})
    url = api.get("base_url")
    if not (isinstance(url, str) and url.startswith("https://")):
        raise CotError("api.base_url tiene que ser https")
    ua = api.get("user_agent")
    if not (isinstance(ua, str) and ua.strip()):
        raise CotError("api.user_agent tiene que ser un texto")
    ints = {k: api.get(k) for k in ("timeout_secs", "page_size")}
    if not all(isinstance(v, int) and not isinstance(v, bool) and v > 0 for v in ints.values()):
        raise CotError("api.timeout_secs y api.page_size tienen que ser enteros > 0")
    lag = pub.get("lag_days")
    if not (isinstance(lag, int) and not isinstance(lag, bool) and 3 <= lag <= 14):
        raise CotError("publication.lag_days tiene que estar entre 3 (viernes) y 14")
    try:
        at = time.fromisoformat(str(pub.get("time")))
        tz = ZoneInfo(str(pub.get("timezone")))
    except (ValueError, KeyError) as e:
        raise CotError(f"publication.time o publication.timezone inválidos: {e}") from e
    markets = raw.get("markets", {})
    series = raw.get("series", {})
    if not markets or not series:
        raise CotError("faltan markets o series")
    for code, inst in markets.items():
        if not (_MARKET.match(code) and isinstance(inst, str) and _INSTRUMENT.match(inst)):
            raise CotError(f"markets: {code!r} = {inst!r} inválido")
    for col, name in series.items():
        if not (_COLUMN.match(col) and isinstance(name, str) and _LABEL.match(name)):
            raise CotError(f"series: {col!r} = {name!r} inválido")
    if len(set(series.values())) != len(series):
        raise CotError("series: nombre repetido")
    return CotConfig(
        base_url=url,
        user_agent=ua,
        timeout_secs=ints["timeout_secs"],
        page_size=ints["page_size"],
        lag=timedelta(days=lag),
        publication_time=at,
        publication_tz=tz,
        markets=dict(markets),
        series=dict(series),
    )


@dataclass(frozen=True)
class Observation:
    """Un documento de `sentiment_observation.v1`."""

    series: str
    instrument_id: str
    observed_for: datetime
    published_at: datetime
    value: Decimal

    def to_contract(self) -> dict[str, str]:
        z = "%Y-%m-%dT%H:%M:%SZ"
        return {
            "source": SOURCE,
            "series": self.series,
            "instrument_id": self.instrument_id,
            "observed_for": self.observed_for.astimezone(UTC).strftime(z),
            "published_at": self.published_at.astimezone(UTC).strftime(z),
            "value": str(self.value),
        }


def available_from(report: date, cfg: CotConfig) -> datetime:
    """Desde cuándo se toma como publicado el informe de `report` (sin contar la lectura)."""
    local = datetime.combine(report + cfg.lag, cfg.publication_time, cfg.publication_tz)
    return local.astimezone(UTC)


def _report_date(raw: object) -> date:
    if not isinstance(raw, str):
        raise CotError(f"report_date inválida: {raw!r}")
    try:
        d = datetime.fromisoformat(raw)
    except ValueError as e:
        raise CotError(f"report_date inválida: {raw!r}") from e
    if (d.hour, d.minute, d.second) != (0, 0, 0):
        raise CotError(f"report_date con hora: {raw!r}")
    return d.date()


def to_observations(
    rows: list[dict[str, object]], cfg: CotConfig, retrieved_at: datetime
) -> list[Observation]:
    """Filas de la API → observaciones. Un valor faltante o no entero rechaza todo."""
    if retrieved_at.tzinfo is None:
        raise CotError("retrieved_at tiene que tener zona horaria")
    out: list[Observation] = []
    for row in rows:
        code = row.get("cftc_contract_market_code")
        if code not in cfg.markets:
            raise CotError(f"mercado no pedido en la respuesta: {code!r}")
        report = _report_date(row.get("report_date_as_yyyy_mm_dd"))
        observed = datetime.combine(report, time(), UTC)
        published = min(available_from(report, cfg), retrieved_at.astimezone(UTC))
        if published < observed:
            raise CotError(f"{code} {report}: leído antes de la fecha del informe")
        for col, name in cfg.series.items():
            raw = row.get(col)
            try:
                value = Decimal(str(raw))
            except InvalidOperation as e:
                raise CotError(f"{code} {report} {col}: {raw!r} no es un número") from e
            if raw is None or value != value.to_integral_value() or value < 0:
                raise CotError(f"{code} {report} {col}: {raw!r} no es un entero ≥ 0")
            out.append(Observation(name, cfg.markets[str(code)], observed, published, value))
    return out


Opener = Callable[[urllib.request.Request, int], bytes]


def fetch_rows(
    cfg: CotConfig, since: date, opener: Opener = https_get
) -> Iterator[dict[str, object]]:
    """Filas de los mercados configurados con informe desde `since`, paginadas, en orden."""
    codes = ", ".join(f"'{c}'" for c in sorted(cfg.markets))
    columns = ["cftc_contract_market_code", "report_date_as_yyyy_mm_dd", *cfg.series]
    offset = 0
    while True:
        query = urllib.parse.urlencode(
            {
                "$select": ",".join(columns),
                "$where": f"cftc_contract_market_code in ({codes}) "
                f"and report_date_as_yyyy_mm_dd >= '{since.isoformat()}T00:00:00'",
                "$order": "report_date_as_yyyy_mm_dd, cftc_contract_market_code",
                "$limit": cfg.page_size,
                "$offset": offset,
            }
        )
        req = urllib.request.Request(  # noqa: S310 (https, validado)
            f"{cfg.base_url}?{query}", headers={"User-Agent": cfg.user_agent}
        )
        page = json.loads(opener(req, cfg.timeout_secs))
        if not isinstance(page, list):
            raise CotError("la API no devolvió una lista")
        yield from page
        if len(page) < cfg.page_size:
            return
        offset += cfg.page_size
