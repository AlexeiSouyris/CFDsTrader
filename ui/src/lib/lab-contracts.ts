/**
 * Tipos de los contratos del laboratorio de estrategias (#710, #778) para la UI:
 * `strategy_catalog.v1`, `strategy_spec.v1`, `spec_validation.v1`, `lab_job_request.v1`,
 * `lab_job_event.v1` y `lab_report.v1`. Son la misma forma del contrato: si cambia el contrato,
 * se cambia acá. Números como texto decimal (sin float); `null` cuando el motor no lo define.
 */

/** Texto decimal (sin float), como en el contrato. */
export type Decimal = string
/** Instante UTC con zona, como "2026-11-20T21:00:00Z". */
export type Utc = string

export type LabTimeframe = '15m' | '1h'
export type LabMode = 'exploratory' | 'confirmatory'
export type Slot = 'entry' | 'filter' | 'stop' | 'exit'

/** Un parámetro de un bloque del catálogo (`kind` y su rango inclusivo). */
export type ParamSchema = { name: string; label: string; required: boolean } & (
  | { kind: 'integer'; min: number; max: number; default: number | null }
  | { kind: 'decimal'; min: Decimal; max: Decimal; default: Decimal | null }
  | { kind: 'time'; default: string | null }
  | { kind: 'enum'; options: string[]; default: string | null }
  | { kind: 'boolean'; default: boolean | null }
)

export type BlockSchema = {
  slot: Slot
  type: string
  label: string
  description: string
  timeframes: LabTimeframe[]
  params: ParamSchema[]
}

/** `strategy_catalog.v1`. */
export type StrategyCatalog = {
  catalog_version: number
  limits: {
    max_entry_conditions: number
    max_filters: number
    max_risk_pct: Decimal
    min_cost_multiplier: Decimal
    max_cost_multiplier: Decimal
    max_variants_per_run: number
  }
  markets: { instrument_id: string; source: 'futures'; timeframes: LabTimeframe[] }[]
  modes: LabMode[]
  blocks: BlockSchema[]
}

export type ParamValue = number | boolean | string

export type SpecBlock = { type: string; params: Record<string, ParamValue> }

/** `strategy_spec.v1`. */
export type StrategySpec = {
  catalog_version: number
  market: { instrument_id: string; source: 'futures'; timeframe: LabTimeframe }
  direction: 'long' | 'short' | 'both'
  entry: SpecBlock[]
  filters: SpecBlock[]
  stop: SpecBlock
  exit: SpecBlock
  risk_pct: Decimal
  cost_multiplier: Decimal
  period: { from: Utc; to: Utc }
  mode: LabMode
  preregistration_sha256?: string
}

/** `spec_validation.v1`. */
export type SpecValidation = {
  valid: boolean
  catalog_version: number
  spec_sha256: string | null
  family_sha256: string | null
  errors: { code: string; path: string; message: string }[]
}

/** `lab_job_request.v1`. */
export type LabJobRequest = { specs: StrategySpec[]; initial_equity: Decimal }

export type JobState = 'queued' | 'running' | 'done' | 'failed' | 'cancelled'
export type JobStage =
  'started' | 'loading_candles' | 'running_variants' | 'luck_test' | 'building_reports' | 'saving'

/** `lab_job_event.v1`. */
export type LabJobEvent = {
  job_id: string
  seq: number
  state: JobState
  stage: JobStage | null
  percent: number
  variants_done: number
  variants_total: number
  report_ids: string[]
  error: string | null
  recorded_at: Utc
}

type Interval = { low: Decimal; high: Decimal }
/** Media y cuantiles 5, 50 y 95 % sobre los caminos o combinaciones. */
export type Summary = { mean: Decimal; p5: Decimal; p50: Decimal; p95: Decimal }
export type Quantiles = { p25: Decimal; p50: Decimal; p75: Decimal; p90: Decimal }
export type Slice = { key: string; trades: number; mean_r_net: Decimal; win_rate: Decimal }

/** `lab_report.v1`. */
export type LabReport = {
  report_id: string
  spec_sha256: string
  family_sha256: string
  spec: StrategySpec
  mode: LabMode
  votes: boolean
  label: string
  instrument_id: string
  candle_source: 'futures'
  period: { from: Utc; to: Utc }
  holdout_start: Utc
  reproducibility: {
    config_sha256: string
    code_version: string | null
    last_candle: Utc
    seed: number
  }
  computed_at: Utc
  initial_equity: Decimal
  cost_multiplier: Decimal
  trades: {
    count: number
    long: number
    short: number
    exits: { stop: number; stop_gap: number; target: number; planned: number; force_close: number }
    rejected: { reason: string; count: number }[]
    through_stop: number
  }
  expectancy: null | {
    mean_r_net: Decimal
    mean_r_net_ci: Interval | null
    mean_r_gross: Decimal
    win_rate: Decimal
    win_rate_ci: Interval
    profit_factor: Decimal | null
    confidence: Decimal
  }
  r_distribution: {
    step: Decimal
    bins: { from: Decimal; to: Decimal; net: number; gross: number }[]
  }
  months: {
    month: string
    trades: number
    return: Decimal
    return_stressed: Decimal
    equity: Decimal
    drawdown: Decimal
    buy_and_hold: Decimal | null
  }[]
  sharpe: {
    monthly_mean: Decimal | null
    sharpe: Decimal | null
    sortino: Decimal | null
    deflated: Decimal | null
    project_trials: number
    family_variants: number
    trials: number
  }
  pbo: null | {
    variants: number
    partitions: number
    pbo: Decimal
    combinations: number
    logits: Summary
    oos_loss: Decimal
  }
  monte_carlo: null | {
    paths: number
    block_len: number
    ruin_threshold: Decimal
    max_drawdown: Summary
    longest_losing_streak: Summary
    longest_under_water: Summary
    final_return: Summary
    risk_of_ruin: Decimal
    observed_max_drawdown: Decimal
  }
  walk_forward: { from: Utc; to: Utc; trades: number; return: Decimal }[]
  halves: null | { split_month: string; first: Decimal; second: Decimal }
  cost_stress: {
    stress: Decimal
    mean_r_net: Decimal | null
    monthly_mean: Decimal | null
    sharpe: Decimal | null
  }
  luck: null | {
    simulations: number
    strategy_mean_r: Decimal
    percentile_mean_r: Decimal
    percentile_total: Decimal
  }
  buy_and_hold: null | {
    monthly_mean: Decimal | null
    sharpe: Decimal | null
    max_drawdown: Decimal
    total_return: Decimal
  }
  excursions: { mae_r: Quantiles | null; mfe_r: Quantiles | null; mfe_r_losers: Quantiles | null }
  edge: {
    cost_per_trade_r: Decimal | null
    edge_cost_factor: Decimal
    required_gross_r: Decimal | null
    measured_gross_r: Decimal | null
  }
  slices: { by_year: Slice[]; by_hour_utc: Slice[]; by_side: Slice[] }
  notes: string[]
}
