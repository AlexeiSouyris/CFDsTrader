"""Estados emocionales que salen del índice (spec §9.1, #220). Sin base ni red."""

from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path

import pytest

from cfds_intel.sentiment.index import (
    ComponentState,
    NotEmitted,
    SentimentConfigError,
    SentimentIndex,
    load_config,
)
from cfds_intel.sentiment.states import (
    COMPLACENCY,
    EXTREME_EUPHORIA,
    EXTREME_FEAR,
    index_states,
    load_states_config,
)

CONFIG = (Path(__file__).resolve().parents[2] / "config" / "sentiment.toml").read_text()
CFG = load_states_config(CONFIG)
T = datetime(2026, 11, 20, 23, tzinfo=UTC)


def vol(status: str, pct: str | None) -> ComponentState:
    p = None if pct is None else Decimal(pct)
    return ComponentState("realized_vol", status, Decimal(1), None, p, None)


def index(value: str, pct: str = "0.5") -> SentimentIndex:
    return SentimentIndex("US500", T, T, Decimal(value), True, 3, (vol("valid", pct),))


def test_the_real_config_loads_with_the_spec_thresholds() -> None:
    assert (CFG.extreme_fear, CFG.extreme_euphoria) == (Decimal("-0.6"), Decimal("0.6"))
    assert CFG.complacency_percentile == Decimal("0.10")
    load_config(CONFIG)  # el índice sigue aceptando el archivo con [states]


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ("-0.6", (EXTREME_FEAR,)),  # el borde cuenta
        ("-0.59", ()),
        ("0.6", (EXTREME_EUPHORIA,)),
        ("0.5999", ()),
        ("-1", (EXTREME_FEAR,)),
    ],
)
def test_extreme_fear_and_euphoria(value: str, expected: tuple[str, ...]) -> None:
    assert index_states(index(value), CFG) == expected


def test_complacency_uses_the_raw_volatility_percentile() -> None:
    assert index_states(index("0", pct="0.10"), CFG) == (COMPLACENCY,)
    assert index_states(index("0", pct="0.11"), CFG) == ()
    assert index_states(index("0.7", pct="0.05"), CFG) == (EXTREME_EUPHORIA, COMPLACENCY)


def test_without_index_only_complacency_can_exist() -> None:
    skipped = NotEmitted("US500", T, 1, 3, (vol("valid", "0.02"),))
    assert index_states(skipped, CFG) == (COMPLACENCY,)
    expired = NotEmitted("US500", T, 0, 3, (vol("expired", None),))
    assert index_states(expired, CFG) == ()


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('extreme_fear = "-0.6"', 'extreme_fear = "0.1"'),
        ('extreme_euphoria = "0.6"', 'extreme_euphoria = "1.5"'),
        ('complacency_percentile = "0.10"', 'complacency_percentile = "0"'),
        ('complacency_percentile = "0.10"', 'complacency_percentile = "x"'),
        ('extreme_fear = "-0.6"', 'extreme_fear = "-0.6"\ntypo = 1'),
    ],
)
def test_a_bad_states_config_is_rejected(old: str, new: str) -> None:
    with pytest.raises(SentimentConfigError):
        load_states_config(CONFIG.replace(old, new))
