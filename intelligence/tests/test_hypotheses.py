"""Arnés de las hipótesis H1-H7 (spec §9.1, #227), solo con datos sintéticos."""

import random
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import pytest

from cfds_intel.sentiment.hypotheses import (
    INSUFFICIENT,
    NOT_SUPPORTED,
    PENDING,
    SUPPORTED,
    Bar,
    Expect,
    HypothesisConfigError,
    IgPoint,
    IndexPoint,
    event_returns,
    forward_returns,
    frequency_test,
    load_config,
    mean_test,
    run_all,
)

TEXT = (Path(__file__).resolve().parents[2] / "config" / "hypotheses.toml").read_text()
# Menos iteraciones para que los tests sean rápidos; el criterio es el mismo.
CFG = load_config(TEXT.replace("bootstrap_iterations = 10000", "bootstrap_iterations = 400"))
T0 = datetime(2024, 1, 1, 22, tzinfo=UTC)


def noise(n: int, center: str, seed: int) -> list[Decimal]:
    rng = random.Random(seed)  # noqa: S311
    return [Decimal(center) + Decimal(rng.randint(-100, 100)) / 10000 for _ in range(n)]


def bars(closes: list[int], step: timedelta = timedelta(days=1)) -> list[Bar]:
    return [Bar(T0 + i * step, Decimal(c)) for i, c in enumerate(closes)]


def test_the_real_config_has_the_spec_horizons() -> None:
    c = load_config(TEXT)
    assert (c.test.confidence, c.test.min_cases) == (Decimal("0.95"), 30)
    assert (c.h1_days, c.h2_days, c.h3_days, c.h4_hours, c.h4_days, c.h5_days) == (
        20,
        20,
        (1, 5),
        4,
        5,
        5,
    )
    assert c.ig_majority_pct == Decimal(75)
    with pytest.raises(HypothesisConfigError):
        load_config(TEXT.replace('confidence = "0.95"', 'confidence = "1.5"'))
    with pytest.raises(HypothesisConfigError):
        load_config(TEXT.replace("min_cases = 30", "min_casos = 30"))


def test_returns_start_at_the_first_bar_known_after_the_event() -> None:
    b = bars([100, 110, 121, 133])
    assert forward_returns(b, 1)[0] == Decimal("0.1")
    # Un evento a mitad del día 1 se mide desde el cierre del día 1, nunca del día 0.
    mid = T0 + timedelta(hours=12)
    assert event_returns(b, [mid], 1) == [Decimal("0.1")]
    # Dos eventos en la misma vela cuentan una vez; sin horizonte completo, no cuenta.
    assert event_returns(b, [T0, T0 - timedelta(hours=1), b[3].end], 1) == [Decimal("0.1")]


def test_fewer_than_30_cases_is_insufficient_and_computes_nothing() -> None:
    r = mean_test("x", noise(29, "0.02", 1), noise(500, "0", 2), Expect.HIGHER, CFG.test)
    assert (r.status, r.cases, r.difference, r.ci_low) == (INSUFFICIENT, 29, None, None)


def test_supported_only_on_the_expected_side() -> None:
    cases, pop = noise(60, "0.02", 1), noise(500, "0", 2)
    up = mean_test("x", cases, pop, Expect.HIGHER, CFG.test)
    assert up.status == SUPPORTED
    assert up.ci_low is not None and up.ci_low > 0
    # La misma diferencia no apoya una hipótesis que esperaba lo contrario.
    assert mean_test("x", cases, pop, Expect.LOWER, CFG.test).status == NOT_SUPPORTED
    down = mean_test("x", noise(60, "-0.02", 3), pop, Expect.LOWER, CFG.test)
    assert down.status == SUPPORTED


def test_no_difference_is_not_supported_and_the_interval_contains_zero() -> None:
    r = mean_test("x", noise(80, "0", 4), noise(500, "0", 5), Expect.HIGHER, CFG.test)
    assert r.status == NOT_SUPPORTED
    assert r.ci_low is not None and r.ci_high is not None and r.ci_low < 0 < r.ci_high


def test_same_input_same_interval() -> None:
    a = mean_test("x", noise(60, "0.01", 6), noise(300, "0", 7), Expect.HIGHER, CFG.test)
    b = mean_test("x", noise(60, "0.01", 6), noise(300, "0", 7), Expect.HIGHER, CFG.test)
    assert a == b


def test_h6_compares_hit_frequencies() -> None:
    hits = [i % 5 != 0 for i in range(100)]  # 80 %
    rand = [i % 5 < 2 for i in range(200)]  # 40 %
    assert frequency_test("H6", hits, rand, CFG.test).status == SUPPORTED
    assert frequency_test("H6", rand[:100], hits * 2, CFG.test).status == NOT_SUPPORTED


def test_run_all_on_a_synthetic_market() -> None:
    rng = random.Random(8)  # noqa: S311
    closes = [1000]
    for _ in range(700):
        closes.append(closes[-1] + rng.randint(-10, 10))
    daily = bars(closes)
    hourly = bars(closes, timedelta(hours=1))
    # Miedo extremo cada 7 días; el día siguiente sube 5 % (H1 debería salir apoyada).
    fear_days = list(range(10, 650, 7))
    for d in fear_days:
        closes[d + 1 :] = [c + closes[d] // 20 for c in closes[d + 1 :]]
    daily = bars(closes)
    index = [IndexPoint(daily[d].end, Decimal("-0.7")) for d in fear_days]
    results = run_all(
        index,
        ig=[IgPoint(daily[d].end, Decimal(80)) for d in range(5, 20)],  # 15 casos: pocos
        fomo=[],
        climax=[daily[d].end for d in fear_days],
        daily=daily,
        hourly=hourly,
        h6_hits=[],
        h6_random_hits=[],
        fear=Decimal("-0.6"),
        euphoria=Decimal("0.6"),
        cfg=CFG,
    )
    by = {r.name: r.status for r in results}
    assert list(by) == [
        "H1",
        "H2",
        "H3 largos 1d",
        "H3 cortos 1d",
        "H3 largos 5d",
        "H3 cortos 5d",
        "H4 continúa",
        "H4 revierte",
        "H5",
        "H6",
        "H7",
    ]
    assert by["H1"] == SUPPORTED
    assert by["H5"] == SUPPORTED  # clímax con el índice ya en miedo extremo
    assert by["H2"] == by["H3 largos 1d"] == by["H4 continúa"] == by["H6"] == INSUFFICIENT
    assert by["H7"] == PENDING


def test_h5_only_uses_the_index_already_published() -> None:
    closes = list(range(1000, 1100))
    daily = bars(closes)
    climax = [daily[d].end for d in range(40, 80)]
    # El índice en miedo se publica después de todos los clímax: H5 no tiene casos.
    late = [IndexPoint(daily[90].end, Decimal(-1))]
    results = run_all(
        late, [], [], climax, daily, daily, [], [], Decimal("-0.6"), Decimal("0.6"), CFG
    )
    h5 = next(r for r in results if r.name == "H5")
    assert (h5.status, h5.cases) == (INSUFFICIENT, 0)
