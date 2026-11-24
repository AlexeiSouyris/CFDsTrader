/**
 * Experimentos del modelo de ML v1 desde el gateway (#426, fase 4b), solo `GET`:
 *
 * | Ruta | Respuesta |
 * |---|---|
 * | `GET /api/v1/ml/experiments` | `ml_experiment_entry.v1[]`: el último experimento de cada instrumento |
 *
 * Los entrena y guarda Python (`cfds_intel.models`); el gateway los devuelve tal cual. La UI no
 * calcula métricas ni decide nada: muestra lo que llega. El modelo es **solo informativo**.
 * Sin gateway no hay datos de ejemplo.
 */
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'

type Utc = string

/** `ml_experiment.v1` `$defs.scores`: números, no dinero. */
export type MlScores = { log_loss: number; brier: number; auc: number | null }

export const ML_MODELS = ['B0', 'B1', 'M1'] as const
export type MlModel = (typeof ML_MODELS)[number]
export type MlByModel = Partial<Record<MlModel, MlScores>>

export type MlStatus = 'suspended' | 'm1_improves' | 'b1_stays'

/** `contracts/ml_experiment.v1`. */
export type MlExperiment = {
  experiment_id: string
  created_at: Utc
  code_version: string
  backtest_run_id: string
  label_source?: 'backtest_runs' | 'ml_label_runs'
  config_sha256: string
  features: string[]
  hyperparameters: {
    l2_grid: number[]
    inner_tune_fraction: number
    max_iterations: number
    seed: number
    min_examples: number
    min_per_class: number
    bootstrap_resamples: number
    confidence: number
  }
  windows: {
    train_start: Utc
    validation_start: Utc
    validation_end: Utc
    n_train: number
    n_validation: number
    purged: number
    embargoed: number
    skipped: string | null
    l2: number | null
    metrics: MlByModel
  }[]
  pooled: MlByModel
  result: {
    status: MlStatus
    /** Log loss de M1 menos el de B1. */
    difference: number | null
    low: number | null
    high: number | null
    blocks: number | null
  }
}

/** `contracts/ml_experiment_entry.v1`: el experimento con el instrumento de su corrida. */
export type MlExperimentEntry = {
  instrument_id: string
  label_source: 'backtest_runs' | 'ml_label_runs'
  experiment: MlExperiment
}

export async function fetchMlExperiments(signal?: AbortSignal): Promise<MlExperimentEntry[]> {
  const res = await fetch('/api/v1/ml/experiments', {
    signal,
    headers: { Accept: 'application/json' },
  })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as MlExperimentEntry[]
}

export const useMlExperiments = () => useRemote('ml-experiments', fetchMlExperiments, REFRESH_MS)
