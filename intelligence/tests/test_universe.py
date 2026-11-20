from datetime import date

import pytest

from cfds_intel.universe import (
    COVERAGE_END,
    COVERAGE_START,
    OutsideCoverageError,
    members_between,
    members_on,
)

START = date(2021, 1, 4)


def test_the_index_has_about_a_hundred_members() -> None:
    # Son 100 empresas, pero algunas tienen dos clases de acciones (por ejemplo GOOG y GOOGL).
    for day in (START, COVERAGE_END):
        assert 100 <= len(members_on(day)) <= 103
        assert "AAPL" in members_on(day)


def test_universe_includes_companies_that_left_the_index() -> None:
    universe = members_between(START, COVERAGE_END)
    today = members_on(COVERAGE_END)
    # Activision (ATVI) salió del índice cuando la compró Microsoft (2023): hoy no está,
    # pero tiene que estar en el universo para no tener sesgo de supervivencia.
    assert "ATVI" in members_on(START)
    assert "ATVI" not in today
    assert "ATVI" in universe
    assert universe > today
    assert universe >= members_on(START) | today


def test_dates_outside_the_source_coverage_are_never_guessed() -> None:
    with pytest.raises(OutsideCoverageError):
        members_on(date(2002, 12, 31))
    with pytest.raises(OutsideCoverageError):
        members_on(date(2026, 11, 18))
    with pytest.raises(OutsideCoverageError):
        members_between(COVERAGE_START, date(2030, 1, 1))


def test_an_inverted_range_is_an_error() -> None:
    with pytest.raises(ValueError):
        members_between(COVERAGE_END, START)
