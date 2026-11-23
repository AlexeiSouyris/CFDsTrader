"""Lector del feed JSON oficial del calendario de Forex Factory (ADR 0010, #239).

Reglas:
- Solo el feed de exportación (`config/calendar.toml`), con User-Agent honesto y por la salida
  HTTPS única (`cfds_intel.net`). Nada de scraping ni de evadir la detección de bots.
- El campo `country` del feed es la **moneda**: el país sale de `[currencies]`. Una moneda que
  no está en la config no se guarda (se cuenta): no se adivina el país.
- `impact` pasa a minúsculas (`low`, `medium`, `high`). Los `Holiday` (feriados bancarios) se
  **descartan** (se cuentan): no son publicaciones que muevan el precio, el contrato
  `economic_event.v1` no tiene ese impacto y los feriados de las bolsas ya vienen de
  `config/holidays/`.
- `date` trae offset: `scheduled_at` se pasa a UTC. Sin offset es un error: no se supone zona.
- `published_at` es **la hora en que lo leímos** (la primera vez que lo vimos lo decide el
  guardado), nunca la hora del evento: antes de leerlo no existía para el sistema (ADR 0006 §3).
- Cualquier evento que no cumple las reglas hace fallar el feed entero: no se guarda nada.
"""

import json
import re
import tomllib
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Any

from cfds_intel.net import https_get

_LABEL = re.compile(r"^[a-z][a-z0-9_]{0,31}$")
_COUNTRY = re.compile(r"^[A-Z]{2}$")
_CURRENCY = re.compile(r"^[A-Z]{3}$")
_IMPACTS = {"Low": "low", "Medium": "medium", "High": "high"}
_HOLIDAY = "Holiday"
_MAX_EVENT = 200
_MAX_EVENTS = 2000


class CalendarError(ValueError):
    """El feed o la configuración no cumplen las reglas: no se guarda nada."""


@dataclass(frozen=True)
class CalendarConfig:
    user_agent: str
    timeout_secs: int
    source: str
    url: str
    currencies: dict[str, str]


def load_config(text: str) -> CalendarConfig:
    """Lee `config/calendar.toml` y lo valida."""
    raw = tomllib.loads(text)
    client, source = raw.get("client", {}), raw.get("source", {})
    user_agent, timeout = client.get("user_agent"), client.get("timeout_secs")
    if not isinstance(user_agent, str) or not user_agent.strip():
        raise CalendarError("client.user_agent tiene que ser un texto")
    if not isinstance(timeout, int) or isinstance(timeout, bool) or timeout <= 0:
        raise CalendarError("client.timeout_secs tiene que ser un entero mayor que 0")
    name, url = source.get("name"), source.get("url")
    if not (isinstance(name, str) and _LABEL.match(name)):
        raise CalendarError(f"source.name inválido: {name!r}")
    if not (isinstance(url, str) and url.startswith("https://")):
        raise CalendarError(f"source.url tiene que ser https: {url!r}")
    currencies = raw.get("currencies")
    if not isinstance(currencies, dict) or not currencies:
        raise CalendarError("falta [currencies]")
    for cur, country in currencies.items():
        if not _CURRENCY.match(cur) or not (isinstance(country, str) and _COUNTRY.match(country)):
            raise CalendarError(f'currencies: {cur!r} = {country!r} no es MONEDA = "PA"')
    unknown = set(raw) - {"client", "source", "currencies"}
    if unknown:
        raise CalendarError(f"secciones desconocidas: {sorted(unknown)}")
    return CalendarConfig(user_agent, timeout, name, url, dict(currencies))


def _utc_z(t: datetime) -> str:
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


@dataclass(frozen=True)
class EconomicEvent:
    """Un evento de `contracts/economic_event.v1` (siempre de un país: sin `instrument_id`)."""

    source: str
    event: str
    country: str
    impact: str
    scheduled_at: datetime
    published_at: datetime

    def key(self) -> tuple[str, str, str, str, datetime]:
        """Clave natural: lo mismo en otra lectura no es un evento nuevo."""
        return (self.source, self.event, self.country, self.impact, self.scheduled_at)

    def to_contract(self) -> dict[str, str | None]:
        return {
            "source": self.source,
            "event": self.event,
            "country": self.country,
            "instrument_id": None,
            "impact": self.impact,
            "scheduled_at": _utc_z(self.scheduled_at),
            "published_at": _utc_z(self.published_at),
        }


@dataclass(frozen=True)
class Parsed:
    events: list[EconomicEvent]
    holidays: int
    unknown_currencies: dict[str, int]


def _scheduled(raw: object, where: str) -> datetime:
    if not isinstance(raw, str):
        raise CalendarError(f"{where}: sin date")
    try:
        t = datetime.fromisoformat(raw)
    except ValueError as e:
        raise CalendarError(f"{where}: date inválido {raw!r}") from e
    if t.tzinfo is None:
        raise CalendarError(f"{where}: date sin zona horaria {raw!r}")
    return t.astimezone(UTC)


def parse_feed(body: bytes, cfg: CalendarConfig, retrieved_at: datetime) -> Parsed:
    """Eventos del feed, vistos en `retrieved_at` (con zona horaria)."""
    if retrieved_at.tzinfo is None:
        raise CalendarError("retrieved_at tiene que tener zona horaria")
    seen_at = retrieved_at.astimezone(UTC).replace(microsecond=0)
    try:
        doc: Any = json.loads(body)
    except (json.JSONDecodeError, UnicodeDecodeError) as e:
        raise CalendarError("el feed no es JSON") from e
    if not isinstance(doc, list) or len(doc) > _MAX_EVENTS:
        raise CalendarError("el feed tiene que ser una lista de eventos")
    events: dict[tuple[str, str, str, str, datetime], EconomicEvent] = {}
    holidays = 0
    unknown: dict[str, int] = {}
    for i, item in enumerate(doc):
        where = f"evento {i}"
        if not isinstance(item, dict):
            raise CalendarError(f"{where}: no es un objeto")
        title, currency, impact = item.get("title"), item.get("country"), item.get("impact")
        if not isinstance(title, str) or not 0 < len(title.strip()) <= _MAX_EVENT:
            raise CalendarError(f"{where}: title vacío o de más de {_MAX_EVENT} caracteres")
        if not isinstance(currency, str) or not currency:
            raise CalendarError(f"{where}: sin country (moneda)")
        scheduled = _scheduled(item.get("date"), where)
        if impact == _HOLIDAY:
            holidays += 1
            continue
        if impact not in _IMPACTS:
            raise CalendarError(f"{where}: impact desconocido {impact!r}")
        country = cfg.currencies.get(currency)
        if country is None:
            unknown[currency] = unknown.get(currency, 0) + 1
            continue
        ev = EconomicEvent(
            source=cfg.source,
            event=title.strip(),
            country=country,
            impact=_IMPACTS[impact],
            scheduled_at=scheduled,
            published_at=seen_at,
        )
        events.setdefault(ev.key(), ev)  # el mismo evento dos veces en el feed cuenta una
    return Parsed(list(events.values()), holidays, unknown)


Opener = Callable[[urllib.request.Request, int], bytes]


def fetch(cfg: CalendarConfig, opener: Opener = https_get) -> Parsed:
    """Baja el feed y lo convierte. `opener` se reemplaza en los tests (sin red en CI)."""
    req = urllib.request.Request(cfg.url, headers={"User-Agent": cfg.user_agent})  # noqa: S310
    body = opener(req, cfg.timeout_secs)
    return parse_feed(body, cfg, datetime.now(UTC))
