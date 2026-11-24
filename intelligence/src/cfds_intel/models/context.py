"""Variables de contexto de las variantes de ML v2 (#432).

Pre-registro: docs/analysis/2026-11-ml-v2-preregistro.md.

Todas **a la hora de la propuesta** (`signal_at`, el cierre de su vela): solo lo publicado hasta
ese momento (CLAUDE.md §5b).

- **Datos publicados (Tesoro, VIX):** el valor vigente es el de la fecha de referencia más
  reciente entre los publicados hasta `t`, en su última corrección publicada hasta `t`. Si esa
  fecha tiene más de `max_age_days` días, falta. Se leen tal cual: no se calcula nada encima
  (CLAUDE.md §1).
- **Análogos:** `cfds_intel.analogs` con la ventana que termina en la última diaria cerrada hasta
  `t`. Su búsqueda solo acepta vecinos cuyo resultado se conocía al cierre de esa ventana.
- Lo que falta queda en `None`: la propuesta no entra a la variante (spec §3, nunca se rellena).
"""

import bisect
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import date, datetime
from decimal import Decimal

import psycopg

from cfds_intel.analogs.config import AnalogsConfig
from cfds_intel.analogs.search import Bar, find_neighbors, hit_first


@dataclass(frozen=True)
class Published:
    ref_date: date
    published_at: datetime
    value: Decimal


class PublishedSeries:
    """Una serie de valores diarios publicados, consultable a cualquier hora sin mirar el futuro."""

    def __init__(self, rows: Sequence[Published]) -> None:
        ordered = sorted(rows, key=lambda r: (r.published_at, r.ref_date))
        self._times = [r.published_at for r in ordered]
        # Para cada prefijo (lo publicado hasta ahí): la fecha más reciente, en su última
        # corrección. Con la misma fecha, gana la publicada después (va después en el orden).
        self._best: list[Published] = []
        best: Published | None = None
        for r in ordered:
            if best is None or r.ref_date >= best.ref_date:
                best = r
            self._best.append(best)

    def at(self, t: datetime, max_age_days: int) -> Decimal | None:
        i = bisect.bisect_right(self._times, t)
        if i == 0:
            return None
        best = self._best[i - 1]
        if (t.date() - best.ref_date).days > max_age_days:
            return None
        return best.value


def load_treasury(conn: psycopg.Connection, tenors: Sequence[str]) -> dict[str, PublishedSeries]:
    out: dict[str, PublishedSeries] = {}
    for tenor in tenors:
        rows = conn.execute(
            "SELECT ref_date, published_at, yield_pct FROM treasury_yields WHERE tenor = %s",
            (tenor,),
        ).fetchall()
        out[tenor] = PublishedSeries([Published(*r) for r in rows])
    return out


def load_vix(conn: psycopg.Connection) -> PublishedSeries:
    rows = conn.execute("SELECT ref_date, published_at, close FROM vix_daily").fetchall()
    return PublishedSeries([Published(*r) for r in rows])


class AnalogFeature:
    """Entre los vecinos de "todos" con ATR: en un largo, la proporción que tocó +2R antes que
    -1R (`up`); en un corto, la que tocó -1R antes que +2R (`down`). Una búsqueda por día."""

    def __init__(
        self, series: Mapping[str, Sequence[Bar]], instrument: str, cfg: AnalogsConfig
    ) -> None:
        self._series = series
        self._instrument = instrument
        self._cfg = cfg
        bars = series.get(instrument, ())
        self._ends = [b.end for b in bars]
        self._cache: dict[int, tuple[int, int, int] | None] = {}

    def _hits(self, end: int) -> tuple[int, int, int] | None:
        if end not in self._cache:
            neighbors = find_neighbors(self._series, self._instrument, end, self._cfg)
            up = down = with_atr = 0
            for n in neighbors:
                hit = hit_first(self._series[n.instrument], n.end_index, self._cfg)
                if hit is None:
                    continue
                with_atr += 1
                up += hit == "up"
                down += hit == "down"
            enough = len(neighbors) >= self._cfg.min_neighbors and with_atr > 0
            self._cache[end] = (up, down, with_atr) if enough else None
        return self._cache[end]

    def at(self, t: datetime, long: bool) -> float | None:
        end = bisect.bisect_right(self._ends, t) - 1
        if end < 0:
            return None
        hits = self._hits(end)
        if hits is None:
            return None
        up, down, with_atr = hits
        return (up if long else down) / with_atr
