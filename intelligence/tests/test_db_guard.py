"""La guardia de `conftest.py` (#376): los tests marcados `pg` solo corren contra `cfds_test*`."""

from __future__ import annotations

from pathlib import Path

import pytest
from conftest import check_test_database, database_name


def test_reads_the_database_name_from_urls_and_libpq_strings() -> None:
    assert database_name("postgresql://u:p@127.0.0.1:5432/cfds_test?sslmode=disable") == (
        "cfds_test"
    )
    assert database_name("postgres://u@h/cfdstrader") == "cfdstrader"
    assert database_name("host=h dbname=cfds_test_ml user=u") == "cfds_test_ml"
    assert database_name("postgres://u@h:5432") is None
    assert database_name("postgres://u@h:5432/") is None
    assert database_name("host=h user=u") is None


@pytest.mark.parametrize(
    "url",
    [
        "postgres://cfds:x@127.0.0.1:5432/cfds_test",
        "postgres://cfds:x@127.0.0.1:5432/cfds_test_guard",
        "postgresql://cfds@h/cfds_test_ml?sslmode=disable",
    ],
)
def test_test_databases_pass(url: str) -> None:
    assert check_test_database(url) is None


@pytest.mark.parametrize(
    "url",
    [
        "postgres://cfds:x@127.0.0.1:5432/cfdstrader",
        "postgres://cfds:x@127.0.0.1:5432/cfdstrader_test",
        "postgres://cfds:x@127.0.0.1:5432/postgres",
        "postgres://cfds:x@127.0.0.1:5432",
    ],
)
def test_any_other_database_is_refused(url: str) -> None:
    error = check_test_database(url)
    assert error is not None and "cfds_test" in error


def test_the_real_database_error_says_why() -> None:
    error = check_test_database("postgres://u@h/cfdstrader")
    assert error is not None and error.startswith("los tests no escriben en la base real")


def test_a_pg_test_fails_against_the_real_database(
    pytester: pytest.Pytester, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Un test marcado `pg` falla sin correr si la base no es de pruebas, y se saltea sin base."""
    pytester.makeconftest((Path(__file__).parent / "conftest.py").read_text(encoding="utf-8"))
    pytester.makepyfile(
        """
        import pytest

        pytestmark = pytest.mark.pg

        def test_writes():
            raise AssertionError("no debería correr")
        """
    )
    monkeypatch.setenv("DATABASE_URL", "postgres://u:p@h:5432/cfdstrader")
    r = pytester.runpytest_inprocess()
    r.assert_outcomes(errors=1)
    r.stdout.fnmatch_lines(["*los tests no escriben en la base real*"])
    assert "no debería correr" not in r.stdout.str()
    monkeypatch.delenv("DATABASE_URL")
    pytester.runpytest_inprocess().assert_outcomes(skipped=1)
    monkeypatch.setenv("DATABASE_URL", "postgres://u:p@h:5432/cfds_test_x")
    pytester.runpytest_inprocess().assert_outcomes(failed=1)
