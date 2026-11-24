"""Un experimento completo de ML v1 sobre una corrida de backtest (docs/spec/ml-v1.md, #158).

```text
python -m cfds_intel.models.experiment <run_id>      # con DATABASE_URL
python -m cfds_intel.models.experiment <run_id> --fuente futuros --sin-registrar \
    --informe docs/analysis/<informe>.md            # #400: sesión de solo lectura
```

- `--fuente futuros`: las propuestas del backtest con futuros (`US500.FUT`,
  `config/candle_sources.toml`). Por omisión, las del CFD. Nunca se mezclan.
- `--sin-registrar`: no guarda en `ml_experiments` y abre la sesión de **solo lectura**.
- `--informe`: escribe el informe en Markdown (`cfds_intel.models.report`).

Dataset → cortes walk-forward (fuera de muestra nunca entregado) → B0, B1 y M1 por ventana →
regla de mejora de §6 → registro en `ml_experiments`. Se registra también si queda en suspenso
(ninguna ventana llega al mínimo): en ese caso se sigue mostrando B1.
"""

import argparse
import os
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import psycopg

from cfds_intel.analogs.data import futures_suffix
from cfds_intel.models.baselines import load_probability_config
from cfds_intel.models.compare import compare
from cfds_intel.models.config import ROOT, MlConfig, load_config
from cfds_intel.models.dataset import (
    BacktestOutcomes,
    Dataset,
    LabelSource,
    MlLabels,
    load_from,
)
from cfds_intel.models.metrics import log_losses
from cfds_intel.models.registry import document, record, sha256_hex
from cfds_intel.models.report import render
from cfds_intel.models.splits import folds, load_embargo, load_split_config, plan, research
from cfds_intel.models.train import Evaluation, evaluate

CONFIG_FILES = (
    "ml.toml",
    "probability.toml",
    "data_split.toml",
    "backtest.toml",
    "candle_sources.toml",
)
SOURCES = ("cfd", "futuros")


def code_version() -> str:
    """Commit de git, con `-dirty` si hay cambios sin commitear. Sin git, es un error: un
    experimento sin versión no se puede reproducir."""

    def git(*args: str) -> str:
        # Comando fijo, sin entrada del usuario.
        cmd = ["git", "-C", str(ROOT), *args]
        return subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip()  # noqa: S603

    dirty = git("status", "--porcelain")
    return git("rev-parse", "HEAD") + ("-dirty" if dirty else "")


def run(
    conn: psycopg.Connection,
    labels: LabelSource,
    cfg: MlConfig,
    texts: dict[str, str],
    source: str = "cfd",
) -> tuple[Dataset, Evaluation, dict[str, Any], int]:
    """Corre el experimento y arma su documento (sin guardarlo). `texts`: los archivos de
    `CONFIG_FILES`, por nombre. Devuelve también cuántas propuestas quedaron en el tramo fuera de
    muestra (solo el número: no se miran)."""
    if source not in SOURCES:
        raise ValueError(f"fuente {source!r}: tiene que ser una de {SOURCES}")
    suffix = futures_suffix(texts["candle_sources.toml"]) if source == "futuros" else None
    dataset = load_from(conn, labels, cfg.dataset, suffix)
    first, last = labels.span(conn)
    split = plan(first, last, load_split_config(texts["data_split.toml"]))
    fs = folds(dataset.examples, split, load_embargo(texts["backtest.toml"]))
    probability = load_probability_config(texts["probability.toml"])
    evaluation = evaluate(fs, cfg.training, probability)
    comparison = None
    if not evaluation.suspended:
        trained = [r for r in evaluation.windows if r.skipped is None]
        val = [e for r in trained for e in r.validation]
        y = [e.y for e in val]
        m1 = log_losses(y, [q for r in trained for q in r.predictions["M1"]])
        b1 = log_losses(y, [q for r in trained for q in r.predictions["B1"]])
        comparison = compare(
            val,
            m1,
            b1,
            cfg.evaluation.bootstrap_resamples,
            cfg.evaluation.confidence,
            cfg.training.seed,
        )
    doc = document(
        created_at=datetime.now(UTC),
        code_version=code_version(),
        backtest_run_id=labels.ref,
        config_sha256=sha256_hex(*(f"{n}\n{texts[n]}" for n in sorted(CONFIG_FILES))),
        examples=dataset.examples,
        feature_names=dataset.feature_names,
        cfg=cfg,
        evaluation=evaluation,
        comparison=comparison,
    )
    excluded = len(dataset.examples) - len(research(dataset.examples, split))
    return dataset, evaluation, doc, excluded


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="python -m cfds_intel.models.experiment")
    parser.add_argument("run_id")
    parser.add_argument("--fuente", choices=SOURCES, default="cfd")
    parser.add_argument(
        "--etiquetas",
        choices=("backtest", "sin-pl"),
        default="backtest",
        help="backtest: outcomes de backtest_runs (#200); sin-pl: ml_labels de ml_dataset (#399)",
    )
    parser.add_argument("--sin-registrar", action="store_true")
    parser.add_argument("--informe", type=Path)
    args = parser.parse_args(argv)
    texts = {n: (ROOT / "config" / n).read_text() for n in CONFIG_FILES}
    cfg = load_config(texts["ml.toml"])
    options = "-c default_transaction_read_only=on" if args.sin_registrar else ""
    labels: LabelSource = (
        MlLabels(args.run_id) if args.etiquetas == "sin-pl" else BacktestOutcomes(args.run_id)
    )
    with psycopg.connect(os.environ["DATABASE_URL"], options=options) as conn:
        dataset, evaluation, doc, excluded = run(conn, labels, cfg, texts, args.fuente)
        new = None if args.sin_registrar else record(conn, doc)
    print(
        f"{len(dataset.examples)} propuestas ({dataset.without_features} sin variables, "
        f"{dataset.without_outcome} sin resultado, {dataset.other_instrument} de otro instrumento "
        f"o fuente, {excluded} fuera de muestra sin mirar)"
    )
    for r in evaluation.windows:
        state = r.skipped or f"entrenó (L2 {r.l2})"
        print(f"  {r.window.validation.start:%Y-%m}: {r.n_train} de entrenamiento — {state}")
    stored = "sin registrar" if new is None else ("nuevo" if new else "ya estaba")
    print(
        f"resultado: {doc['result']['status']} — experimento {str(doc['experiment_id'])[:12]} "
        f"({stored})"
    )
    if args.informe is not None:
        args.informe.write_text(
            render(
                title=f"ML v1: evaluación walk-forward ({', '.join(cfg.dataset.instruments)})",
                source=args.fuente,
                dataset=dataset,
                out_of_sample_excluded=excluded,
                evaluation=evaluation,
                doc=doc,
                calibration_bins=cfg.evaluation.calibration_bins,
            )
        )
        print(f"informe: {args.informe}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
