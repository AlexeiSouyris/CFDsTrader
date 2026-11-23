"""Un experimento completo de ML v1 sobre una corrida de backtest (docs/spec/ml-v1.md, #158).

```text
python -m cfds_intel.models.experiment <run_id>      # con DATABASE_URL
```

Dataset → cortes walk-forward (fuera de muestra nunca entregado) → B0, B1 y M1 por ventana →
regla de mejora de §6 → registro en `ml_experiments`. Se registra también si queda en suspenso
(ninguna ventana llega al mínimo): en ese caso se sigue mostrando B1.
"""

import os
import subprocess
import sys
from datetime import UTC, datetime
from typing import Any

import psycopg

from cfds_intel.models.baselines import load_probability_config
from cfds_intel.models.compare import compare
from cfds_intel.models.config import ROOT, MlConfig, load_config
from cfds_intel.models.dataset import Dataset, load
from cfds_intel.models.metrics import log_losses
from cfds_intel.models.registry import document, record, sha256_hex
from cfds_intel.models.splits import folds, load_embargo, load_split_config, plan
from cfds_intel.models.train import Evaluation, evaluate

CONFIG_FILES = ("ml.toml", "probability.toml", "data_split.toml", "backtest.toml")


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
    conn: psycopg.Connection, run_id: str, cfg: MlConfig, texts: dict[str, str]
) -> tuple[Dataset, Evaluation, dict[str, Any]]:
    """Corre el experimento y arma su documento (sin guardarlo). `texts`: los archivos de
    `CONFIG_FILES`, por nombre."""
    dataset = load(conn, run_id, cfg.dataset)
    with conn.cursor() as cur:
        cur.execute("SELECT from_at, to_at FROM backtest_runs WHERE run_id = %s", (run_id,))
        row = cur.fetchone()
    if row is None:
        raise ValueError(f"la corrida {run_id!r} no está en backtest_runs")
    split = plan(row[0], row[1], load_split_config(texts["data_split.toml"]))
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
        backtest_run_id=run_id,
        config_sha256=sha256_hex(*(f"{n}\n{texts[n]}" for n in sorted(CONFIG_FILES))),
        examples=dataset.examples,
        feature_names=dataset.feature_names,
        cfg=cfg,
        evaluation=evaluation,
        comparison=comparison,
    )
    return dataset, evaluation, doc


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print("uso: python -m cfds_intel.models.experiment <run_id>", file=sys.stderr)
        return 2
    texts = {n: (ROOT / "config" / n).read_text() for n in CONFIG_FILES}
    cfg = load_config(texts["ml.toml"])
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        dataset, evaluation, doc = run(conn, argv[0], cfg, texts)
        new = record(conn, doc)
    print(
        f"{len(dataset.examples)} propuestas ({dataset.without_features} sin variables, "
        f"{dataset.without_outcome} sin resultado, {dataset.other_instrument} de otro instrumento)"
    )
    for r in evaluation.windows:
        state = r.skipped or f"entrenó (L2 {r.l2})"
        print(f"  {r.window.validation.start:%Y-%m}: {r.n_train} de entrenamiento — {state}")
    result = doc["result"]
    print(
        f"resultado: {result['status']} — experimento {str(doc['experiment_id'])[:12]} "
        f"({'nuevo' if new else 'ya estaba'})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
