"""Tipos de cambio del BCE (ADR 0013 §4, #418): sin red ni base."""

from dataclasses import replace
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path

import pytest

from cfds_intel.rates.ecb import RatesError, SeriesConfig, load_config, parse_csv
from cfds_intel.rates.fx import CONFIG, to_fx_rates

CSV = (
    "KEY,FREQ,CURRENCY,CURRENCY_DENOM,EXR_TYPE,EXR_SUFFIX,TIME_PERIOD,OBS_VALUE\n"
    "EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-20,1.0512\n"
    "EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-23,1.0498\n"
)


def series() -> SeriesConfig:
    return load_config(Path(CONFIG).read_text()).series["EURUSD"]


def test_the_repository_config_reads_eurusd_from_the_ecb() -> None:
    s = series()
    assert (s.flow, s.key) == ("EXR", "D.USD.EUR.SP00.A")


def test_each_rate_is_published_the_same_day_at_the_configured_hour() -> None:
    s = series()
    rates = to_fx_rates(parse_csv(CSV, s), s)
    assert [r.rate for r in rates] == [Decimal("1.0512"), Decimal("1.0498")]
    # 17:00 de Berlín en invierno (CET, UTC+1) = 16:00 UTC; nunca antes.
    assert rates[0].published_at == datetime(2026, 11, 20, 16, 0, tzinfo=UTC)
    assert rates[1].to_contract() == {
        "pair": "EURUSD",
        "ref_date": "2026-11-23",
        "published_at": "2026-11-23T16:00:00Z",
        "rate": "1.0498",
    }


def test_summer_time_moves_the_publication_in_utc() -> None:
    s = series()
    [r] = to_fx_rates([(date(2026, 7, 8), Decimal("1.1"))], s)
    assert r.published_at == datetime(2026, 7, 8, 15, 0, tzinfo=UTC)


def test_non_positive_rates_and_bad_pairs_are_errors() -> None:
    s = series()
    with pytest.raises(RatesError):
        to_fx_rates([(date(2026, 11, 20), Decimal(0))], s)
    with pytest.raises(RatesError):
        to_fx_rates([(date(2026, 11, 20), Decimal("1.1"))], replace(s, name="ESTR"))


def test_a_row_of_another_series_rejects_everything() -> None:
    bad = CSV.replace(
        "EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-23",
        "EXR.D.GBP.EUR.SP00.A,D,GBP,EUR,SP00,A,2026-11-23",
    )
    with pytest.raises(RatesError):
        parse_csv(bad, series())
