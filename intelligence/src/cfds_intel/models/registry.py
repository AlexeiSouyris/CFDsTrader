"""Registro de experimentos de ML v1 (docs/spec/ml-v1.md §7, `contracts/ml_experiment.v1`).

Se registra cada entrenamiento, también los que no funcionan o quedan en suspenso, para el
Sharpe deflactado y el PBO de la fase 5. La tabla `ml_experiments` es de solo agregar: el mismo
experimento (mismo código, corrida, configuración y datos) tiene el mismo id y no se repite.
"""

import hashlib
import json
from collections.abc import Sequence
from datetime import datetime
from typing import Any

import psycopg

from cfds_intel.models.compare import Comparison
from cfds_intel.models.config import MlConfig
from cfds_intel.models.dataset import Example
from cfds_intel.models.metrics import auc, brier, log_loss
from cfds_intel.models.train import MODELS, Evaluation, Scores


def _utc(t: datetime) -> str:
    return t.isoformat().replace("+00:00", "Z")


def sha256_hex(*parts: str) -> str:
    """SHA-256 de varios textos, cada uno con su largo: ninguna concatenación se confunde."""
    h = hashlib.sha256()
    for p in parts:
        b = p.encode()
        h.update(len(b).to_bytes(8, "big"))
        h.update(b)
    return h.hexdigest()


def dataset_fingerprint(examples: Sequence[Example]) -> str:
    return sha256_hex(*(json.dumps([e.proposal_id, e.y, list(e.x)]) for e in examples))


def _scores(s: Scores) -> dict[str, Any]:
    return {"log_loss": s.log_loss, "brier": s.brier, "auc": s.auc}


def document(
    *,
    created_at: datetime,
    code_version: str,
    backtest_run_id: str,
    config_sha256: str,
    examples: Sequence[Example],
    feature_names: Sequence[str],
    cfg: MlConfig,
    evaluation: Evaluation,
    comparison: Comparison | None,
) -> dict[str, Any]:
    """El documento de `ml_experiment.v1`."""
    windows = []
    for r in evaluation.windows:
        metrics: dict[str, Any] = {}
        if r.skipped is None:
            y = [e.y for e in r.validation]
            for name in MODELS:
                p = r.predictions[name]
                metrics[name] = _scores(Scores(log_loss(y, p), brier(y, p), auc(y, p)))
        windows.append(
            {
                "train_start": _utc(r.window.train.start),
                "validation_start": _utc(r.window.validation.start),
                "validation_end": _utc(r.window.validation.end),
                "n_train": r.n_train,
                "n_validation": len(r.validation),
                "purged": r.purged,
                "embargoed": r.embargoed,
                "skipped": r.skipped,
                "l2": r.l2,
                "metrics": metrics,
            }
        )
    if comparison is None:
        result: dict[str, Any] = {
            "status": "suspended",
            "difference": None,
            "low": None,
            "high": None,
            "blocks": None,
        }
    else:
        result = {
            "status": "m1_improves" if comparison.improves else "b1_stays",
            "difference": comparison.difference,
            "low": comparison.low,
            "high": comparison.high,
            "blocks": comparison.blocks,
        }
    t = cfg.training
    return {
        "experiment_id": sha256_hex(
            code_version, backtest_run_id, config_sha256, dataset_fingerprint(examples)
        ),
        "created_at": _utc(created_at),
        "code_version": code_version,
        "backtest_run_id": backtest_run_id,
        "config_sha256": config_sha256,
        "features": list(feature_names),
        "hyperparameters": {
            "l2_grid": list(t.l2_grid),
            "inner_tune_fraction": t.inner_tune_fraction,
            "max_iterations": t.max_iterations,
            "seed": t.seed,
            "min_examples": t.min_examples,
            "min_per_class": t.min_per_class,
            "bootstrap_resamples": cfg.evaluation.bootstrap_resamples,
            "confidence": cfg.evaluation.confidence,
        },
        "windows": windows,
        "pooled": {name: _scores(s) for name, s in evaluation.pooled.items()},
        "result": result,
    }


def record(conn: psycopg.Connection, doc: dict[str, Any]) -> bool:
    """Guarda el experimento. `True` si era nuevo; si ya estaba (mismo id), no guarda nada."""
    with conn.transaction(), conn.cursor() as cur:
        cur.execute(
            """INSERT INTO ml_experiments (experiment_id, created_at, code_version,
                 backtest_run_id, config_sha256, status, document)
               VALUES (%s, %s, %s, %s, %s, %s, %s)
               ON CONFLICT (experiment_id) DO NOTHING""",
            (
                doc["experiment_id"],
                datetime.fromisoformat(doc["created_at"].replace("Z", "+00:00")),
                doc["code_version"],
                doc["backtest_run_id"],
                doc["config_sha256"],
                doc["result"]["status"],
                json.dumps(doc),
            ),
        )
        return cur.rowcount == 1
