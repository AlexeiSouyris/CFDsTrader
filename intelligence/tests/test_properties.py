"""Tests de propiedades con `hypothesis` en funciones puras (#665, plan B1 de
`docs/analysis/2026-11-autoanalisis.md`).

- Hora del Este → UTC (calendario de la Fed y del BLS): el desfase sigue la regla de horario de
  verano de EE. UU. (desde 2007: del segundo domingo de marzo al primer domingo de noviembre),
  calculada acá de forma independiente, y la conversión vuelve a la misma hora local.
- Fechas del BLS: `parse_day` lee cualquier fecha bien escrita (con o sin cero adelante) y
  rechaza un día de la semana equivocado; los meses de referencia avanzan bien.
- Horas de publicación de series diarias (Tesoro, VIX, BCE): nunca antes del día siguiente,
  en orden, y el último valor vale desde que se leyó (no mirar el futuro).
- Períodos de Databento: los meses y años parten `[inicio, fin)` sin huecos ni solapes.

Sin red ni base de datos. Pocos ejemplos para que el CI no tarde.
"""

import itertools
from dataclasses import dataclass
from datetime import UTC, date, datetime, time, timedelta
from decimal import Decimal
from zoneinfo import ZoneInfo

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

from cfds_intel.calendar.official.bls import _months_between, _next_ref, parse_day
from cfds_intel.calendar.official.fed import eastern
from cfds_intel.calendar.official.model import OfficialCalendarError
from cfds_intel.daily_publication import published_times
from cfds_intel.databento.bars import months, years
from cfds_intel.rates.ecb import RatesError, to_rates

NY = ZoneInfo("America/New_York")
FAST = settings(max_examples=150, deadline=None)

days_2007_on = st.dates(min_value=date(2007, 1, 1), max_value=date(2040, 12, 31))
# Horas que existen todos los días: el salto del cambio de hora es de 2:00 a 3:00.
release_times = st.builds(
    time,
    st.sampled_from([0, 1, *range(3, 24)]),
    st.integers(0, 59),
)


def _nth_sunday(year: int, month: int, n: int) -> date:
    first = date(year, month, 1)
    first_sunday = first + timedelta(days=(6 - first.weekday()) % 7)
    return first_sunday + timedelta(weeks=n - 1)


def _us_summer_time(day: date) -> bool:
    """Regla de EE. UU. desde 2007 (Energy Policy Act de 2005), sin usar `zoneinfo`."""
    return _nth_sunday(day.year, 3, 2) < day < _nth_sunday(day.year, 11, 1)


@FAST
@given(day=days_2007_on, at=release_times)
def test_eastern_time_follows_the_us_daylight_saving_rule(day: date, at: time) -> None:
    utc = eastern(day, at, NY)
    assert utc.tzinfo == UTC
    local = datetime.combine(day, at)
    offset = local - utc.replace(tzinfo=None)
    if day in (_nth_sunday(day.year, 3, 2), _nth_sunday(day.year, 11, 1)):
        # Día del cambio: antes o después de la hora del salto.
        assert offset in (timedelta(hours=-4), timedelta(hours=-5))
    else:
        assert offset == timedelta(hours=-4 if _us_summer_time(day) else -5)
    # Vuelve a la misma hora local.
    back = utc.astimezone(NY)
    assert (back.date(), back.time()) == (day, at)


WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]
MONTH_NAMES = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
]


@FAST
@given(day=st.dates(min_value=date(2000, 1, 1), max_value=date(2040, 12, 31)), padded=st.booleans())
def test_bls_dates_round_trip(day: date, padded: bool) -> None:
    d = f"{day.day:02d}" if padded else str(day.day)
    text = f"{WEEKDAYS[day.weekday()]}, {MONTH_NAMES[day.month - 1]} {d}, {day.year}"
    assert parse_day(text, "test") == day


@FAST
@given(
    day=st.dates(min_value=date(2000, 1, 1), max_value=date(2040, 12, 31)), shift=st.integers(1, 6)
)
def test_bls_dates_reject_the_wrong_weekday(day: date, shift: int) -> None:
    wrong = WEEKDAYS[(day.weekday() + shift) % 7]
    text = f"{wrong}, {MONTH_NAMES[day.month - 1]} {day.day}, {day.year}"
    with pytest.raises(OfficialCalendarError):
        parse_day(text, "test")


@FAST
@given(year=st.integers(1990, 2040), month=st.integers(1, 12), day=days_2007_on)
def test_next_reference_month_is_one_month_later(year: int, month: int, day: date) -> None:
    ref = f"{year:04d}-{month:02d}"
    nxt = _next_ref(ref)
    assert _months_between(nxt, day) == _months_between(ref, day) - 1
    assert (int(nxt[:4]), int(nxt[5:])) == ((year + 1, 1) if month == 12 else (year, month + 1))


@st.composite
def business_days(draw: st.DrawFn) -> list[date]:
    start = draw(st.dates(min_value=date(2010, 1, 1), max_value=date(2035, 1, 1)))
    gaps = draw(st.lists(st.integers(1, 5), min_size=0, max_size=30))
    out = [start]
    for g in gaps:
        out.append(out[-1] + timedelta(days=g))
    return out


@FAST
@given(days=business_days(), at=release_times, late_hours=st.integers(1, 72))
def test_daily_values_are_never_published_before_the_next_day(
    days: list[date], at: time, late_hours: int
) -> None:
    retrieved = datetime.combine(days[-1] + timedelta(days=1), time(), NY) + timedelta(
        hours=late_hours
    )
    out = published_times(days, at, NY, retrieved, time(0, 0), NY)
    assert len(out) == len(days)
    assert out == sorted(out)
    assert out[-1] == retrieved.astimezone(UTC)
    for day, published in zip(days[:-1], out[:-1], strict=True):
        assert published.tzinfo == UTC
        assert published >= datetime.combine(day + timedelta(days=1), time(), NY).astimezone(UTC)


@dataclass(frozen=True)
class _Series:
    name: str
    publication_time: time
    publication_tz: ZoneInfo


@FAST
@given(days=business_days(), at=release_times, read_after=st.integers(0, 72))
def test_rates_are_never_published_before_their_day_ends(
    days: list[date], at: time, read_after: int
) -> None:
    tz = ZoneInfo("Europe/Berlin")
    values = [(d, Decimal("3.25")) for d in days]
    retrieved = datetime.combine(days[-1], time(), tz) + timedelta(hours=read_after)
    s = _Series("estr", at, tz)
    end_of_last = datetime.combine(days[-1] + timedelta(days=1), time(), tz)
    if retrieved < end_of_last:
        # Leído antes de que terminara el último día: nunca se guarda.
        with pytest.raises(RatesError):
            to_rates(values, s, retrieved)
        return
    rates = to_rates(values, s, retrieved)
    assert [r.published_at for r in rates] == sorted(r.published_at for r in rates)
    for (day, _), r in zip(values, rates, strict=True):
        assert r.published_at >= datetime.combine(day + timedelta(days=1), time(), tz)


@FAST
@given(
    start=st.dates(min_value=date(2000, 1, 1), max_value=date(2040, 1, 1)),
    length=st.integers(0, 2000),
)
def test_months_and_years_partition_the_period(start: date, length: int) -> None:
    end = start + timedelta(days=length)
    for parts, same_period in (
        (months(start, end), lambda a, b: (a.year, a.month) == (b.year, b.month)),
        (years(start, end), lambda a, b: a.year == b.year),
    ):
        if length == 0:
            assert parts == []
            continue
        assert parts[0][0] == start
        assert parts[-1][1] == end
        for (_, b), (c, _) in itertools.pairwise(parts):
            assert b == c
        for a, b in parts:
            assert a < b
            assert same_period(a, b - timedelta(days=1))
