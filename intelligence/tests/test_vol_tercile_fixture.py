"""El tercio de volatilidad de los análogos (spec §13) coincide con el que arma Rust para
`ml_features.v1` (#158): el mismo fixture lo prueban los dos lenguajes."""

import json
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from pathlib import Path

from cfds_intel.analogs.search import Bar, vol_band

FIXTURE = Path(__file__).parent / "fixtures" / "vol_tercile.json"
NAMES = {"baja": "low", "media": "mid", "alta": "high"}


def test_vol_band_matches_the_fixture_shared_with_rust() -> None:
    doc = json.loads(FIXTURE.read_text())
    day = date.fromisoformat(doc["start"])
    bars: list[Bar] = []
    for v in doc["realized_vol"]:
        while day.weekday() >= 5:
            day += timedelta(days=1)
        end = datetime(day.year, day.month, day.day, doc["published_hour_utc"], tzinfo=UTC)
        vol = None if v is None else Decimal(v)
        one = Decimal(1)
        bars.append(Bar(day, end, one, one, one, None, vol))
        day += timedelta(days=1)
    for q in doc["queries"]:
        t = datetime.fromisoformat(q["as_of"].replace("Z", "+00:00"))
        ends = [i for i, b in enumerate(bars) if b.end <= t]
        band = vol_band(bars, ends[-1], doc["years"]) if ends else None
        assert (NAMES[band] if band else None) == q["expected"], q
