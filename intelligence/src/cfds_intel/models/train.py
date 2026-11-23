"""Evaluación walk-forward de ML v1 (docs/spec/ml-v1.md §4 a §6).

En cada ventana: si el entrenamiento llega al mínimo (§6: 300 propuestas y 50 de cada clase),
se ajustan B0, B1 y M1 y se predice la validación. Si no llega, la ventana **no entrena** y queda
el motivo. Las métricas son del **conjunto** de las validaciones, nunca de una sola. Si ninguna
ventana entrena, ML v1 queda en suspenso y se sigue mostrando B1.
"""

from collections.abc import Sequence
from dataclasses import dataclass, field

from cfds_intel.models.baselines import B0, B1, ProbabilityConfig
from cfds_intel.models.config import TrainingConfig
from cfds_intel.models.dataset import Example
from cfds_intel.models.logistic import fit_with_inner_tuning
from cfds_intel.models.metrics import auc, brier, log_loss
from cfds_intel.models.splits import Fold, Window

MODELS = ("B0", "B1", "M1")


def minimum_reason(train: Sequence[Example], cfg: TrainingConfig) -> str | None:
    """Por qué una ventana no entrena, o `None` si llega al mínimo."""
    positives = sum(e.y for e in train)
    negatives = len(train) - positives
    if len(train) < cfg.min_examples:
        return f"{len(train)} propuestas, el mínimo es {cfg.min_examples}"
    if min(positives, negatives) < cfg.min_per_class:
        return (
            f"{positives} con TP1 y {negatives} sin TP1, el mínimo es {cfg.min_per_class} "
            "de cada clase"
        )
    return None


@dataclass(frozen=True)
class WindowResult:
    window: Window
    n_train: int
    validation: tuple[Example, ...]
    # Motivo por el que no entrenó; `None` si entrenó.
    skipped: str | None
    l2: float | None = None
    predictions: dict[str, list[float]] = field(default_factory=dict)
    # Lo que la purga y el embargo sacaron del entrenamiento (§5).
    purged: int = 0
    embargoed: int = 0


@dataclass(frozen=True)
class Scores:
    log_loss: float
    brier: float
    auc: float | None


@dataclass(frozen=True)
class Evaluation:
    windows: tuple[WindowResult, ...]
    # Por modelo, sobre todas las validaciones de las ventanas que entrenaron.
    pooled: dict[str, Scores]

    @property
    def suspended(self) -> bool:
        """Ninguna ventana llegó al mínimo: ML v1 en suspenso, se muestra B1 (§6)."""
        return not self.pooled


def evaluate(
    folds: Sequence[Fold], cfg: TrainingConfig, probability: ProbabilityConfig
) -> Evaluation:
    results: list[WindowResult] = []
    for f in folds:
        reason = minimum_reason(f.train, cfg)
        if reason is None and not f.validation:
            reason = "sin propuestas de validación"
        if reason is not None:
            results.append(
                WindowResult(
                    f.window,
                    len(f.train),
                    f.validation,
                    reason,
                    purged=f.purged,
                    embargoed=f.embargoed,
                )
            )
            continue
        m1 = fit_with_inner_tuning(
            f.train, cfg.l2_grid, cfg.inner_tune_fraction, cfg.seed, cfg.max_iterations
        )
        preds = {
            "B0": B0.fit(f.train).predict(f.validation),
            "B1": B1.fit(f.train, probability).predict(f.validation),
            "M1": m1.predict(f.validation),
        }
        results.append(
            WindowResult(
                f.window,
                len(f.train),
                f.validation,
                None,
                m1.l2,
                preds,
                purged=f.purged,
                embargoed=f.embargoed,
            )
        )
    trained = [r for r in results if r.skipped is None]
    pooled: dict[str, Scores] = {}
    if trained:
        y = [e.y for r in trained for e in r.validation]
        for name in MODELS:
            p = [q for r in trained for q in r.predictions[name]]
            pooled[name] = Scores(log_loss(y, p), brier(y, p), auc(y, p))
    return Evaluation(tuple(results), pooled)
