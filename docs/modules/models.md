---
modulo: models
ruta: `intelligence/cfds_intel/models`
lenguaje: Python
estado: en curso (#399)
resumen: Modelos PyTorch de predicción y su entrenamiento (ML v1, solo informativo).
---

# models

## Responsabilidad (única)

- Modelos PyTorch de predicción y su entrenamiento. ML v1 (`docs/spec/ml-v1.md`): probabilidad de TP1 antes que el stop, solo informativa. Hoy: dataset X/y desde una fuente de etiquetas intercambiable (`LabelSource`: `BacktestOutcomes`, una corrida de `backtest_runs` con sus `proposals`, `outcomes` y `ml_features`, o `MlLabels`, una corrida de `ml_label_runs` con sus `proposals`, `ml_labels` y `ml_features`), con la fuente de velas del CFD o de futuros (`US500.FUT`) sin mezclarlas (`dataset.py`, config en `config/ml.toml`)
- cortes walk-forward de §5 con purga, embargo, fuera de muestra nunca entregado y corte interno para hiperparámetros, con el mismo plan que `cfds_market_data::split` (fixture compartido) (`splits.py`)
- comparadores B0 y B1 (la tabla §13, `baselines.py`), M1 regresión logística L2 en PyTorch con estandarización solo del entrenamiento y L2 elegida en el corte interno (`logistic.py`), log loss, Brier, AUC y calibración con Wilson (`metrics.py`) y evaluación walk-forward con el mínimo de 300 propuestas y 50 por clase (`train.py`)
- en el experimento, `--etiquetas sin-pl` elige `MlLabels` (#399)
- regla de mejora de M1 sobre B1 con bootstrap por bloques de una semana (`compare.py`) e intervalo de cada predicción con 200 modelos bootstrap, o "datos insuficientes" (`intervals.py`)
- registro de experimentos `ml_experiment.v1` en `ml_experiments`, de solo agregar (`registry.py`), y el experimento completo sobre una corrida: `python -m cfds_intel.models.experiment <run_id> [--fuente futuros] [--sin-registrar] [--informe <md>]` (`experiment.py`), con informe en Markdown (`report.py`)
- variantes de ML v2 pre-registradas (#432): contexto a la hora de la propuesta (Tesoro y VIX publicados, sin valores viejos; análogos con diarias cerradas, `context.py`) y la familia con Bonferroni contra B1, un registro por variante: `python -m cfds_intel.models.variants` (`variants.py`)

## Estado

- pipeline de ML v1 hecho (#400), sin datos reales para entrenar
- falta el productor de `ml_features` y la etiqueta sin P/L (#399)
