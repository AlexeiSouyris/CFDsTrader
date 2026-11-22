"""Cortes del tiempo de ML v1 (docs/spec/ml-v1.md §5 y §8.3, #158): sin fuga."""

import json
from datetime import datetime, timedelta
from pathlib import Path

import pytest

from cfds_intel.models.dataset import Example
from cfds_intel.models.splits import (
    DataSplit,
    Period,
    SplitError,
    Window,
    folds,
    inner_split,
    load_embargo,
    load_split_config,
    plan,
    research,
)

ROOT = Path(__file__).resolve().parents[2]
CFG = load_split_config((ROOT / "config" / "data_split.toml").read_text())
EMBARGO = load_embargo((ROOT / "config" / "backtest.toml").read_text())


def utc(s: str) -> datetime:
    return datetime.fromisoformat(s.replace("Z", "+00:00"))


def ex(k: str, signal: datetime, known: timedelta = timedelta(hours=2)) -> Example:
    return Example(k, signal, signal + known, (0.0,), 0, ("sweep",), 6)


def test_the_repository_config_is_the_spec() -> None:
    # §5: 12 meses fuera de muestra, 24 + 3, paso 3; embargo de 32 velas de 15m.
    assert (CFG.out_of_sample_months, CFG.train_months, CFG.validation_months) == (12, 24, 3)
    assert CFG.step_months == 3
    assert EMBARGO == timedelta(hours=8)
    for bad in ["", "[out_of_sample]\nmonths = 0\n[walk_forward]\ntrain_months = 1\n"]:
        with pytest.raises(SplitError):
            load_split_config(bad)


def z(t: datetime) -> str:
    return t.isoformat().replace("+00:00", "Z")


def test_the_plan_is_the_same_as_rust() -> None:
    # Mismo fixture que `cfds-market-data/tests/split.rs`.
    doc = json.loads((ROOT / "intelligence/tests/fixtures/data_split_plan.json").read_text())
    for case in doc["cases"]:
        first, last = utc(case["first"]), utc(case["last"])
        if "error" in case:
            with pytest.raises(SplitError):
                plan(first, last, CFG)
            continue
        s = plan(first, last, CFG)
        got = [[z(w.train.start), z(w.train.end), z(w.validation.end)] for w in s.windows]
        assert got == case["windows"]
        assert [z(s.out_of_sample.start), z(s.out_of_sample.end)] == case["out_of_sample"]
        assert all(w.train.end == w.validation.start for w in s.windows)


def small_split() -> DataSplit:
    w = Window(
        Period(utc("2024-01-01T00:00:00Z"), utc("2024-03-01T00:00:00Z")),
        Period(utc("2024-03-01T00:00:00Z"), utc("2024-04-01T00:00:00Z")),
    )
    return DataSplit((w,), Period(utc("2024-04-01T00:00:00Z"), utc("2024-06-01T00:00:00Z")))


def test_no_proposal_is_in_train_and_validation_of_the_same_window() -> None:
    split = plan(utc("2020-01-01T00:00:00Z"), utc("2026-11-20T00:00:00Z"), CFG)
    examples = [ex(f"p{i}", utc("2020-01-01T15:00:00Z") + timedelta(days=i)) for i in range(2500)]
    for f in folds(examples, split, EMBARGO):
        train, val = {e.proposal_id for e in f.train}, {e.proposal_id for e in f.validation}
        assert not train & val
        assert all(e.known_at < f.window.validation.start for e in f.train)
        assert all(e.signal_at in f.window.validation for e in f.validation)


def test_the_purge_removes_exactly_the_ones_known_after_validation_starts() -> None:
    split = small_split()
    v0 = split.windows[0].validation.start
    inside = ex("resuelta antes", v0 - timedelta(hours=3), timedelta(hours=2))
    edge = ex("resuelta en el inicio", v0 - timedelta(hours=2), timedelta(hours=2))
    crossing = ex("resuelta después", v0 - timedelta(hours=1), timedelta(hours=2))
    (f,) = folds([inside, edge, crossing], split, EMBARGO)
    assert [e.proposal_id for e in f.train] == ["resuelta antes"]
    assert f.purged == 2


def test_the_embargo_removes_the_bars_after_validation_from_training() -> None:
    # Con ventanas hacia adelante el entrenamiento nunca queda después de su validación: para
    # probar el embargo, una ventana armada a mano con el entrenamiento al final.
    v = Period(utc("2024-01-01T00:00:00Z"), utc("2024-02-01T00:00:00Z"))
    t = Period(utc("2024-02-01T00:00:00Z"), utc("2024-04-01T00:00:00Z"))
    split = DataSplit(
        (Window(t, v),), Period(utc("2024-05-01T00:00:00Z"), utc("2024-06-01T00:00:00Z"))
    )
    in_embargo = ex("en el embargo", v.end + EMBARGO - timedelta(minutes=15))
    after = ex("después del embargo", v.end + EMBARGO)
    (f,) = folds([in_embargo, after], split, EMBARGO)
    assert f.embargoed == 1
    assert [e.proposal_id for e in f.train] == []  # la otra queda purgada: se resuelve después
    assert f.purged == 1


def test_out_of_sample_is_never_handed_out() -> None:
    split = small_split()
    oos = split.out_of_sample.start
    decided_in_oos = ex("decidida fuera de muestra", oos + timedelta(hours=1))
    resolved_in_oos = ex("resuelta fuera de muestra", oos - timedelta(hours=1))
    clean = ex("limpia", oos - timedelta(days=5))
    usable = research([decided_in_oos, resolved_in_oos, clean], split)
    assert [e.proposal_id for e in usable] == ["limpia"]
    (f,) = folds([decided_in_oos, resolved_in_oos, clean], split, EMBARGO)
    seen = {e.proposal_id for e in (*f.train, *f.validation)}
    assert seen == {"limpia"}


def test_the_inner_split_is_the_last_fifth_with_the_same_purge() -> None:
    t0 = utc("2024-01-01T15:00:00Z")
    train = [ex(f"p{i:02d}", t0 + timedelta(hours=i), timedelta(minutes=90)) for i in range(10)]
    fit, tune = inner_split(train, 0.2)
    assert [e.proposal_id for e in tune] == ["p08", "p09"]
    # p07 se resuelve 30 min después de que empieza p08: queda purgada.
    assert [e.proposal_id for e in fit] == [f"p{i:02d}" for i in range(7)]
    assert inner_split([], 0.2) == ((), ())
    with pytest.raises(SplitError):
        inner_split(train, 1.0)
