"""Variantes de ML v2 (#432), pre-registradas en docs/analysis/2026-11-ml-v2-preregistro.md.

```text
python -m cfds_intel.models.variants <run_id> --etiquetas sin-pl --fuente futuros \\
    [--sin-registrar] [--informe docs/analysis/<informe>.md]
```

Cada variante de la familia (`config/ml.toml` [variants]) es M1 con columnas nuevas, comparada
contra B1 **en sus mismas propuestas** con la regla de §6 y el intervalo corregido por
Bonferroni (1 - error de familia / K). Se corren todas las variantes de la familia, una vez cada
una, y cada una se registra en `ml_experiments` (salvo `--sin-registrar`, que abre la sesión de
solo lectura). De referencia, sin decidir nada: M1 sin la variante sobre las mismas propuestas.
El modelo es informativo: nada de esto cambia la estrategia.
"""

import argparse
import os
import sys
from collections.abc import Callable, Mapping
from dataclasses import dataclass, replace
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import psycopg

from cfds_intel.analogs.config import load_config as load_analogs_config
from cfds_intel.analogs.data import daily_params, futures_suffix, load_bars, select_source
from cfds_intel.models.compare import Comparison
from cfds_intel.models.config import ROOT, MlConfig, load_config
from cfds_intel.models.context import AnalogFeature, load_treasury, load_vix
from cfds_intel.models.dataset import (
    BacktestOutcomes,
    Dataset,
    Example,
    LabelSource,
    MlLabels,
    load_from,
)
from cfds_intel.models.experiment import CONFIG_FILES, SOURCES, assess, code_version
from cfds_intel.models.registry import document, record, sha256_hex
from cfds_intel.models.report import render
from cfds_intel.models.train import Evaluation

# Lo que la variante agrega a una propuesta: (signal_at, es largo) → valores, o None si falta.
Lookup = Callable[[datetime, bool], tuple[float, ...] | None]
VARIANT_FILES = (*CONFIG_FILES, "analogs.toml", "indicators.toml", "treasury.toml", "vix.toml")


@dataclass(frozen=True)
class Variant:
    name: str
    features: tuple[str, ...]
    lookup: Lookup


def augment(dataset: Dataset, variant: Variant) -> Dataset:
    """Las propuestas con todas las variables de la variante, con sus columnas al final. Las que
    no las tienen no entran y se cuentan aparte (spec §3: nunca se rellena)."""
    side = dataset.feature_names.index("side_long")
    kept: list[Example] = []
    missing = 0
    for e in dataset.examples:
        extra = variant.lookup(e.signal_at, e.x[side] == 1.0)
        if extra is None:
            missing += 1
            continue
        if len(extra) != len(variant.features):
            raise ValueError(f"{variant.name}: {len(extra)} valores para {variant.features}")
        kept.append(replace(e, x=(*e.x, *extra)))
    return replace(
        dataset,
        examples=tuple(kept),
        feature_names=(*dataset.feature_names, *variant.features),
        without_context=dataset.without_context + missing,
    )


def base_of(augmented: Dataset, base: Dataset) -> Dataset:
    """Las mismas propuestas de la variante, con solo las columnas de v1 (la referencia)."""
    n = len(base.feature_names)
    return replace(
        augmented,
        examples=tuple(replace(e, x=e.x[:n]) for e in augmented.examples),
        feature_names=base.feature_names,
    )


@dataclass(frozen=True)
class VariantResult:
    variant: Variant
    dataset: Dataset
    evaluation: Evaluation
    comparison: Comparison | None
    reference: Evaluation
    excluded: int
    doc: dict[str, Any]


def run_variant(
    base: Dataset,
    variant: Variant,
    span: tuple[datetime, datetime],
    cfg: MlConfig,
    texts: Mapping[str, str],
    labels: LabelSource,
) -> VariantResult:
    confidence = cfg.variants.confidence
    vcfg = replace(cfg, evaluation=replace(cfg.evaluation, confidence=confidence))
    dataset = augment(base, variant)
    evaluation, comparison, excluded = assess(dataset, *span, vcfg, dict(texts), confidence)
    reference, _, _ = assess(base_of(dataset, base), *span, vcfg, dict(texts), confidence)
    doc = document(
        created_at=datetime.now(UTC),
        code_version=code_version(),
        backtest_run_id=labels.ref,
        label_source=labels.table,
        config_sha256=sha256_hex(*(f"{n}\n{texts[n]}" for n in sorted(texts))),
        examples=dataset.examples,
        feature_names=dataset.feature_names,
        cfg=vcfg,
        evaluation=evaluation,
        comparison=comparison,
    )
    return VariantResult(variant, dataset, evaluation, comparison, reference, excluded, doc)


def treasury_variant(conn: psycopg.Connection, cfg: MlConfig) -> Variant:
    tenors = cfg.variants.treasury_tenors
    series = load_treasury(conn, tenors)
    age = cfg.variants.max_age_days

    def lookup(t: datetime, long: bool) -> tuple[float, ...] | None:
        values = [series[k].at(t, age) for k in tenors]
        # Un rendimiento de 0 % es un dato (2020-2021): solo `None` falta.
        present = [v for v in values if v is not None]
        return tuple(float(v) for v in present) if len(present) == len(values) else None

    return Variant("tasas", tuple(f"yield_{k}" for k in tenors), lookup)


def vix_variant(conn: psycopg.Connection, cfg: MlConfig) -> Variant:
    series = load_vix(conn)
    age = cfg.variants.max_age_days

    def lookup(t: datetime, long: bool) -> tuple[float, ...] | None:
        v = series.at(t, age)
        return None if v is None else (float(v),)

    return Variant("vix", ("vix_close",), lookup)


def analog_variant(
    conn: psycopg.Connection, texts: Mapping[str, str], suffix: str | None, instrument: str
) -> Variant:
    acfg = load_analogs_config(texts["analogs.toml"])
    atr, vol, years = daily_params(texts["indicators.toml"])
    series, _ = select_source(load_bars(conn, atr, vol, years), {}, suffix)
    feature = AnalogFeature(series, instrument + (suffix or ""), acfg)

    def lookup(t: datetime, long: bool) -> tuple[float, ...] | None:
        v = feature.at(t, long)
        return None if v is None else (v,)

    return Variant("analogos", ("analog_hit_share",), lookup)


def _pct(v: float) -> str:
    return f"{100 * v:.2f} %"


def render_family(results: list[VariantResult], cfg: MlConfig, source: str) -> str:
    """Informe de la familia: la tabla de decisión y, por variante, el informe completo."""
    k = len(cfg.variants.family)
    lines = [
        "# ML v2: variantes de US500 (#432)",
        "",
        "> **Modelo informativo** (spec §2): no suma al puntaje ni filtra propuestas. Las"
        " probabilidades son estimaciones con incertidumbre, nunca certezas. Regla y familia"
        " pre-registradas en `docs/analysis/2026-11-ml-v2-preregistro.md`.",
        "",
        f"Familia: {', '.join(cfg.variants.family)} (K = {k}). Bonferroni: cada intervalo al"
        f" {_pct(cfg.variants.confidence)} (error de familia {_pct(cfg.variants.family_error)}).",
        "",
        "| Variante | Propuestas | Sin la variable | Ventanas | Validación | Log loss B1 |"
        " M1 sin variante | M1 con variante | AUC M1 con variante | Variante - B1 (IC) |"
        " Veredicto |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|",
    ]
    for r in results:
        trained = [w for w in r.evaluation.windows if w.skipped is None]
        n_val = sum(len(w.validation) for w in trained)
        if r.evaluation.suspended or r.comparison is None:
            cells = ["—"] * 5 + ["en suspenso"]
        else:
            p, ref, c = r.evaluation.pooled, r.reference.pooled, r.comparison
            auc = "—" if p["M1"].auc is None else f"{p['M1'].auc:.3f}"
            cells = [
                f"{p['B1'].log_loss:.4f}",
                f"{ref['M1'].log_loss:.4f}" if "M1" in ref else "—",
                f"{p['M1'].log_loss:.4f}",
                auc,
                f"{c.difference:+.4f} ({c.low:+.4f} a {c.high:+.4f})",
                "**mejora a B1**" if c.improves else "se queda B1",
            ]
        lines.append(
            f"| {r.variant.name} | {len(r.dataset.examples)} | {r.dataset.without_context} |"
            f" {len(trained)}/{len(r.evaluation.windows)} | {n_val} | " + " | ".join(cells) + " |"
        )
    lines.append("")
    for r in results:
        body = render(
            title=f"Variante `{r.variant.name}`",
            source=source,
            dataset=r.dataset,
            out_of_sample_excluded=r.excluded,
            evaluation=r.evaluation,
            doc=r.doc,
            calibration_bins=cfg.evaluation.calibration_bins,
        )
        lines += ["#" + ln if ln.startswith("#") else ln for ln in body.splitlines()]
        lines.append("")
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="python -m cfds_intel.models.variants")
    parser.add_argument("run_id")
    parser.add_argument("--fuente", choices=SOURCES, default="cfd")
    parser.add_argument("--etiquetas", choices=("backtest", "sin-pl"), default="backtest")
    parser.add_argument("--sin-registrar", action="store_true")
    parser.add_argument("--informe", type=Path)
    args = parser.parse_args(argv)
    texts = {n: (ROOT / "config" / n).read_text() for n in VARIANT_FILES}
    cfg = load_config(texts["ml.toml"])
    if len(cfg.dataset.instruments) != 1:
        print("las variantes de v2 son de un solo instrumento (#432)", file=sys.stderr)
        return 2
    suffix = futures_suffix(texts["candle_sources.toml"]) if args.fuente == "futuros" else None
    labels: LabelSource = (
        MlLabels(args.run_id) if args.etiquetas == "sin-pl" else BacktestOutcomes(args.run_id)
    )
    options = "-c default_transaction_read_only=on" if args.sin_registrar else ""
    results: list[VariantResult] = []
    with psycopg.connect(os.environ["DATABASE_URL"], options=options) as conn:
        base = load_from(conn, labels, cfg.dataset, suffix)
        span = labels.span(conn)
        builders: dict[str, Callable[[], Variant]] = {
            "tasas": lambda: treasury_variant(conn, cfg),
            "vix": lambda: vix_variant(conn, cfg),
            "analogos": lambda: analog_variant(conn, texts, suffix, cfg.dataset.instruments[0]),
        }
        for name in cfg.variants.family:
            r = run_variant(base, builders[name](), span, cfg, texts, labels)
            stored = (
                "sin registrar"
                if args.sin_registrar
                else ("nuevo" if record(conn, r.doc) else "ya estaba")
            )
            print(
                f"{name}: {len(r.dataset.examples)} propuestas ({r.dataset.without_context} sin"
                f" la variable) — {r.doc['result']['status']} — experimento"
                f" {str(r.doc['experiment_id'])[:12]} ({stored})"
            )
            results.append(r)
    if args.informe is not None:
        args.informe.write_text(render_family(results, cfg, args.fuente))
        print(f"informe: {args.informe}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
