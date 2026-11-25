"""Pronóstico (`forecast`) y dato anterior (`previous`) de Forex Factory (ADR 0018 §1, #696).

Contrato: `contracts/calendar_estimate.v1`. Reglas:
- Se guarda el texto **tal cual** vino en el feed; vacío o ausente es `None`.
- El número y su unidad, **solo si se interpretan sin ambigüedad**: un número decimal con
  signo opcional y, como mucho, un sufijo `%`, `K`, `M`, `B` o `T` (sin sufijo, `level`).
  Cualquier otra forma (`<0.1%`, `1.2|1.3`, `0,3%`) queda con el número en `None`: no se adivina.
- La unidad es **una sola** para los dos valores. Si los dos se interpretan con unidades
  distintas (`215K` y `1.2M`), la unidad es ambigua: los dos números quedan en `None`.
- `seen_at` es la hora de la lectura; el guardado decide si el par cambió (fila nueva) o no.
"""

import re
from dataclasses import dataclass
from datetime import UTC, datetime
from decimal import Decimal

_NUMBER = re.compile(r"^(-?\d+(?:\.\d+)?)([%KMBT]?)$")
_UNITS = {
    "%": "percent",
    "K": "thousands",
    "M": "millions",
    "B": "billions",
    "T": "trillions",
    "": "level",
}
MAX_RAW = 50


def parse_value(raw: str | None) -> tuple[Decimal, str] | None:
    """El número y la unidad de un valor del feed, o `None` si no es inequívoco."""
    if raw is None:
        return None
    m = _NUMBER.match(raw)
    if m is None:
        return None
    return Decimal(m.group(1)), _UNITS[m.group(2)]


def _utc_z(t: datetime) -> str:
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


@dataclass(frozen=True)
class CalendarEstimate:
    """Una instantánea de `contracts/calendar_estimate.v1`."""

    source: str
    event: str
    country: str
    scheduled_at: datetime
    forecast_raw: str | None
    previous_raw: str | None
    forecast_value: Decimal | None
    previous_value: Decimal | None
    unit: str | None
    seen_at: datetime

    def event_key(self) -> tuple[str, str, str, datetime]:
        """El evento al que pertenece (como en `economic_events`, sin el impacto)."""
        return (self.source, self.event, self.country, self.scheduled_at)

    def pair(self) -> tuple[str | None, str | None]:
        """Lo que se compara para saber si hay una instantánea nueva."""
        return (self.forecast_raw, self.previous_raw)

    def to_contract(self) -> dict[str, str | None]:
        return {
            "source": self.source,
            "event": self.event,
            "country": self.country,
            "scheduled_at": _utc_z(self.scheduled_at),
            "forecast_raw": self.forecast_raw,
            "previous_raw": self.previous_raw,
            "forecast_value": None if self.forecast_value is None else str(self.forecast_value),
            "previous_value": None if self.previous_value is None else str(self.previous_value),
            "unit": self.unit,
            "seen_at": _utc_z(self.seen_at),
        }


def build_estimate(
    source: str,
    event: str,
    country: str,
    scheduled_at: datetime,
    forecast_raw: str | None,
    previous_raw: str | None,
    seen_at: datetime,
) -> CalendarEstimate:
    """Arma la instantánea e interpreta los números con las reglas del módulo."""
    f, p = parse_value(forecast_raw), parse_value(previous_raw)
    units = {v[1] for v in (f, p) if v is not None}
    if len(units) > 1:  # unidades distintas: ambiguo, no se adivina
        f = p = None
        units = set()
    return CalendarEstimate(
        source=source,
        event=event,
        country=country,
        scheduled_at=scheduled_at,
        forecast_raw=forecast_raw,
        previous_raw=previous_raw,
        forecast_value=None if f is None else f[0],
        previous_value=None if p is None else p[0],
        unit=units.pop() if units else None,
        seen_at=seen_at,
    )
