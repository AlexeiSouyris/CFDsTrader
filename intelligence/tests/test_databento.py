"""Importador de Databento (ADR 0011, #247) con un cliente falso y respuestas sintéticas: sin red,
sin key y sin gastar."""

import json
from collections.abc import Iterable, Mapping
from dataclasses import replace
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.databento import __main__ as cli
from cfds_intel.databento.bars import (
    BudgetError,
    DatabentoError,
    check_budget,
    estimate,
    fetch_bars,
    months,
    to_bar,
    validate_day,
)
from cfds_intel.databento.config import ConfigError, load_config

ROOT = Path(__file__).resolve().parents[2]
TEXT = (ROOT / "config" / "databento.toml").read_text()
CFG = load_config(TEXT)
ES = CFG.symbols[0]
NS = 1_000_000_000
DAY = date(2026, 11, 18)


def ts(t: str) -> int:
    return int(datetime.fromisoformat(t).replace(tzinfo=UTC).timestamp()) * NS


def rec(t: str, o: int = 5_000_250_000_000, contract: int = 42, **over: Any) -> SimpleNamespace:
    """Registro OHLCV como los del SDK: precios enteros con escala 1e-9."""
    base = {
        "ts_event": ts(t),
        "instrument_id": contract,
        "open": o,
        "high": o + 500_000_000,
        "low": o - 250_000_000,
        "close": o + 250_000_000,
        "volume": 17,
    }
    return SimpleNamespace(**{**base, **over})


class FakeMetadata:
    def __init__(self, ranges: Mapping[str, Mapping[str, Any]], cost: float = 1.25) -> None:
        self.ranges, self.cost, self.calls = ranges, cost, list[tuple[str, dict[str, Any]]]()

    def get_cost(self, **kw: Any) -> float:
        self.calls.append(("get_cost", kw))
        return self.cost

    def get_record_count(self, **kw: Any) -> int:
        self.calls.append(("get_record_count", kw))
        return 1000

    def get_dataset_range(self, dataset: str) -> Mapping[str, Any]:
        self.calls.append(("get_dataset_range", {"dataset": dataset}))
        return self.ranges[dataset]


class FakeTimeseries:
    def __init__(self, records: Iterable[SimpleNamespace]) -> None:
        self.records, self.calls = list(records), list[dict[str, Any]]()

    def get_range(self, **kw: Any) -> Iterable[Any]:
        self.calls.append(kw)
        return iter(self.records)


class FakeClient:
    def __init__(self, cost: float = 1.25, records: Iterable[SimpleNamespace] = ()) -> None:
        # Respuesta sintética con la forma documentada de get_dataset_range.
        self._metadata = FakeMetadata(
            {
                "GLBX.MDP3": {
                    "start": "2010-06-06T00:00:00.000000000Z",
                    "end": "2026-11-23T00:00:00.000000000Z",
                    "schema": {"ohlcv-1m": {"start": "2010-06-06T00:00:00.000000000Z"}},
                },
                "XEUR.EOBI": {
                    "start": "2025-03-10T00:00:00.000000000Z",
                    "end": "2026-11-23T00:00:00.000000000Z",
                    "schema": {"ohlcv-1m": {"start": "2025-03-10T00:00:00.000000000Z"}},
                },
            },
            cost,
        )
        self._timeseries = FakeTimeseries(records)

    @property
    def metadata(self) -> FakeMetadata:
        return self._metadata

    @property
    def timeseries(self) -> FakeTimeseries:
        return self._timeseries


def test_the_repository_config_maps_the_four_futures() -> None:
    assert [(s.dataset, s.symbol, s.instrument_id) for s in CFG.symbols] == [
        ("GLBX.MDP3", "ES.c.0", "US500"),
        ("GLBX.MDP3", "NQ.c.0", "US100"),
        ("GLBX.MDP3", "YM.c.0", "US30"),
        ("XEUR.EOBI", "FDAX.c.0", "DE40"),
    ]
    assert (CFG.start, CFG.max_usd) == (date(2021, 11, 1), Decimal(100))
    assert (CFG.schema, CFG.stype_in) == ("ohlcv-1m", "continuous")


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('max_usd = "100"', "max_usd = 100"),
        ('max_usd = "100"', 'max_usd = "-1"'),
        ('symbol = "ES.c.0"', 'symbol = "ESZ6"'),
        ('instrument_id = "US30"', 'instrument_id = "US500"'),
        ('start = "2021-11-01"', 'start = "1/11/2021"'),
        ('start = "2021-11-01"', 'start = "2021-11-01"\nmin_start = "2023-08-01"'),
        ('schema = "ohlcv-1m"', 'schema = "trades"'),
        ("min_session_hours = 20", "min_session_hours = 25"),
        ("max_gap_minutes = 5", "max_gap_minutes = 0"),
    ],
)
def test_bad_config_is_rejected(old: str, new: str) -> None:
    assert old in TEXT
    with pytest.raises(ConfigError):
        load_config(TEXT.replace(old, new, 1))


def test_estimate_uses_only_free_metadata_and_respects_dataset_starts() -> None:
    c = FakeClient(cost=1.25)
    est = estimate(c, CFG, date(2026, 11, 23), {})
    assert c.timeseries.calls == [], "estimar nunca baja datos"
    assert {n for n, _ in c.metadata.calls} == {
        "get_dataset_range",
        "get_cost",
        "get_record_count",
    }
    es = est[0]
    assert (es.start, es.end, es.cost_usd, es.records, es.skipped) == (
        date(2021, 11, 1),
        date(2026, 11, 23),
        Decimal("1.25"),
        1000,
        None,
    )
    # Eurex empieza después de `start`: se baja desde el inicio que da get_dataset_range.
    assert (est[3].start, est[3].skipped) == (date(2025, 3, 10), None)
    kw = next(k for n, k in c.metadata.calls if n == "get_cost")
    assert kw == {
        "dataset": "GLBX.MDP3",
        "symbols": ["ES.c.0"],
        "schema": "ohlcv-1m",
        "stype_in": "continuous",
        "start": "2021-11-01",
        "end": "2026-11-23",
    }
    assert check_budget(est, CFG.max_usd) == Decimal("5.00")


def test_the_dataset_start_comes_from_metadata_not_from_the_code() -> None:
    c = FakeClient()
    c.metadata.ranges = {**c.metadata.ranges, "XEUR.EOBI": {"start": "2025-06-02T00:00:00Z"}}
    est = estimate(c, CFG, date(2026, 11, 23), {})
    assert est[3].start == date(2025, 6, 2)
    # Sin el esquema en el dataset, no se adivina.
    c.metadata.ranges = {**c.metadata.ranges, "XEUR.EOBI": {"schema": {"trades": {}}}}
    with pytest.raises(DatabentoError):
        estimate(c, CFG, date(2026, 11, 23), {})


def test_estimate_continues_from_what_is_already_stored() -> None:
    loaded = {
        ("GLBX.MDP3", "ES.c.0"): date(2026, 11, 23),
        ("GLBX.MDP3", "NQ.c.0"): date(2025, 1, 1),
    }
    est = estimate(FakeClient(), CFG, date(2026, 11, 23), loaded)
    assert est[0].skipped == "ya está todo guardado"
    assert est[1].start == date(2025, 1, 1)


def test_the_budget_stops_everything() -> None:
    c = FakeClient(cost=30.0)
    est = estimate(c, CFG, date(2026, 11, 23), {})
    with pytest.raises(BudgetError):
        check_budget(est, CFG.max_usd)  # 4 x 30 = 120 > 100


def test_cli_estimate_stops_over_budget_and_never_prints_the_key(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    key = "db-" + "k" * 29
    monkeypatch.setenv("DATABENTO_API_KEY", key)
    monkeypatch.delenv("DATABASE_URL", raising=False)
    monkeypatch.setattr(cli, "client", lambda: FakeClient(cost=30.0))
    assert cli.main(["x", "--estimate"]) == 2
    out = capsys.readouterr()
    assert "USD 30.00" in out.out and "no se baja nada" in out.err
    assert key not in out.out + out.err
    monkeypatch.setattr(cli, "client", lambda: FakeClient(cost=2.0))
    assert cli.main(["x", "--estimate"]) == 0
    assert "total estimado: USD 8.00" in capsys.readouterr().out


def test_cli_without_key_or_off_the_data_machine_does_nothing(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.delenv("DATABENTO_API_KEY", raising=False)
    assert cli.main(["x", "--estimate"]) == 1
    assert "falta DATABENTO_API_KEY" in capsys.readouterr().err
    monkeypatch.setattr(cli, "client", lambda: FakeClient())
    monkeypatch.delenv("CFDS_DATA_MACHINE", raising=False)
    assert cli.main(["x", "--run"]) == 2
    assert cli.main(["x"]) == 2


def test_records_become_exact_decimal_bars_in_utc_following_the_contract() -> None:
    b = to_bar(rec("2026-11-18T14:31:00"), ES)
    assert b.start == datetime(2026, 11, 18, 14, 31, tzinfo=UTC)
    assert (b.open, b.high, b.low, b.close) == (
        Decimal("5000.25"),
        Decimal("5000.75"),
        Decimal("5000"),
        Decimal("5000.5"),
    )
    doc = json.loads((ROOT / "contracts" / "futures_bar.v1.schema.json").read_text())
    v = Draft202012Validator(doc)
    d = b.to_contract()
    assert not list(v.iter_errors(d)), d
    assert d["start"] == "2026-11-18T14:31:00Z"
    assert d["instrument_id"] == "US500" and d["contract_id"] == 42


@pytest.mark.parametrize(
    "bad",
    [
        rec("2026-11-18T14:31:00", ts_event=ts("2026-11-18T14:31:00") + 1),  # no es un minuto
        rec("2026-11-18T14:31:00", open=9_223_372_036_854_775_807),  # UNDEF_PRICE
        rec("2026-11-18T14:31:00", low=0),
        rec("2026-11-18T14:31:00", high=1),  # OHLC incoherente
        rec("2026-11-18T14:31:00", volume=-1),
        rec("2026-11-18T14:31:00", contract=0),
        rec("2026-11-18T14:31:00", open=5000.25),  # float: nunca
    ],
)
def test_bad_records_fail(bad: SimpleNamespace) -> None:
    with pytest.raises(DatabentoError):
        to_bar(bad, ES)


def test_fetch_checks_order_and_period() -> None:
    good = [rec("2026-11-18T14:31:00"), rec("2026-11-18T14:32:00")]
    c = FakeClient(records=good)
    bars = fetch_bars(c, CFG, ES, DAY, date(2026, 11, 19))
    assert len(bars) == 2
    assert c.timeseries.calls[0]["symbols"] == ["ES.c.0"]
    for wrong in [list(reversed(good)), [good[0], good[0]], [rec("2026-11-19T00:00:00")]]:
        with pytest.raises(DatabentoError):
            fetch_bars(FakeClient(records=wrong), CFG, ES, DAY, date(2026, 11, 19))


def day_of_cme(skip: set[str] = frozenset()) -> list[SimpleNamespace]:  # type: ignore[assignment]
    """Un día UTC de CME sintético: 00:00-21:00 y 22:00-24:00 (pausa de 21 a 22)."""
    out = []
    for m in range(24 * 60):
        h, mi = divmod(m, 60)
        t = f"2026-11-18T{h:02d}:{mi:02d}:00"
        if h != 21 and t not in skip:
            out.append(rec(t))
    return out


def test_a_full_cme_day_passes_the_sample_validation() -> None:
    bars = fetch_bars(FakeClient(records=day_of_cme()), CFG, ES, DAY, date(2026, 11, 19))
    r = validate_day(bars, ES, DAY, CFG.max_gap_minutes)
    assert r.problems == []
    assert (r.bars, r.hours, r.longest_gap_minutes, r.contracts) == (1380, Decimal(23), 60, [42])


def test_the_sample_validation_reports_gaps_and_short_sessions() -> None:
    hole = {f"2026-11-18T10:{m:02d}:00" for m in range(10)}  # 10 min sin barras
    bars = fetch_bars(FakeClient(records=day_of_cme(hole)), CFG, ES, DAY, date(2026, 11, 19))
    r = validate_day(bars, ES, DAY, CFG.max_gap_minutes)
    assert r.gaps == [(datetime(2026, 11, 18, 9, 59, tzinfo=UTC), 10)]
    assert any("huecos" in p for p in r.problems)
    short = validate_day(bars[:600], replace(ES, min_session_hours=22), DAY, 5)
    assert any("h con barras" in p for p in short.problems)


def test_months_split_the_period_for_resumable_loads() -> None:
    assert months(date(2021, 11, 1), date(2022, 1, 15)) == [
        (date(2021, 11, 1), date(2021, 12, 1)),
        (date(2021, 12, 1), date(2022, 1, 1)),
        (date(2022, 1, 1), date(2022, 1, 15)),
    ]
    assert months(date(2022, 1, 1), date(2022, 1, 1)) == []
