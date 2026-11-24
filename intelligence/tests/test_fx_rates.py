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


FIXTURES = Path(__file__).resolve().parent / "fixtures"


def test_ecb_holidays_with_an_empty_value_are_skipped_not_invented() -> None:
    # CSV real del BCE (#447), del 20 al 27-04-2011: Viernes Santo (22-04) y Lunes de Pascua
    # (25-04) vienen con OBS_VALUE vacío y OBS_STATUS H. Son días sin publicación.
    s = series()
    real = (FIXTURES / "ecb_eurusd_2011-04-20_27.csv").read_text()
    values = parse_csv(real, s)
    assert values == [
        (date(2011, 4, 20), Decimal("1.4515")),
        (date(2011, 4, 21), Decimal("1.4584")),
        (date(2011, 4, 26), Decimal("1.4617")),
        (date(2011, 4, 27), Decimal("1.4668")),
    ]
    # Cada tipo sigue publicado su propio día: el feriado no adelanta ni corre nada.
    rates = to_fx_rates(values, s)
    assert rates[2].published_at == datetime(2011, 4, 26, 15, 0, tzinfo=UTC)


def test_only_an_empty_value_means_no_publication() -> None:
    s = series()
    for bad in [" ", "NaN", "-", "n/a"]:
        with pytest.raises(RatesError):
            parse_csv(CSV.replace(",2026-11-20,1.0512", f",2026-11-20,{bad}"), s)
    # Un feriado fuera de orden sigue siendo un error, aunque no tenga valor.
    disordered = CSV + "EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-11-21,\n"
    with pytest.raises(RatesError):
        parse_csv(disordered, s)
    # Solo feriados: no hay nada que guardar.
    empty = CSV.replace(",1.0512", ",").replace(",1.0498", ",")
    assert parse_csv(empty, s) == []
