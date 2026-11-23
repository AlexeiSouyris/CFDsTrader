"""El informe de análogos como documento de `analog_report.v1` (#294). Solo da forma: los números
ya los calculó `search.analyze` (no se recalcula nada acá)."""

from collections.abc import Mapping, Sequence
from datetime import UTC, datetime
from decimal import Decimal
from typing import Any, Literal

from cfds_intel.analogs.config import AnalogsConfig
from cfds_intel.analogs.search import AnalogReport, Bar, Summary

Source = Literal["ig", "futures"]


def _dec(d: Decimal) -> str:
    """Decimal exacto como texto, sin exponente (`1E-5` no cumple el contrato)."""
    text = format(d, "f")
    return "0" if text in ("-0", "0") else text


def _utc(t: datetime) -> str:
    if t.tzinfo is None:
        raise ValueError("hora sin zona horaria")
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def _group(name: str, s: Summary) -> dict[str, Any]:
    return {
        "group": name,
        "count": s.count,
        "insufficient": s.insufficient,
        "horizons": [
            {
                "horizon": h.horizon,
                "percentiles": [
                    {"p": p, "return": _dec(v)} for p, v in sorted(h.percentiles.items())
                ],
                "positive_pct": _dec(h.positive_pct),
            }
            for h in s.horizons
        ],
        "hits": {k: s.hits[k] for k in ("up", "down", "none")},
        "without_atr": s.without_atr,
    }


def to_contract(
    report: AnalogReport,
    series: Mapping[str, Sequence[Bar]],
    source: Source,
    cfg: AnalogsConfig,
    computed_at: datetime,
) -> dict[str, Any]:
    """`report` como `analog_report.v1`. `series` es la misma con la que se calculó."""
    end_bar = next(b for b in series[report.instrument] if b.day == report.day)
    return {
        "instrument_id": report.instrument,
        "source": source,
        "window_end_day": report.day.isoformat(),
        "as_of": _utc(end_bar.end),
        "computed_at": _utc(computed_at),
        "window": cfg.window,
        "min_neighbors": cfg.min_neighbors,
        "context": {"vol": report.context.vol, "rates": report.context.rates},
        "neighbors": [
            {
                "instrument_id": n.instrument,
                "end_day": series[n.instrument][n.end_index].day.isoformat(),
                "distance": f"{n.distance:.6f}",
            }
            for n in report.neighbors
        ],
        "groups": [
            _group("all", report.all_neighbors),
            _group("same_context", report.same_context),
        ],
    }
