"""Hora de publicación de una serie diaria sin mirar el futuro (ADR 0012).

Regla común del Tesoro y del VIX:

```text
cada valor (menos el último) → día hábil siguiente de la serie, a la hora `at` de `tz`
el último valor             → la hora en que se leyó (todavía no se sabe cuál es el siguiente)
nunca antes de `earliest` (hora de `earliest_tz`) del día del valor → error, no se guarda nada
```
El "día hábil siguiente" es la fecha del valor siguiente: la serie solo tiene días hábiles.
"""

import itertools
from collections.abc import Sequence
from datetime import UTC, date, datetime, time
from zoneinfo import ZoneInfo


class PublicationError(ValueError):
    """Una hora de publicación que miraría el futuro: no se guarda nada."""


def published_times(
    days: Sequence[date],
    at: time,
    tz: ZoneInfo,
    retrieved_at: datetime,
    earliest: time,
    earliest_tz: ZoneInfo,
) -> list[datetime]:
    """`days` en orden ascendente → la hora (UTC) desde la que vale cada uno."""
    if retrieved_at.tzinfo is None:
        raise PublicationError("retrieved_at tiene que tener zona horaria")
    read = retrieved_at.astimezone(UTC)
    for prev, day in itertools.pairwise(days):
        if day <= prev:
            raise PublicationError(f"fechas desordenadas o repetidas en {day}")
    out: list[datetime] = []
    for i, day in enumerate(days):
        if i + 1 < len(days):
            published = datetime.combine(days[i + 1], at, tz).astimezone(UTC)
        else:
            published = read
        if published < datetime.combine(day, earliest, earliest_tz).astimezone(UTC):
            raise PublicationError(f"{day}: leído antes de que existiera el valor")
        out.append(published)
    return out
