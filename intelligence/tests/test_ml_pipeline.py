"""Pipeline de ML v1 de punta a punta sin base de datos (#400): una fuente de etiquetas
sintética → dataset → cortes walk-forward → B0, B1 y M1 → regla de mejora → informe."""

import math
import random
from dataclasses import dataclass
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Any

import psycopg

from cfds_intel.models.config import load_default
from cfds_intel.models.dataset import LabelSource, Row
from cfds_intel.models.experiment import CONFIG_FILES, run
from cfds_intel.models.report import render
from cfds_intel.models.splits import load_split_config, plan, research

ROOT = Path(__file__).resolve().parents[2]
CFG = load_default()
TEXTS = {n: (ROOT / "config" / n).read_text() for n in CONFIG_FILES}
FIRST = datetime(2015, 1, 1, tzinfo=UTC)
LAST = datetime(2019, 12, 31, 21, 0, tzinfo=UTC)
TRIGGERS = ("candle_pattern", "double_break", "level_rejection", "sweep")


def utc(t: datetime) -> str:
    return t.isoformat().replace("+00:00", "Z")


def proposals(seed: int) -> list[Row]:
    """~1 propuesta por día hábil; `y` depende de la distancia a TP1 y del puntaje."""
    rnd = random.Random(seed)  # noqa: S311 (datos de prueba)
    out = []
    t = FIRST + timedelta(hours=15)
    k = 0
    while t < LAST:
        if t.weekday() < 5:
            score = rnd.randint(5, 9)
            tp1_r = rnd.choice([Decimal("1"), Decimal("1.5"), Decimal("2"), Decimal("3")])
            stop_atr = Decimal(rnd.randint(3, 20)) / 10
            logit = -0.8 * float(tp1_r) + 0.35 * (score - 7) + 0.5
            y = rnd.random() < 1 / (1 + math.exp(-logit))
            atr = Decimal("4")
            risk = stop_atr * atr
            trig = TRIGGERS[k % 4]
            out.append(
                Row(
                    proposal_id=f"bt:US500.FUT:{k}",
                    # Algunas del CFD mezcladas: con la fuente de futuros no tienen que entrar.
                    instrument_id="US500" if k % 17 == 0 else "US500.FUT",
                    side="long" if k % 2 else "short",
                    signal_at=t,
                    reference=Decimal("100"),
                    stop=Decimal("100") - risk,
                    tp1=Decimal("100") + risk * tp1_r,
                    score=score,
                    breakdown=[
                        {"component": trig, "points": 2},
                        {"component": "trend", "points": 2},
                    ],
                    triggers=[trig],
                    hit_tp1=y,
                    exits=[{"at": utc(t + timedelta(hours=rnd.randint(1, 8)))}],
                    as_of=None if k < 30 else t,
                    atr14_m15=None if k < 30 else atr,
                    realized_vol_tercile=None if k < 30 else ("low", "mid", "high")[k % 3],
                    session_band=None if k < 30 else ("open", "mid", "close")[k % 3],
                )
            )
            k += 1
        t += timedelta(days=1)
    return out


@dataclass(frozen=True)
class Synthetic:
    data: tuple[Row, ...]

    @property
    def ref(self) -> str:
        return "sintetico"

    @property
    def table(self) -> str:
        return "ml_label_runs"

    def rows(self, conn: psycopg.Connection) -> list[Row]:
        return list(self.data)

    def span(self, conn: psycopg.Connection) -> tuple[datetime, datetime]:
        return FIRST, LAST


def small_cfg() -> Any:
    """La config del repo con mínimos y bootstrap chicos: los datos sintéticos son pocos."""
    from dataclasses import replace

    return replace(
        CFG,
        training=replace(CFG.training, min_examples=150, min_per_class=30),
        evaluation=replace(CFG.evaluation, bootstrap_resamples=200),
    )


def experiment(seed: int = 1) -> Any:
    src: LabelSource = Synthetic(tuple(proposals(seed)))
    return run(None, src, small_cfg(), TEXTS, "futuros")  # type: ignore[arg-type]


def test_the_pipeline_runs_end_to_end_on_the_futures_source() -> None:
    dataset, evaluation, doc, excluded = experiment()
    # 30 sin variables, menos las 2 del CFD (k = 0 y 17), que cuentan como de otra fuente.
    assert dataset.without_features == 28, "calentamiento: contado aparte, nunca rellenado"
    assert dataset.other_instrument > 0, "las del CFD no entran con la fuente de futuros"
    assert not evaluation.suspended
    trained = [r for r in evaluation.windows if r.skipped is None]
    assert trained
    split = plan(FIRST, LAST, load_split_config(TEXTS["data_split.toml"]))
    oos = split.out_of_sample.start
    assert excluded == len(dataset.examples) - len(research(dataset.examples, split)) > 0
    for r in evaluation.windows:
        assert all(e.signal_at < oos and e.known_at < oos for e in r.validation)
    assert doc["backtest_run_id"] == "sintetico"
    assert doc["result"]["status"] in ("m1_improves", "b1_stays")
    # M1 ve tp1_r y el puntaje: tiene que perder menos que la tasa base sola.
    assert evaluation.pooled["M1"].log_loss < evaluation.pooled["B0"].log_loss


def test_the_pipeline_is_reproducible() -> None:
    _, a, doc_a, _ = experiment()
    _, b, doc_b, _ = experiment()
    assert a == b
    assert doc_a["pooled"] == doc_b["pooled"] and doc_a["result"] == doc_b["result"]


def test_the_report_has_every_section_and_no_promises() -> None:
    dataset, evaluation, doc, excluded = experiment()
    text = render(
        title="ML v1 sintético",
        source="futuros",
        dataset=dataset,
        out_of_sample_excluded=excluded,
        evaluation=evaluation,
        doc=doc,
        calibration_bins=CFG.evaluation.calibration_bins,
    )
    for section in ("## Datos", "## Ventanas walk-forward", "## Resultado", "## Calibración"):
        assert section in text
    assert text.count("| Grupo | N |") == 3, "una tabla de calibración por modelo"
    assert f"**no se miran**): {excluded}" in text
    assert "informativo" in text.lower()
    assert "ganancia" not in text.lower()


def test_a_suspended_experiment_still_reports() -> None:
    from dataclasses import replace

    cfg = replace(CFG, training=replace(CFG.training, min_examples=10_000))
    src = Synthetic(tuple(proposals(2)))
    dataset, evaluation, doc, excluded = run(None, src, cfg, TEXTS, "futuros")  # type: ignore[arg-type]
    assert evaluation.suspended and doc["result"]["status"] == "suspended"
    text = render(
        title="t",
        source="futuros",
        dataset=dataset,
        out_of_sample_excluded=excluded,
        evaluation=evaluation,
        doc=doc,
        calibration_bins=10,
    )
    assert "en suspenso" in text and "## Calibración" not in text
