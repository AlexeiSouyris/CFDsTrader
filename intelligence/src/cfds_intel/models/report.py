"""Informe en Markdown de un experimento de ML v1 (docs/spec/ml-v1.md §6, #400).

Solo muestra lo que ya calcularon el experimento y sus métricas: el conjunto de las validaciones
(nunca una sola), B0 (la tasa base del entrenamiento: el modelo trivial), B1 (la tabla de §13, el
rival) y M1, la regla de mejora y la calibración por grupos de la predicción con su intervalo de
Wilson. Del tramo fuera de muestra solo dice **cuántas** propuestas quedaron afuera: no se mira
(§5). El modelo es informativo (§2): nada de esto cambia la estrategia ni las propuestas.
"""

from collections import Counter
from collections.abc import Sequence
from typing import Any

from cfds_intel.models.dataset import Dataset, Example
from cfds_intel.models.metrics import calibration, log_loss
from cfds_intel.models.train import MODELS, Evaluation

NAMES = {
    "B0": "B0: tasa base del entrenamiento (trivial)",
    "B1": "B1: tabla de §13 (el rival)",
    "M1": "M1: regresión logística L2 (PyTorch)",
}
STATUS = {
    "suspended": "**en suspenso**: ninguna ventana llegó al mínimo; se sigue mostrando B1",
    "b1_stays": "**se queda B1**: M1 no cumple la regla de mejora",
    "m1_improves": "**M1 mejora a B1** en validación (falta la evaluación fuera de muestra)",
}


def _pct(v: float) -> str:
    return f"{100 * v:.1f} %"


def _num(v: float | None, digits: int = 4) -> str:
    return "—" if v is None else f"{v:.{digits}f}"


def render(
    *,
    title: str,
    source: str,
    dataset: Dataset,
    out_of_sample_excluded: int,
    evaluation: Evaluation,
    doc: dict[str, Any],
    calibration_bins: int,
) -> str:
    """El informe completo. `doc`: el documento de `ml_experiment.v1` del mismo experimento."""
    lines = [
        f"# {title}",
        "",
        "> Generado por `python -m cfds_intel.models.experiment` (#400). **Modelo informativo**"
        " (spec §2): no suma al puntaje ni filtra propuestas. Las probabilidades son estimaciones"
        " con incertidumbre, nunca certezas.",
        "",
        "## Experimento",
        "",
        f"- Corrida de backtest: `{doc['backtest_run_id']}` (fuente: {source})",
        f"- Versión del código: `{doc['code_version']}`",
        f"- Experimento: `{str(doc['experiment_id'])[:12]}`"
        f" (configuración `{str(doc['config_sha256'])[:12]}`)",
        f"- Variables ({len(dataset.feature_names)}): "
        + ", ".join(f"`{n}`" for n in dataset.feature_names),
        "",
        "## Datos",
        "",
        f"- Propuestas con resultado y variables: **{len(dataset.examples)}**",
        f"- Sin variables (calentamiento, no se rellenan): {dataset.without_features}",
        f"- Sin resultado: {dataset.without_outcome}",
        f"- De otro instrumento o de otra fuente: {dataset.other_instrument}",
        f"- Fuera de muestra (decididas o resueltas en ese tramo, **no se miran**):"
        f" {out_of_sample_excluded}",
        "",
    ]
    lines += _windows_summary(evaluation)
    lines += _pooled(evaluation, doc)
    lines += _calibration(evaluation, calibration_bins)
    lines += _per_window(evaluation)
    return "\n".join(lines) + "\n"


def _windows_summary(evaluation: Evaluation) -> list[str]:
    trained = [r for r in evaluation.windows if r.skipped is None]
    out = [
        "## Ventanas walk-forward",
        "",
        f"- Ventanas: {len(evaluation.windows)}; entrenaron: **{len(trained)}**",
    ]
    reasons = Counter(r.skipped for r in evaluation.windows if r.skipped is not None)
    for reason, n in sorted(reasons.items()):
        out.append(f"- No entrenaron ({n}): {reason}")
    if trained:
        l2 = Counter(r.l2 for r in trained)
        out.append(
            "- L2 elegida en el corte interno: "
            + ", ".join(f"{k:g} ({n})" for k, n in sorted(l2.items(), key=lambda kv: kv[0] or 0))
        )
    return [*out, ""]


def _pooled(evaluation: Evaluation, doc: dict[str, Any]) -> list[str]:
    out = ["## Resultado (todas las validaciones juntas)", ""]
    result = doc["result"]
    out.append(f"Veredicto (regla de §6, fijada antes de mirar): {STATUS[result['status']]}.")
    out.append("")
    if evaluation.suspended:
        return out
    trained = [r for r in evaluation.windows if r.skipped is None]
    y = [e.y for r in trained for e in r.validation]
    out += [
        f"- Propuestas de validación: {len(y)}; con TP1 (`y = 1`): {sum(y)}"
        f" ({_pct(sum(y) / len(y))}, solo de referencia: no es un modelo)",
        f"- M1 - B1 en log loss: {_num(result['difference'])}"
        f" (IC {_num(result['low'])} a {_num(result['high'])},"
        f" bootstrap por bloques de una semana, {result['blocks']} semanas)",
        "",
        "| Modelo | Log loss | Brier | AUC |",
        "|---|---:|---:|---:|",
    ]
    for name in MODELS:
        s = evaluation.pooled[name]
        out.append(f"| {NAMES[name]} | {_num(s.log_loss)} | {_num(s.brier)} | {_num(s.auc, 3)} |")
    return [*out, ""]


def _calibration(evaluation: Evaluation, bins: int) -> list[str]:
    if evaluation.suspended:
        return []
    trained = [r for r in evaluation.windows if r.skipped is None]
    y = [e.y for r in trained for e in r.validation]
    out = [
        "## Calibración",
        "",
        f"Validaciones juntas, en {bins} grupos iguales por orden de la predicción; la frecuencia"
        " observada lleva su intervalo de Wilson del 95 %.",
        "",
    ]
    for name in MODELS:
        p = [q for r in trained for q in r.predictions[name]]
        out += [
            f"### {NAMES[name]}",
            "",
            "| Grupo | N | Predicción media | Observada | Intervalo |",
            "|---:|---:|---:|---:|---|",
        ]
        for i, b in enumerate(calibration(y, p, bins), start=1):
            out.append(
                f"| {i} | {b.n} | {_pct(b.mean_predicted)} | {_pct(b.observed)} |"
                f" {_pct(b.low)} a {_pct(b.high)} |"
            )
        out.append("")
    return out


def _per_window(evaluation: Evaluation) -> list[str]:
    out = [
        "## Por ventana",
        "",
        "Solo para ver la estabilidad: la decisión sale del conjunto, nunca de una ventana.",
        "",
        "| Validación | Entren. | Purga | Embargo | Valid. | y = 1 | L2 | Log loss B0 | B1 | M1 |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for r in evaluation.windows:
        v = r.window.validation
        period = f"{v.start:%Y-%m} a {v.end:%Y-%m}"
        rate = _pct(sum(e.y for e in r.validation) / len(r.validation)) if r.validation else "—"
        if r.skipped is not None:
            cells = ["—", f"no entrenó: {r.skipped}", "", ""]
        else:
            cells = [f"{r.l2:g}", *(_ll(r.validation, r.predictions[m]) for m in MODELS)]
        out.append(
            f"| {period} | {r.n_train} | {r.purged} | {r.embargoed} | {len(r.validation)} |"
            f" {rate} | " + " | ".join(cells) + " |"
        )
    return [*out, ""]


def _ll(validation: Sequence[Example], p: Sequence[float]) -> str:
    return _num(log_loss([e.y for e in validation], p))
