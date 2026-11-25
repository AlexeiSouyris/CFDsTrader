"""Dato real del CPI y del empleo desde la API pública v1 del BLS (ADR 0018 §2, #697).

Contrato: `contracts/release_actual.v1`. Reglas:
- Solo la API v1 sin registro, con User-Agent propio **sin datos personales**
  (`config/calendar_surprise.toml`). Nunca el sitio web del BLS (ADR 0016).
- **Una consulta por publicación**, el día hábil siguiente (la API tiene un día de demora), con
  todas las series de esa publicación. Solo publicaciones de los últimos `lookback_days`: no se
  rellena el pasado. Si ya están todos los primeros datos, no se consulta.
- `released_at` = la hora oficial de la publicación en `official_releases`; `retrieved_at` = la
  hora de la lectura.
- `first` solo si se leyó **antes de la publicación siguiente** de la serie (si esa todavía no
  está en `official_releases`, a no más de `first_max_age_days` de la publicación). Si no,
  `revised`. El mes anterior de la misma consulta va como `revised` (ya lo revisó esta
  publicación): Rust lo usa para la variación.
- Un valor que el BLS no tiene (`-`) no se guarda. Nada se adivina.
"""

import json
import re
import tomllib
import urllib.request
from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from typing import Any
from zoneinfo import ZoneInfo

from cfds_intel.net import https_get

SOURCE = "bls"
ET = ZoneInfo("America/New_York")
_SERIES_ID = re.compile(r"^[A-Z0-9][A-Z0-9_#-]{0,39}$")
_PERIOD = re.compile(r"^\d{4}-(0[1-9]|1[0-2])$")
_DECIMAL = re.compile(r"^-?\d+(\.\d+)?$")
_UNITS = {"percent", "thousands", "millions", "billions", "trillions", "level"}
_RELEASES = {"cpi", "employment_situation"}
_TRANSFORMS = {"level", "change", "pct_change"}
_MAX_SERIES = 25  # por consulta, límite de la API


class ActualsError(ValueError):
    """La config, la respuesta o los datos no cumplen las reglas: no se guarda nada."""


@dataclass(frozen=True)
class SeriesConfig:
    series_id: str
    release: str
    unit: str


@dataclass(frozen=True)
class ActualsConfig:
    api_url: str
    user_agent: str
    timeout_secs: int
    lookback_days: int
    first_max_age_days: int
    series: tuple[SeriesConfig, ...]

    def series_of(self, release: str) -> list[SeriesConfig]:
        return [s for s in self.series if s.release == release]


def _positive_int(raw: dict[str, Any], key: str) -> int:
    v = raw.get(key)
    if not isinstance(v, int) or isinstance(v, bool) or v <= 0:
        raise ActualsError(f"bls.{key} tiene que ser un entero mayor que 0")
    return v


def _check_surprise(items: object, series_ids: set[str]) -> None:
    """Valida [[surprise]] (lo usa Rust): que cada serie exista y la transformación sea conocida."""
    if not isinstance(items, list):
        raise ActualsError("[[surprise]] tiene que ser una lista")
    for i, s in enumerate(items):
        if not isinstance(s, dict) or set(s) != {"event", "country", "series_id", "transform"}:
            raise ActualsError(f"surprise {i}: campos event, country, series_id y transform")
        if s["series_id"] not in series_ids:
            raise ActualsError(f"surprise {i}: serie {s['series_id']!r} sin [[bls.series]]")
        if s["transform"] not in _TRANSFORMS:
            raise ActualsError(f"surprise {i}: transform desconocida {s['transform']!r}")


def load_config(text: str) -> ActualsConfig:
    """Lee `config/calendar_surprise.toml` y lo valida."""
    raw = tomllib.loads(text)
    unknown = set(raw) - {"bls", "surprise"}
    if unknown:
        raise ActualsError(f"secciones desconocidas: {sorted(unknown)}")
    bls = raw.get("bls")
    if not isinstance(bls, dict):
        raise ActualsError("falta [bls]")
    url, ua = bls.get("api_url"), bls.get("user_agent")
    if not (isinstance(url, str) and url.startswith("https://api.bls.gov/publicAPI/v1/")):
        raise ActualsError(f"bls.api_url tiene que ser la API v1 del BLS: {url!r}")
    if not isinstance(ua, str) or not ua.strip() or "@" in ua:
        raise ActualsError("bls.user_agent tiene que ser un texto sin datos de contacto")
    series: list[SeriesConfig] = []
    for i, s in enumerate(bls.get("series", [])):
        if not isinstance(s, dict) or set(s) != {"series_id", "release", "unit", "name"}:
            raise ActualsError(f"bls.series {i}: campos series_id, release, unit y name")
        if not (isinstance(s["series_id"], str) and _SERIES_ID.match(s["series_id"])):
            raise ActualsError(f"bls.series {i}: series_id inválido {s['series_id']!r}")
        if s["release"] not in _RELEASES or s["unit"] not in _UNITS:
            raise ActualsError(f"bls.series {i}: release o unit desconocidos")
        series.append(SeriesConfig(s["series_id"], s["release"], s["unit"]))
    ids = [s.series_id for s in series]
    if not series or len(set(ids)) != len(ids):
        raise ActualsError("bls.series vacío o con series repetidas")
    if any(len([s for s in series if s.release == r]) > _MAX_SERIES for r in _RELEASES):
        raise ActualsError(f"más de {_MAX_SERIES} series en una publicación")
    _check_surprise(raw.get("surprise", []), set(ids))
    return ActualsConfig(
        api_url=url,
        user_agent=ua,
        timeout_secs=_positive_int(bls, "timeout_secs"),
        lookback_days=_positive_int(bls, "lookback_days"),
        first_max_age_days=_positive_int(bls, "first_max_age_days"),
        series=tuple(series),
    )


@dataclass(frozen=True)
class ReleaseRow:
    """Lo que se usa de una fila de `official_releases` (fuente `bls`)."""

    series: str
    reference_period: str
    status: str
    scheduled_at: datetime | None
    published_at: datetime


@dataclass(frozen=True)
class Release:
    """Una publicación tal como se la conoce en `as_of`: su hora y la de la siguiente."""

    series: str
    reference_period: str
    released_at: datetime
    next_released_at: datetime | None


def effective_releases(rows: Iterable[ReleaseRow], as_of: datetime) -> list[Release]:
    """Las publicaciones vigentes en `as_of` (anti look-ahead: solo filas con
    `published_at <= as_of`). De cada serie y mes vale la fila conocida más reciente (una
    reprogramación pisa a la original); si es una cancelación, no hay publicación."""
    latest: dict[tuple[str, str], ReleaseRow] = {}
    for r in rows:
        if r.published_at > as_of:
            continue
        k = (r.series, r.reference_period)
        if k not in latest or r.published_at > latest[k].published_at:
            latest[k] = r
    times: dict[str, list[tuple[datetime, str]]] = {}
    for (series, period), r in latest.items():
        if r.status != "cancelled" and r.scheduled_at is not None:
            times.setdefault(series, []).append((r.scheduled_at, period))
    out: list[Release] = []
    for series, ts in times.items():
        ts.sort()
        for i, (at, period) in enumerate(ts):
            nxt = ts[i + 1][0] if i + 1 < len(ts) else None
            out.append(Release(series, period, at, nxt))
    return sorted(out, key=lambda r: (r.released_at, r.series))


def next_business_day(d: date) -> date:
    """El día hábil siguiente (lunes a viernes; los feriados no se descuentan: solo atrasa)."""
    d += timedelta(days=1)
    while d.weekday() >= 5:
        d += timedelta(days=1)
    return d


def due_releases(
    releases: Sequence[Release], now: datetime, lookback_days: int, done: set[tuple[str, str]]
) -> list[Release]:
    """Las publicaciones que toca consultar en `now`: salieron hace como mucho `lookback_days`,
    ya es el día hábil siguiente (hora del Este) y todavía no tienen su primer dato (`done`)."""
    today = now.astimezone(ET).date()
    return [
        r
        for r in releases
        if now - timedelta(days=lookback_days) <= r.released_at <= now
        and today >= next_business_day(r.released_at.astimezone(ET).date())
        and (r.series, r.reference_period) not in done
    ]


def previous_period(period: str) -> str:
    y, m = int(period[:4]), int(period[5:])
    return f"{y - 1}-12" if m == 1 else f"{y}-{m - 1:02d}"


def request_body(series_ids: Sequence[str], period: str) -> bytes:
    """Consulta v1 con años (firma de la API): del año del mes anterior al del mes."""
    first = previous_period(period)[:4]
    return json.dumps(
        {"seriesid": list(series_ids), "startyear": first, "endyear": period[:4]}
    ).encode()


def parse_response(body: bytes) -> dict[tuple[str, str], Decimal]:
    """Valores mensuales de la respuesta, por (serie, `AAAA-MM`). `-` (sin dato) y los períodos
    que no son meses (`M13`, promedio anual) no se devuelven."""
    try:
        doc: Any = json.loads(body)
    except (json.JSONDecodeError, UnicodeDecodeError) as e:
        raise ActualsError("la respuesta no es JSON") from e
    if not isinstance(doc, dict) or doc.get("status") != "REQUEST_SUCCEEDED":
        msg = doc.get("message") if isinstance(doc, dict) else None
        raise ActualsError(f"el BLS no respondió REQUEST_SUCCEEDED: {msg!r}")
    series = (doc.get("Results") or {}).get("series")
    if not isinstance(series, list):
        raise ActualsError("respuesta sin Results.series")
    out: dict[tuple[str, str], Decimal] = {}
    for s in series:
        sid, data = s.get("seriesID"), s.get("data")
        if not isinstance(sid, str) or not isinstance(data, list):
            raise ActualsError("serie sin seriesID o sin data")
        for d in data:
            year, period, value = d.get("year"), d.get("period"), d.get("value")
            if not (isinstance(year, str) and isinstance(period, str) and isinstance(value, str)):
                raise ActualsError(f"{sid}: dato con year, period o value que no son texto")
            ym = f"{year}-{period[1:]}"
            if not (period.startswith("M") and _PERIOD.match(ym)):
                continue  # M13 = promedio anual
            if value.strip() == "-":
                continue  # el BLS no tiene ese dato (por ejemplo, un corte de fondos)
            if not _DECIMAL.match(value):
                raise ActualsError(f"{sid} {ym}: valor raro {value!r}")
            out[(sid, ym)] = Decimal(value)
    return out


def _utc_z(t: datetime) -> str:
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


@dataclass(frozen=True)
class ReleaseActual:
    """Una fila de `contracts/release_actual.v1`."""

    source: str
    series_id: str
    reference_period: str
    value: Decimal
    unit: str
    vintage: str
    released_at: datetime
    retrieved_at: datetime
    source_url: str

    def to_contract(self) -> dict[str, str]:
        return {
            "source": self.source,
            "series_id": self.series_id,
            "reference_period": self.reference_period,
            "value": str(self.value),
            "unit": self.unit,
            "vintage": self.vintage,
            "released_at": _utc_z(self.released_at),
            "retrieved_at": _utc_z(self.retrieved_at),
            "source_url": self.source_url,
        }


def vintage(release: Release, retrieved_at: datetime, first_max_age_days: int) -> str:
    """`first` solo si se leyó antes de la publicación siguiente de la serie."""
    if retrieved_at < release.released_at:
        raise ActualsError("lectura anterior a la publicación: no puede ser")
    if release.next_released_at is not None:
        return "first" if retrieved_at < release.next_released_at else "revised"
    age = retrieved_at - release.released_at
    return "first" if age <= timedelta(days=first_max_age_days) else "revised"


def build_actuals(
    release: Release,
    previous: Release | None,
    cfg: ActualsConfig,
    values: Mapping[tuple[str, str], Decimal],
    retrieved_at: datetime,
) -> list[ReleaseActual]:
    """Filas de una consulta: el mes de la publicación (`first` o `revised`) y, si se conoce su
    publicación, el mes anterior (`revised`). Lo que falta en la respuesta no se guarda."""
    retrieved_at = retrieved_at.astimezone(UTC).replace(microsecond=0)
    v = vintage(release, retrieved_at, cfg.first_max_age_days)
    out: list[ReleaseActual] = []
    for s in cfg.series_of(release.series):
        targets = [(release, v)] + ([(previous, "revised")] if previous is not None else [])
        for rel, vin in targets:
            value = values.get((s.series_id, rel.reference_period))
            if value is None:
                continue
            out.append(
                ReleaseActual(
                    source=SOURCE,
                    series_id=s.series_id,
                    reference_period=rel.reference_period,
                    value=value,
                    unit=s.unit,
                    vintage=vin,
                    released_at=rel.released_at,
                    retrieved_at=retrieved_at,
                    source_url=cfg.api_url,
                )
            )
    return out


Opener = Callable[[urllib.request.Request, int], bytes]


def query(
    cfg: ActualsConfig, release: Release, opener: Opener = https_get
) -> dict[tuple[str, str], Decimal]:
    """Una consulta (POST) con todas las series de la publicación. `opener` se reemplaza en los
    tests (sin red en CI). Un HTTPError (403, 429) sube tal cual: quien llama frena."""
    ids = [s.series_id for s in cfg.series_of(release.series)]
    req = urllib.request.Request(  # noqa: S310 (https validado en la config)
        cfg.api_url,
        data=request_body(ids, release.reference_period),
        headers={"User-Agent": cfg.user_agent, "Content-Type": "application/json"},
        method="POST",
    )
    return parse_response(opener(req, cfg.timeout_secs))
