"""Registro de experimentos de ML v1 (docs/spec/ml-v1.md §7): el documento cumple
`contracts/ml_experiment.v1` en todos los casos, también en suspenso."""

import json
import math
import random
from datetime import UTC, datetime, timedelta
from pathlib import Path

import jsonschema
import pytest

from cfds_intel.models.baselines import load_probability_config
from cfds_intel.models.compare import Comparison
from cfds_intel.models.config import load_default
from cfds_intel.models.dataset import Example
from cfds_intel.models.registry import document
from cfds_intel.models.splits import Fold, Period, Window
from cfds_intel.models.train import evaluate

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads((ROOT / "contracts" / "ml_experiment.v1.schema.json").read_text())
CFG = load_default()
PROB = load_probability_config((ROOT / "config" / "probability.toml").read_text())
T0 = datetime(2024, 1, 1, tzinfo=UTC)


def synthetic(n: int, seed: int) -> tuple[Example, ...]:
    rnd = random.Random(seed)  # noqa: S311
    out = []
    for i in range(n):
        s = rnd.gauss(0, 1)
        y = 1 if rnd.random() < 1 / (1 + math.exp(-2 * s)) else 0
        at = T0 + timedelta(hours=5 * i)
        out.append(Example(f"r{seed}-{i}", at, at, (s, 1.0), y, ("sweep",), 6))
    return tuple(out)


def doc(folds: list[Fold], comparison: Comparison | None, data: tuple[Example, ...]) -> dict:  # type: ignore[type-arg]
    return document(
        created_at=T0,
        code_version="0123456789abcdef0123456789abcdef01234567",
        backtest_run_id="bt-1",
        config_sha256="a" * 64,
        examples=data,
        feature_names=["signal", "instrument_US500"],
        cfg=CFG,
        evaluation=evaluate(folds, CFG.training, PROB),
        comparison=comparison,
    )


W = Window(
    Period(T0, T0 + timedelta(days=60)), Period(T0 + timedelta(days=60), T0 + timedelta(days=90))
)


def test_a_trained_experiment_matches_the_contract() -> None:
    train, val = synthetic(500, 1), synthetic(100, 2)
    d = doc([Fold(W, train, val, 3, 0)], Comparison(-0.02, -0.04, -0.001, 12), train + val)
    jsonschema.validate(d, SCHEMA)
    assert d["result"]["status"] == "m1_improves"
    assert d["windows"][0]["purged"] == 3 and d["windows"][0]["skipped"] is None
    assert set(d["pooled"]) == {"B0", "B1", "M1"}


def test_a_suspended_experiment_is_recorded_too() -> None:
    small = synthetic(50, 3)
    d = doc([Fold(W, small, small, 0, 0)], None, small)
    jsonschema.validate(d, SCHEMA)
    assert d["result"] == {
        "status": "suspended",
        "difference": None,
        "low": None,
        "high": None,
        "blocks": None,
    }
    assert d["pooled"] == {} and d["windows"][0]["skipped"]


def test_the_id_changes_with_the_data_but_not_with_the_time() -> None:
    a = doc([], None, synthetic(10, 4))
    b = doc([], None, synthetic(10, 4))
    c = doc([], None, synthetic(11, 4))
    assert a["experiment_id"] == b["experiment_id"] != c["experiment_id"]


def test_the_contract_rejects_bad_documents() -> None:
    good = doc([], None, synthetic(10, 5))
    for key, bad in [
        ("code_version", "main"),
        ("created_at", "2024-01-01T00:00:00+01:00"),
        (
            "result",
            {"status": "maybe", "difference": None, "low": None, "high": None, "blocks": None},
        ),
    ]:
        try:
            jsonschema.validate({**good, key: bad}, SCHEMA)
        except jsonschema.ValidationError:
            continue
        raise AssertionError(f"el contrato aceptó {key}={bad!r}")


def test_the_label_source_is_optional_and_changes_the_id_only_when_present() -> None:
    # #409: los experimentos de backtest_runs conservan documento e id; los de ml_label_runs
    # llevan el campo y otro id, aunque el run_id sea el mismo.
    train, val = synthetic(300, 1), synthetic(60, 2)
    base = doc([Fold(W, train, val, 0, 0)], None, train + val)
    assert "label_source" not in base
    other = document(
        created_at=T0,
        code_version="0123456789abcdef0123456789abcdef01234567",
        backtest_run_id="bt-1",
        config_sha256="a" * 64,
        examples=train + val,
        feature_names=["signal", "instrument_US500"],
        cfg=CFG,
        evaluation=evaluate([Fold(W, train, val, 0, 0)], CFG.training, PROB),
        comparison=None,
        label_source="ml_label_runs",
    )
    jsonschema.validate(other, SCHEMA)
    assert other["label_source"] == "ml_label_runs"
    assert other["experiment_id"] != base["experiment_id"]
    with pytest.raises(ValueError, match="label_source"):
        document(
            created_at=T0,
            code_version="0123456789abcdef0123456789abcdef01234567",
            backtest_run_id="bt-1",
            config_sha256="a" * 64,
            examples=train + val,
            feature_names=["signal"],
            cfg=CFG,
            evaluation=evaluate([Fold(W, train, val, 0, 0)], CFG.training, PROB),
            comparison=None,
            label_source="otra",
        )
