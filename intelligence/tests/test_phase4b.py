"""Reporte de aprobación de la fase 4b (#221) con corridas sintéticas: sin base ni red."""

from dataclasses import replace
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import pytest

from cfds_intel.models.splits import load_split_config, plan
from cfds_intel.phase4b.__main__ import min_weeks
from cfds_intel.phase4b.report import (
    LABEL,
    Bootstrap,
    Run,
    Trade,
    compare_news,
    ml_from_document,
    render,
)

ROOT = Path(__file__).resolve().parents[2]
SPLIT = load_split_config((ROOT / "config" / "data_split.toml").read_text())
BOOT = Bootstrap(resamples=300, confidence=0.95, seed=7)
FROM, TO = datetime(2021, 1, 1, tzinfo=UTC), datetime(2026, 4, 30, tzinfo=UTC)
NEWS = {"interpreter_name": "x", "published_from": "2025-12-03T00:00:00Z", "interpretations": 40}


def weekly_trades(pnl: list[str], start: datetime) -> tuple[Trade, ...]:
    """Una operación por semana desde `start`, resuelta 2 h después de la señal."""
    out = []
    for i, p in enumerate(pnl):
        at = start + timedelta(weeks=i, hours=15)
        out.append(Trade(at, at + timedelta(hours=2), Decimal(p)))
    return tuple(out)


def run(run_id: str, trades: tuple[Trade, ...], news: dict[str, object] | None) -> Run:
    return Run(run_id, ("US500",), FROM, TO, "a" * 64, news, trades)


def validation_start() -> datetime:
    return plan(FROM, TO, SPLIT).windows[0].validation.start


def pair(on_pnl: list[str], off_pnl: list[str]) -> tuple[Run, Run]:
    s = validation_start()
    return run("on", weekly_trades(on_pnl, s), NEWS), run("off", weekly_trades(off_pnl, s), None)


def test_the_real_config_loads() -> None:
    assert min_weeks((ROOT / "config" / "phase4b.toml").read_text()) == 26
    with pytest.raises(ValueError, match="min_weeks"):
        min_weeks("[news]\nmin_weeks = 0")


def test_news_improves_when_every_week_is_better() -> None:
    n = 60
    on, off = pair([str(10 + i % 3) for i in range(n)], [str(i % 3) for i in range(n)])
    r = compare_news(on, off, SPLIT, BOOT, 26)
    assert r.status == "improves", r
    assert r.difference is not None and r.low is not None and r.low > 0
    assert r.trades_on == r.trades_off == n
    assert r.weeks >= r.trades_on  # todas las semanas de validación cuentan
    # Reproducible con la misma semilla.
    assert compare_news(on, off, SPLIT, BOOT, 26) == r


def test_equal_runs_do_not_improve() -> None:
    on, off = pair(["5", "-3"] * 30, ["5", "-3"] * 30)
    r = compare_news(on, off, SPLIT, BOOT, 26)
    assert r.status == "no_improvement"
    assert r.difference == 0


def test_one_lucky_week_is_not_an_improvement() -> None:
    # Solo una semana distinta: la media es positiva pero el intervalo incluye 0.
    on, off = pair(["500"] + ["0"] * 59, ["0"] * 60)
    r = compare_news(on, off, SPLIT, BOOT, 26)
    assert r.difference is not None and r.difference > 0
    assert r.status == "no_improvement"


def test_nothing_outside_validation_or_resolved_out_of_sample_counts() -> None:
    split = plan(FROM, TO, SPLIT)
    oos = split.out_of_sample.start
    on, off = pair(["1"] * 60, ["0"] * 60)
    # Una operación del entrenamiento y otra resuelta ya fuera de muestra, enormes.
    train = Trade(FROM + timedelta(days=40), FROM + timedelta(days=40, hours=2), Decimal(10**6))
    late_signal = oos - timedelta(hours=1)
    late = Trade(late_signal, oos + timedelta(hours=1), Decimal(10**6))
    on2 = replace(on, trades=(*on.trades, train, late))
    r, r2 = compare_news(on, off, SPLIT, BOOT, 26), compare_news(on2, off, SPLIT, BOOT, 26)
    assert r2.pnl_on == r.pnl_on and r2.difference == r.difference
    assert r2.excluded_oos == r.excluded_oos + (1 if late_signal >= validation_start() else 0)


@pytest.mark.parametrize(
    ("case", "reason"),
    [
        ("short", "sin ventanas walk-forward"),
        ("no_interpretations", "ninguna interpretación"),
        ("few_weeks", "semanas de validación"),
        ("no_trades", "ninguna operación"),
    ],
)
def test_insufficient_data_is_said_not_invented(case: str, reason: str) -> None:
    on, off = pair(["1"] * 60, ["0"] * 60)
    weeks = 26
    if case == "short":
        on = replace(on, to_at=FROM + timedelta(days=400))
        off = replace(off, to_at=on.to_at)
    elif case == "no_interpretations":
        on = replace(on, news={**NEWS, "interpretations": 0})
    elif case == "few_weeks":
        weeks = 10_000
    else:
        on, off = replace(on, trades=()), replace(off, trades=())
    r = compare_news(on, off, SPLIT, BOOT, weeks)
    assert r.status == "insufficient"
    assert r.reason is not None and reason in r.reason
    assert r.difference is None and r.low is None


def test_runs_that_differ_in_more_than_the_layer_are_refused() -> None:
    on, off = pair(["1"], ["0"])
    with pytest.raises(ValueError, match="iguales"):
        compare_news(on, replace(off, config_sha256="b" * 64), SPLIT, BOOT, 26)
    with pytest.raises(ValueError, match="sin-noticias"):
        compare_news(on, replace(off, news=NEWS), SPLIT, BOOT, 26)
    with pytest.raises(ValueError, match="sin-noticias"):
        compare_news(replace(on, news=None), off, SPLIT, BOOT, 26)


def ml_doc(status: str) -> dict[str, object]:
    window = {"skipped": None, "purged": 3, "embargoed": 0}
    skipped = {"skipped": "menos de 300 propuestas", "purged": 0, "embargoed": 0}
    improves = status == "m1_improves"
    return {
        "experiment_id": "f" * 64,
        "result": {
            "status": status,
            "difference": None if status == "suspended" else (-0.02 if improves else 0.01),
            "low": None if status == "suspended" else (-0.03 if improves else -0.01),
            "high": None if status == "suspended" else (-0.01 if improves else 0.03),
            "blocks": None if status == "suspended" else 40,
        },
        "windows": [window, skipped],
    }


def test_ml_reads_the_registered_rule_without_recomputing_it() -> None:
    assert ml_from_document(None).status == "insufficient"
    assert ml_from_document(ml_doc("m1_improves")).status == "improves"
    assert ml_from_document(ml_doc("b1_stays")).status == "no_improvement"
    s = ml_from_document(ml_doc("suspended"))
    assert s.status == "insufficient" and s.reason
    m = ml_from_document(ml_doc("b1_stays"))
    assert (m.trained, m.skipped, m.purged, m.blocks) == (1, 1, 3, 40)


def test_the_report_has_a_conclusion_per_layer_and_promises_nothing() -> None:
    on, off = pair([str(10 + i % 3) for i in range(60)], [str(i % 3) for i in range(60)])
    news = compare_news(on, off, SPLIT, BOOT, 26)
    text = render(
        on,
        off,
        news,
        ml_from_document(None),
        BOOT,
        datetime(2026, 11, 23, tzinfo=UTC),
        "abc1234",
    )
    assert "| Noticias (ajuste del puntaje) | **mejora** |" in text
    assert f"| Análogos históricos | **{LABEL['not_applicable']}** |" in text
    assert "| ML v1 (M1 contra B1) | **datos insuficientes** |" in text
    assert "sin experimento registrado" in text
    assert "fuera de muestra" in text and "no prometen" in text
    assert "2025-12-03" in text  # corte de las noticias
