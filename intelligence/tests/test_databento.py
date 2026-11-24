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
    BACKWARD,
    BAR_NS,
    FORWARD,
    BudgetError,
    DatabentoError,
    Stored,
    check_budget,
    check_spend,
    chunks,
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
        "publisher_id": 1,  # GLBX.MDP3.GLBX
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


def test_the_repository_config_maps_the_futures() -> None:
    assert [(s.dataset, s.symbol, s.instrument_id) for s in CFG.symbols] == [
        ("GLBX.MDP3", "ES.c.0", "US500"),
        ("GLBX.MDP3", "NQ.c.0", "US100"),
        ("GLBX.MDP3", "YM.c.0", "US30"),
        ("XEUR.EOBI", "FDAX.c.0", "DE40"),
        # Solo para el estudio (#371).
        ("GLBX.MDP3", "RTY.c.0", "US2000"),
        ("GLBX.MDP3", "GC.v.0", "XAUUSD"),
    ]
    assert (CFG.start, CFG.max_usd) == (date(2011, 1, 1), Decimal(100))
    assert (CFG.schema, CFG.stype_in) == ("ohlcv-1m", "continuous")


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('max_usd = "100"', "max_usd = 100"),
        ('max_usd = "100"', 'max_usd = "-1"'),
        ('symbol = "ES.c.0"', 'symbol = "ESZ6"'),
        ('instrument_id = "US30"', 'instrument_id = "US500"'),
        ('start = "2011-01-01"', 'start = "1/1/2011"'),
        ('start = "2011-01-01"', 'start = "2011-01-01"\nmin_start = "2023-08-01"'),
        ('schema = "ohlcv-1m"', 'schema = "trades"'),
        ("min_session_hours = 18", "min_session_hours = 25"),
        ("max_gap_minutes = 5", "max_gap_minutes = 0"),
        # El inicio propio de un símbolo (#410) es una fecha y solo puede ser posterior al global.
        ('start = "2017-07-09"', 'start = "9/7/2017"'),
        ('start = "2017-07-09"', 'start = "2010-01-01"'),
        ('start = "2017-07-09"', 'start = "2011-01-01"'),
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
        date(2011, 1, 1),
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
        "start": "2011-01-01",
        "end": "2026-11-23",
    }
    assert check_budget(est, CFG.max_usd) == Decimal("7.50")


def test_the_dataset_start_comes_from_metadata_not_from_the_code() -> None:
    c = FakeClient()
    c.metadata.ranges = {
        **c.metadata.ranges,
        "XEUR.EOBI": {"start": "2025-06-02T00:00:00Z", "end": "2026-11-23T00:00:00Z"},
    }
    est = estimate(c, CFG, date(2026, 11, 23), {})
    assert est[3].start == date(2025, 6, 2)
    # Sin el fin publicado, tampoco se adivina (#396).
    c.metadata.ranges = {**c.metadata.ranges, "XEUR.EOBI": {"start": "2025-06-02T00:00:00Z"}}
    with pytest.raises(DatabentoError, match="sin end"):
        estimate(c, CFG, date(2026, 11, 23), {})
    # Sin el esquema en el dataset, no se adivina.
    c.metadata.ranges = {**c.metadata.ranges, "XEUR.EOBI": {"schema": {"trades": {}}}}
    with pytest.raises(DatabentoError):
        estimate(c, CFG, date(2026, 11, 23), {})


def test_estimate_continues_from_what_is_already_stored() -> None:
    loaded = {
        ("GLBX.MDP3", "ES.c.0"): Stored(date(2011, 1, 1), date(2026, 11, 23)),
        ("GLBX.MDP3", "NQ.c.0"): Stored(date(2011, 1, 1), date(2025, 1, 1)),
    }
    est = estimate(FakeClient(), CFG, date(2026, 11, 23), loaded)
    assert est[0].skipped == "ya está todo guardado"
    assert (est[1].part, est[1].start, est[1].end) == (
        FORWARD,
        date(2025, 1, 1),
        date(2026, 11, 23),
    )


def test_estimate_also_fills_the_part_before_what_is_stored() -> None:
    # Lo bajado con el `start` anterior (2021-11-01) hasta el 2026-11-23; FDAX desde su inicio.
    loaded = {
        ("GLBX.MDP3", "ES.c.0"): Stored(date(2021, 11, 1), date(2026, 11, 23)),
        ("GLBX.MDP3", "NQ.c.0"): Stored(date(2021, 11, 1), date(2026, 11, 20)),
        ("XEUR.EOBI", "FDAX.c.0"): Stored(date(2025, 3, 10), date(2026, 11, 23)),
    }
    c = FakeClient(cost=1.25)
    est = estimate(c, CFG, date(2026, 11, 23), loaded)
    got = [(e.symbol.symbol, e.part, e.start, e.end, e.skipped) for e in est]
    assert got == [
        ("ES.c.0", BACKWARD, date(2011, 1, 1), date(2021, 11, 1), None),
        ("NQ.c.0", BACKWARD, date(2011, 1, 1), date(2021, 11, 1), None),
        ("NQ.c.0", FORWARD, date(2026, 11, 20), date(2026, 11, 23), None),
        # Sin nada guardado: un solo tramo entero.
        ("YM.c.0", None, date(2011, 1, 1), date(2026, 11, 23), None),
        # Eurex empieza el 2025-03-10: no hay nada anterior que pedir.
        ("FDAX.c.0", None, date(2026, 11, 23), date(2026, 11, 23), "ya está todo guardado"),
        # Inicio propio del símbolo (#410): RTY está en CME desde julio de 2017.
        ("RTY.c.0", None, date(2017, 7, 9), date(2026, 11, 23), None),
        ("GC.v.0", None, date(2011, 1, 1), date(2026, 11, 23), None),
    ]
    assert c.timeseries.calls == [], "estimar nunca baja datos"
    # El tope de gasto se aplica al total de todos los tramos (6 x 1,25).
    assert check_budget(est, CFG.max_usd) == Decimal("7.50")
    with pytest.raises(BudgetError):
        check_budget(est, Decimal("7.49"))
    # Nunca se pide lo que ya está guardado, ni un hueco entre el primer y el último tramo.
    for e in est:
        s = loaded.get((e.symbol.dataset, e.symbol.symbol))
        assert s is None or e.end <= s.first or e.start >= s.until


def test_the_part_before_is_fetched_from_the_newest_month_back() -> None:
    est = estimate(
        FakeClient(),
        replace(CFG, start=date(2021, 8, 15), symbols=CFG.symbols[:1]),
        date(2026, 11, 23),
        {("GLBX.MDP3", "ES.c.0"): Stored(date(2021, 11, 1), date(2026, 11, 23))},
    )
    assert chunks(est[0]) == [
        (date(2021, 10, 1), date(2021, 11, 1)),
        (date(2021, 9, 1), date(2021, 10, 1)),
        (date(2021, 8, 15), date(2021, 9, 1)),
    ]
    fwd = replace(est[0], part=FORWARD, start=date(2026, 10, 20), end=date(2026, 11, 23))
    assert chunks(fwd) == [
        (date(2026, 10, 20), date(2026, 11, 1)),
        (date(2026, 11, 1), date(2026, 11, 23)),
    ]


def test_the_budget_stops_everything() -> None:
    c = FakeClient(cost=30.0)
    est = estimate(c, CFG, date(2026, 11, 23), {})
    with pytest.raises(BudgetError):
        check_budget(est, CFG.max_usd)  # 6 x 30 = 180 > 100


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
    assert "total estimado: USD 12.00" in capsys.readouterr().out


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
    bars = fetch_bars(c, CFG, ES, DAY, date(2026, 11, 19)).bars
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
    bars = fetch_bars(FakeClient(records=day_of_cme()), CFG, ES, DAY, date(2026, 11, 19)).bars
    r = validate_day(bars, ES, DAY, 5)
    assert r.problems == []
    assert (r.bars, r.hours, r.longest_gap_minutes, r.contracts) == (1380, Decimal(23), 60, [42])


def test_the_sample_validation_reports_gaps_and_short_sessions() -> None:
    hole = {f"2026-11-18T10:{m:02d}:00" for m in range(10)}  # 10 min sin barras
    bars = fetch_bars(FakeClient(records=day_of_cme(hole)), CFG, ES, DAY, date(2026, 11, 19)).bars
    r = validate_day(bars, ES, DAY, 5)
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


FDAX = CFG.symbols[3]
NOV20 = date(2026, 11, 20)


def test_the_config_keeps_only_the_central_order_book_publisher() -> None:
    assert [(x.symbol, x.publisher, x.publisher_id) for x in CFG.symbols] == [
        ("ES.c.0", "GLBX.MDP3.GLBX", 1),
        ("NQ.c.0", "GLBX.MDP3.GLBX", 1),
        ("YM.c.0", "GLBX.MDP3.GLBX", 1),
        ("FDAX.c.0", "XEUR.EOBI.XEUR", 101),
        ("RTY.c.0", "GLBX.MDP3.GLBX", 1),
        ("GC.v.0", "GLBX.MDP3.GLBX", 1),
    ]
    for old, new in [
        ('publisher = "XEUR.EOBI.XEUR"', 'publisher = "XEUR.EOBI.XLIBRO"'),  # no existe
        ('publisher = "XEUR.EOBI.XEUR"', 'publisher = "GLBX.MDP3.GLBX"'),  # otro dataset
        ('publisher = "XEUR.EOBI.XEUR"\n', ""),  # falta
    ]:
        assert old in TEXT
        with pytest.raises(ConfigError):
            load_config(TEXT.replace(old, new, 1))


def fdax_minute() -> list[SimpleNamespace]:
    """El caso real del 2026-11-20 09:35 UTC (contrato 667): una barra del libro central y un
    print fuera de libro a 54 puntos del mercado, en el mismo minuto."""
    t = "2026-11-20T09:35:00"
    px = 1_000_000_000
    return [
        rec("2026-11-20T09:34:00", o=25060 * px, contract=667, publisher_id=101),
        rec(
            t,
            contract=667,
            publisher_id=101,
            open=25062 * px,
            high=25062 * px,
            low=25041 * px,
            close=25046 * px,
            volume=68,
        ),
        rec(
            t,
            contract=667,
            publisher_id=103,
            open=25100 * px,
            high=25100 * px,
            low=25100 * px,
            close=25100 * px,
            volume=41,
        ),
        rec("2026-11-20T09:36:00", o=25046 * px, contract=667, publisher_id=101),
    ]


def test_an_off_book_print_in_the_same_minute_is_dropped_not_mixed() -> None:
    f = fetch_bars(FakeClient(records=fdax_minute()), CFG, FDAX, NOV20, date(2026, 11, 21))
    assert [b.start.strftime("%H:%M") for b in f.bars] == ["09:34", "09:35", "09:36"]
    b = f.bars[1]
    assert (b.open, b.high, b.low, b.close, b.volume) == (
        Decimal(25062),
        Decimal(25062),
        Decimal(25041),
        Decimal(25046),
        68,
    )
    assert max(x.high for x in f.bars) < Decimal(25100), "el print fuera de libro no entra"
    assert (f.dropped, f.shared_minutes) == ({103: 1}, 1)
    assert "publisher 103: 1" in cli.describe_dropped(f, FDAX)


def test_two_records_of_the_same_publisher_in_one_minute_are_an_error() -> None:
    twice = fdax_minute()
    twice[2].publisher_id = 101
    with pytest.raises(DatabentoError, match="mismo publisher"):
        fetch_bars(FakeClient(records=twice), CFG, FDAX, NOV20, date(2026, 11, 21))
    no_pub = [rec("2026-11-20T09:35:00", publisher_id=None)]
    with pytest.raises(DatabentoError):
        fetch_bars(FakeClient(records=no_pub), CFG, FDAX, NOV20, date(2026, 11, 21))


def test_check_month_counts_duplicates_without_storing_and_only_filters_symbols(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.setenv("DATABENTO_API_KEY", "db-" + "k" * 29)
    monkeypatch.delenv("DATABASE_URL", raising=False)
    c = FakeClient(cost=0.55, records=fdax_minute())
    monkeypatch.setattr(cli, "client", lambda: c)
    assert cli.main(["x", "--check-month", "2026-11", "--only", "FDAX.c.0"]) == 0
    out = capsys.readouterr().out
    assert "FDAX.c.0 2026-11: 3 barras" in out
    assert "publisher 103: 1; 1 minutos" in out
    assert "ES.c.0" not in out
    req = c.timeseries.calls[0]
    # Hasta el fin publicado del dataset (2026-11-23 en el cliente falso), no hasta fin de mes.
    assert (req["symbols"], req["start"], req["end"]) == (["FDAX.c.0"], "2026-11-01", "2026-11-23")
    assert cli.main(["x", "--run", "--only", "XX.c.0"]) == 2
    assert cli.main(["x", "--check-month"]) == 2


def test_cli_run_fills_before_and_after_what_is_stored(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    import contextlib

    import psycopg

    from cfds_intel.databento import store

    monkeypatch.setenv("DATABENTO_API_KEY", "db-" + "k" * 29)
    monkeypatch.setenv("CFDS_DATA_MACHINE", "1")
    monkeypatch.setenv("DATABASE_URL", "postgresql://falsa")
    monkeypatch.setattr(cli, "CONFIG", ROOT / "config" / "databento.toml")
    monkeypatch.setattr(cli, "today_utc", lambda: date(2026, 11, 23))
    c = FakeClient(cost=0.5)
    monkeypatch.setattr(cli, "client", lambda: c)
    monkeypatch.setattr(psycopg, "connect", lambda *a, **k: contextlib.nullcontext(object()))
    stored_es = Stored(date(2021, 11, 1), date(2026, 11, 1))
    monkeypatch.setattr(
        store, "loaded_spans", lambda conn, *_: {("GLBX.MDP3", "ES.c.0"): stored_es}
    )
    saved: list[tuple[date, date]] = []

    def insert(conn: object, sym: object, a: date, b: date, *_: object) -> store.ChunkReport:
        saved.append((a, b))
        return store.ChunkReport(0, 0)

    monkeypatch.setattr(store, "insert_chunk", insert)
    assert cli.main(["x", "--run", "--only", "ES.c.0"]) == 0
    # Primero lo anterior, del mes más nuevo al más viejo; después lo posterior.
    assert saved[0] == (date(2021, 10, 1), date(2021, 11, 1))
    assert saved[129] == (date(2011, 1, 1), date(2011, 2, 1))
    assert saved[130:] == [(date(2026, 11, 1), date(2026, 11, 23))]
    assert all(a >= stored_es.until or b <= stored_es.first for a, b in saved)
    out = capsys.readouterr().out
    assert "ES.c.0 (GLBX.MDP3 → US500) [antes de lo guardado]: 2011-01-01 → 2021-11-01" in out
    assert "[después de lo guardado]: 2026-11-01 → 2026-11-23" in out
    assert "total estimado: USD 1.00" in out


def test_nothing_is_requested_past_the_published_end_of_the_dataset() -> None:
    # Forma real del 24-11-2026: el fin publicado es un instante a media tarde del día anterior.
    c = FakeClient()
    ranges = dict(c.metadata.ranges)
    ranges["GLBX.MDP3"] = {
        "start": "2010-06-06T00:00:00.000000000Z",
        "end": "2026-11-23T18:10:06.760366000Z",
        "schema": {
            "ohlcv-1m": {
                "start": "2010-06-06T00:00:00.000000000Z",
                "end": "2026-11-23T18:10:06.760366000Z",
            }
        },
    }
    c.metadata.ranges = ranges
    loaded = {
        ("GLBX.MDP3", "ES.c.0"): Stored(date(2011, 1, 1), date(2026, 11, 23)),
        ("GLBX.MDP3", "NQ.c.0"): Stored(date(2011, 1, 1), date(2026, 11, 20)),
    }
    est = estimate(c, CFG, date(2026, 11, 24), loaded)
    # ES ya tiene todo lo publicado (el 23 está a medias): no se pide nada, ni el 23.
    assert (est[0].symbol.symbol, est[0].skipped) == ("ES.c.0", "ya está todo guardado")
    # NQ: solo hasta la medianoche del 23, nunca hasta hoy.
    assert (est[1].start, est[1].end) == (date(2026, 11, 20), date(2026, 11, 23))
    for _, kw in c.metadata.calls:
        if "end" in kw and kw.get("dataset") == "GLBX.MDP3":
            assert kw["end"] <= "2026-11-23"


def test_cli_run_reports_a_failing_symbol_and_goes_on_with_the_others(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    import contextlib

    import psycopg

    from cfds_intel.databento import store

    key = "db-" + "k" * 29
    monkeypatch.setenv("DATABENTO_API_KEY", key)
    monkeypatch.setenv("CFDS_DATA_MACHINE", "1")
    monkeypatch.setenv("DATABASE_URL", "postgresql://falsa")
    monkeypatch.setattr(cli, "today_utc", lambda: date(2026, 11, 23))
    c = FakeClient(cost=0.01)
    calls: list[str] = []

    def get_range(**kw: Any) -> Iterable[Any]:
        calls.append(kw["symbols"][0])
        if kw["symbols"] == ["ES.c.0"]:
            raise RuntimeError("422 dataset_unavailable_range")
        return iter(())

    monkeypatch.setattr(c.timeseries, "get_range", get_range)
    monkeypatch.setattr(cli, "client", lambda: c)
    monkeypatch.setattr(psycopg, "connect", lambda *a, **k: contextlib.nullcontext(object()))
    every = {
        (s.dataset, s.symbol): Stored(date(2011, 1, 1), date(2026, 11, 20)) for s in CFG.symbols
    }
    monkeypatch.setattr(store, "loaded_spans", lambda conn, *_: every)
    saved: list[str] = []

    def insert(conn: object, sym: Any, a: date, b: date, *_: object) -> store.ChunkReport:
        saved.append(sym.symbol)
        return store.ChunkReport(0, 0)

    monkeypatch.setattr(store, "insert_chunk", insert)
    assert cli.main(["x", "--run"]) == 1
    names = [s.symbol for s in CFG.symbols]
    assert names[0] == "ES.c.0" and len(names) > 1
    assert calls == names
    assert saved == names[1:]
    err = capsys.readouterr().err
    assert "ES.c.0: RuntimeError: 422 dataset_unavailable_range" in err
    assert "tramos con error: ES.c.0" in err
    assert key not in err


def test_a_symbol_start_wins_over_the_global_one() -> None:
    rty = next(s for s in CFG.symbols if s.symbol == "RTY.c.0")
    assert rty.start == date(2017, 7, 9)
    assert all(s.start is None for s in CFG.symbols if s is not rty)
    c = FakeClient()
    est = estimate(c, replace(CFG, symbols=(rty,)), date(2026, 11, 23), {})
    assert (est[0].start, est[0].from_dataset_start) == (date(2017, 7, 9), False)
    # Nunca se pide nada anterior a su inicio (en CME daría 422 symbology_invalid_request).
    assert all(k["start"] >= "2017-07-09" for n, k in c.metadata.calls if "start" in k)
    # Con lo guardado desde su inicio, no hay tramo anterior que pedir.
    stored = {("GLBX.MDP3", "RTY.c.0"): Stored(date(2017, 7, 9), date(2026, 11, 23))}
    est = estimate(c, replace(CFG, symbols=(rty,)), date(2026, 11, 23), stored)
    assert est[0].skipped == "ya está todo guardado"
    # Un período anterior a su inicio (--sample, --check-month) no pide nada.
    est = estimate(c, replace(CFG, start=date(2015, 3, 1), symbols=(rty,)), date(2015, 4, 1), {})
    assert est[0].skipped == "no hay datos en el período pedido"


def test_check_month_skips_a_symbol_before_its_own_start(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.setenv("DATABENTO_API_KEY", "db-" + "k" * 29)
    monkeypatch.delenv("DATABASE_URL", raising=False)
    c = FakeClient(cost=0.01)
    monkeypatch.setattr(cli, "client", lambda: c)
    assert cli.main(["x", "--check-month", "2015-03", "--only", "RTY.c.0"]) == 0
    assert c.timeseries.calls == []
    assert "no hay datos en el período pedido" in capsys.readouterr().out
    # El mes en que empieza: solo desde su inicio.
    assert cli.main(["x", "--check-month", "2017-07", "--only", "RTY.c.0"]) == 0
    assert (c.timeseries.calls[0]["start"], c.timeseries.calls[0]["end"]) == (
        "2017-07-09",
        "2017-08-01",
    )


# --- Velas diarias (`--daily`, `config/databento_daily.toml`, #522) ---

DAILY_TEXT = (ROOT / "config" / "databento_daily.toml").read_text()
DAILY_CFG = load_config(DAILY_TEXT)
NKD = DAILY_CFG.symbols[0]
ROOTS = ["NKD", "ZN", "ZB", "6E", "6J", "6B", "6A", "SI", "CL", "HG"]


def test_the_daily_config_has_the_ten_research_futures_and_a_one_dollar_cap() -> None:
    assert [(s.dataset, s.symbol, s.instrument_id) for s in DAILY_CFG.symbols] == [
        ("GLBX.MDP3", f"{r}.v.0", r) for r in ROOTS
    ]
    assert (DAILY_CFG.schema, DAILY_CFG.stype_in) == ("ohlcv-1d", "continuous")
    assert (DAILY_CFG.start, DAILY_CFG.max_usd) == (date(2011, 1, 1), Decimal(1))
    assert DAILY_CFG.max_gap_minutes is None
    assert all(s.min_session_hours is None and s.publisher_id == 1 for s in DAILY_CFG.symbols)
    # Datos de investigación: ninguno es un instrumento operable del catálogo.
    import tomllib

    catalog = tomllib.loads((ROOT / "config" / "instruments.toml").read_text())
    ids = set(catalog["instruments"])
    assert ids and not ids & set(ROOTS)


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('max_usd = "1"', "max_usd = 1"),
        ('schema = "ohlcv-1d"', 'schema = "ohlcv-1h"'),
        # `[sample]` y `min_session_hours` son solo de 1 minuto.
        ('max_usd = "1"', 'max_usd = "1"\n\n[sample]\nmax_gap_minutes = 5'),
        ('instrument_id = "NKD"', 'instrument_id = "NKD"\nmin_session_hours = 22'),
        ('instrument_id = "ZB"', 'instrument_id = "NKD"'),
    ],
)
def test_bad_daily_config_is_rejected(old: str, new: str) -> None:
    assert old in DAILY_TEXT
    with pytest.raises(ConfigError):
        load_config(DAILY_TEXT.replace(old, new, 1))


def test_daily_records_start_at_utc_midnight_and_follow_their_contract() -> None:
    b = to_bar(rec("2026-11-18T00:00:00"), NKD, BAR_NS["ohlcv-1d"])
    assert b.start == datetime(2026, 11, 18, tzinfo=UTC)
    doc = json.loads((ROOT / "contracts" / "futures_bar_1d.v1.schema.json").read_text())
    d = b.to_contract()
    assert not list(Draft202012Validator(doc).iter_errors(d)), d
    assert d["start"] == "2026-11-18T00:00:00Z" and d["instrument_id"] == "NKD"
    # Una barra de 1 minuto no es una diaria, ni para la función ni para el contrato.
    with pytest.raises(DatabentoError, match="medianoche"):
        to_bar(rec("2026-11-18T14:31:00"), NKD, BAR_NS["ohlcv-1d"])
    minute = to_bar(rec("2026-11-18T14:31:00"), NKD).to_contract()
    assert list(Draft202012Validator(doc).iter_errors(minute))


def test_daily_fetch_asks_for_ohlcv_1d_and_checks_the_period() -> None:
    recs = [rec("2026-11-18T00:00:00"), rec("2026-11-19T00:00:00")]
    c = FakeClient(records=recs)
    f = fetch_bars(c, DAILY_CFG, NKD, date(2026, 11, 18), date(2026, 11, 20))
    assert [b.start.day for b in f.bars] == [18, 19]
    assert c.timeseries.calls[0]["schema"] == "ohlcv-1d"
    assert c.timeseries.calls[0]["symbols"] == ["NKD.v.0"]
    with pytest.raises(DatabentoError, match="fuera del período"):
        fetch_bars(FakeClient(records=recs), DAILY_CFG, NKD, date(2026, 11, 18), DAY)


def test_daily_requests_go_year_by_year() -> None:
    e = estimate(FakeClient(cost=0.0489), DAILY_CFG, date(2026, 11, 23), {})[0]
    assert (e.start, e.end) == (date(2011, 1, 1), date(2026, 11, 23))
    got = chunks(e, "ohlcv-1d")
    assert got[0] == (date(2011, 1, 1), date(2012, 1, 1))
    assert got[-1] == (date(2026, 1, 1), date(2026, 11, 23)) and len(got) == 16
    back = replace(e, part=BACKWARD)
    assert chunks(back, "ohlcv-1d")[0] == (date(2026, 1, 1), date(2026, 11, 23))


def test_the_approved_daily_estimate_fits_and_a_bigger_one_is_refused(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.setenv("DATABENTO_API_KEY", "db-" + "k" * 29)
    monkeypatch.delenv("DATABASE_URL", raising=False)
    monkeypatch.setattr(cli, "today_utc", lambda: date(2026, 11, 23))
    ok = FakeClient(cost=0.0489)
    monkeypatch.setattr(cli, "client", lambda: ok)
    assert cli.main(["x", "--daily", "--estimate"]) == 0
    out = capsys.readouterr().out
    assert "NKD.v.0 (GLBX.MDP3 → NKD): 2011-01-01 → 2026-11-23" in out
    assert "total estimado: USD 0.49 (tope USD 1)" in out
    assert {k for k, _ in ok.metadata.calls} <= {
        "get_cost",
        "get_record_count",
        "get_dataset_range",
    }
    # USD 0,11 por mercado = USD 1,10 > 1: ni --estimate ni --run siguen, y no se baja nada.
    big = FakeClient(cost=0.11)
    monkeypatch.setattr(cli, "client", lambda: big)
    monkeypatch.setenv("CFDS_DATA_MACHINE", "1")
    monkeypatch.setenv("DATABASE_URL", "postgresql://falsa")
    import contextlib

    import psycopg

    from cfds_intel.databento import store

    monkeypatch.setattr(psycopg, "connect", lambda *a, **k: contextlib.nullcontext(object()))
    monkeypatch.setattr(store, "loaded_spans", lambda conn, *_: {})
    assert cli.main(["x", "--daily", "--estimate"]) == 2
    assert cli.main(["x", "--daily", "--run"]) == 2
    assert "pasa el tope de USD 1" in capsys.readouterr().err
    assert big.timeseries.calls == []


def test_daily_run_stops_hard_when_the_requests_reach_the_cap(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    import contextlib

    import psycopg

    from cfds_intel.databento import store

    monkeypatch.setenv("DATABENTO_API_KEY", "db-" + "k" * 29)
    monkeypatch.setenv("CFDS_DATA_MACHINE", "1")
    monkeypatch.setenv("DATABASE_URL", "postgresql://falsa")
    monkeypatch.setattr(cli, "today_utc", lambda: date(2026, 11, 23))
    # La estimación total da USD 0,50 (10 por 0,05), pero cada pedido de un año también da 0,05
    # según get_cost: a los 20 pedidos la corrida llega a USD 1 y el 21 ya no se hace.
    c = FakeClient(cost=0.05)
    monkeypatch.setattr(cli, "client", lambda: c)
    monkeypatch.setattr(psycopg, "connect", lambda *a, **k: contextlib.nullcontext(object()))
    schemas: list[str] = []

    def spans(conn: object, schema: str) -> dict[tuple[str, str], Stored]:
        schemas.append(schema)
        return {}

    monkeypatch.setattr(store, "loaded_spans", spans)
    saved: list[tuple[str, date, str]] = []

    def insert(
        conn: object, sym: Any, a: date, b: date, bars: object, cost: object, schema: str
    ) -> store.ChunkReport:
        saved.append((sym.symbol, a, schema))
        return store.ChunkReport(0, 0)

    monkeypatch.setattr(store, "insert_chunk", insert)
    assert cli.main(["x", "--daily", "--run"]) == 2
    assert schemas == ["ohlcv-1d"]
    assert len(c.timeseries.calls) == 20 and len(saved) == 20
    assert all(s == "ohlcv-1d" for *_, s in saved)
    assert saved[16] == ("ZN.v.0", date(2011, 1, 1), "ohlcv-1d")
    assert "más que el tope de USD 1" in capsys.readouterr().err


def test_check_spend_is_a_hard_cap() -> None:
    assert check_spend(Decimal("0.5"), Decimal("0.5"), Decimal(1)) == Decimal(1)
    with pytest.raises(BudgetError):
        check_spend(Decimal("0.5"), Decimal("0.51"), Decimal(1))


def test_there_is_no_daily_sample(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.setenv("DATABENTO_API_KEY", "db-" + "k" * 29)
    c = FakeClient()
    monkeypatch.setattr(cli, "client", lambda: c)
    assert cli.main(["x", "--daily", "--sample"]) == 2
    assert "--sample es solo para ohlcv-1m" in capsys.readouterr().err
    assert c.metadata.calls == [] and c.timeseries.calls == []
    with pytest.raises(DatabentoError):
        validate_day([], NKD, DAY, 5)
