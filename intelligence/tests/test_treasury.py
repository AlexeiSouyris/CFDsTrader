"""Curva par del Tesoro (ADR 0012) con respuestas sintéticas: los números no son reales."""

import json
import urllib.request
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.treasury import ust

ROOT = Path(__file__).resolve().parents[2]
CFG = ust.load_config((ROOT / "config" / "treasury.toml").read_text())

# Como lo publica el Tesoro: de la más nueva a la más vieja, con una celda vacía.
Y2025 = (
    'Date,"1 Mo","1.5 Month","2 Mo","3 Mo","4 Mo","6 Mo","1 Yr","2 Yr","3 Yr","5 Yr","7 Yr",'
    '"10 Yr","20 Yr","30 Yr"\n'
    "11/21/2025,1.10,1.15,1.20,1.30,1.40,1.50,1.60,1.70,1.80,1.90,2.00,2.10,2.20,2.30\n"
    "11/20/2025,1.11,,1.21,1.31,1.41,1.51,1.61,1.71,1.81,1.91,2.01,2.11,2.21,2.31\n"
)
Y2019 = (
    'Date,"1 Mo","2 Mo","3 Mo","6 Mo","1 Yr","2 Yr","3 Yr","5 Yr","7 Yr","10 Yr","20 Yr",'
    '"30 Yr"\n'
    "12/31/2019,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0,1.0\n"
)
Y2020 = Y2019.replace("12/31/2019", "01/02/2020").replace("1.0", "2.0")


def test_repo_config_is_valid() -> None:
    assert CFG.start_year == 2019
    assert CFG.csv_url.startswith("https://home.treasury.gov/")
    assert str(CFG.publication_tz) == "America/New_York"


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ("https://home.treasury.gov/", "http://home.treasury.gov/"),
        ("https://home.treasury.gov/", "https://otro.example/"),
        ('time = "09:30"', 'time = "nunca"'),
        ("start_year = 2019", "start_year = 1800"),
    ],
)
def test_bad_config_is_rejected(old: str, new: str) -> None:
    text = (ROOT / "config" / "treasury.toml").read_text()
    assert old in text
    with pytest.raises(ust.TreasuryError):
        ust.load_config(text.replace(old, new, 1))


def test_parse_orders_ascending_and_skips_empty_cells() -> None:
    rows = ust.parse_csv(Y2025)
    assert [d for d, _ in rows] == [date(2025, 11, 20), date(2025, 11, 21)]
    first, second = rows[0][1], rows[1][1]
    assert "1_5M" not in first
    assert second["1_5M"] == Decimal("1.15")
    assert first["2Y"] == Decimal("1.71")
    assert first["10Y"] == Decimal("2.11")
    assert len(second) == 14


def test_older_files_without_some_tenors_parse() -> None:
    rows = ust.parse_csv(Y2019)
    assert set(rows[0][1]) == {
        "1M", "2M", "3M", "6M", "1Y", "2Y", "3Y", "5Y", "7Y", "10Y", "20Y", "30Y"
    }  # fmt: skip


@pytest.mark.parametrize(
    ("text", "match"),
    [
        (Y2019.replace('"30 Yr"', '"40 Yr"'), "columnas desconocidas"),
        (Y2019.replace("Date", "Fecha"), "encabezado"),
        (Y2019.replace("12/31/2019", "2019-12-31"), "fecha inválida"),
        (Y2019.replace("12/31/2019,1.0", "12/31/2019,uno"), "no es un número"),
        (Y2019.replace("12/31/2019,1.0,", "12/31/2019,"), "celdas"),
        (Y2019 + Y2019.splitlines()[1] + "\n", "repetida"),
    ],
)
def test_odd_responses_are_rejected(text: str, match: str) -> None:
    with pytest.raises(ust.TreasuryError, match=match):
        ust.parse_csv(text)


def test_published_at_is_next_business_day_0930_et_or_read_time() -> None:
    rows = ust.parse_csv(Y2025)
    read = datetime(2025, 11, 21, 23, 40, tzinfo=UTC)
    out = ust.to_yields(rows, CFG, read)
    by_day = {(y.ref_date, y.tenor): y.published_at for y in out}
    # 20-11: vale desde el día hábil siguiente (21-11) a las 09:30 ET = 14:30 UTC (EST).
    assert by_day[(date(2025, 11, 20), "2Y")] == datetime(2025, 11, 21, 14, 30, tzinfo=UTC)
    # el último: desde que se leyó
    assert by_day[(date(2025, 11, 21), "2Y")] == read
    assert len(out) == 13 + 14


def test_never_before_the_fed_takes_the_prices() -> None:
    rows = ust.parse_csv(Y2025)
    too_early = datetime(2025, 11, 21, 15, 0, tzinfo=UTC)  # 10:00 ET del día del último valor
    with pytest.raises(ust.TreasuryError, match="antes"):
        ust.to_yields(rows, CFG, too_early)
    with pytest.raises(ust.TreasuryError):
        ust.to_yields(rows, CFG, datetime(2025, 11, 22))  # sin zona horaria


def test_yields_match_the_contract() -> None:
    schema = json.loads((ROOT / "contracts" / "treasury_yield.v1.schema.json").read_text())
    v = Draft202012Validator(schema)
    out = ust.to_yields(ust.parse_csv(Y2025), CFG, datetime(2025, 11, 22, tzinfo=UTC))
    for y in out:
        assert not list(v.iter_errors(y.to_contract())), y
    assert out[0].to_contract()["published_at"].endswith("Z")


def test_fetch_asks_each_year_and_links_december_to_january() -> None:
    seen: list[tuple[str, str | None, int]] = []
    files = {"2019": Y2019, "2020": Y2020}

    def opener(req: urllib.request.Request, timeout: int) -> bytes:
        seen.append((req.full_url, req.get_header("User-agent"), timeout))
        year = req.full_url.split("daily-treasury-rates.csv/")[1][:4]
        return files[year].encode()

    rows = ust.fetch_rows(CFG, 2019, 2020, opener)
    assert [d for d, _ in rows] == [date(2019, 12, 31), date(2020, 1, 2)]
    assert seen[0][0] == CFG.csv_url.replace("{year}", "2019")
    assert "field_tdr_date_value=2019" in seen[0][0]
    assert seen[0][1] == CFG.user_agent
    assert seen[0][2] == CFG.timeout_secs
    out = ust.to_yields(rows, CFG, datetime(2020, 1, 3, tzinfo=UTC))
    # el 31-12 vale desde el 2-1 (siguiente fila, del archivo del año siguiente) a las 09:30 ET
    dec31 = next(y for y in out if y.ref_date == date(2019, 12, 31))
    assert dec31.published_at == datetime(2020, 1, 2, 14, 30, tzinfo=UTC)
