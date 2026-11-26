/**
 * Tipos de los contratos de los simuladores para la UI: `variance_sim_request.v1` y
 * `variance_sim_report.v1` (#801), y `prop_rules.v1`, `prop_sim_request.v1` y
 * `prop_sim_result.v1` (#802, #803). Son la misma forma del contrato: si cambia el contrato, se
 * cambia acá. Números como texto decimal (sin float); `null` cuando el motor no lo define.
 */
import type { Decimal, Utc } from '@/lib/lab-contracts'

/** Media y cuantiles 5, 50 y 95 % sobre los caminos. */
export type SimSummary = { mean: Decimal; p5: Decimal; p50: Decimal; p95: Decimal }

/** Una probabilidad con su intervalo de Wilson. */
export type Probability = { probability: Decimal; low: Decimal; high: Decimal }

// ── Simulador de varianza ─────────────────────────────────────────────────────────────────

export type VarianceSource =
  | {
      kind: 'model'
      win_rate: Decimal
      avg_win_r: Decimal
      avg_loss_r: Decimal
      observed_trades?: number | null
    }
  | { kind: 'r_values'; r: Decimal[]; block_len?: number }
  | { kind: 'report'; report_id: string; block_len?: number }

/** `variance_sim_request.v1`: lo opcional que falta lo pone el motor (`[variance]` de config). */
export type VarianceRequest = {
  source: VarianceSource
  risk_pct: Decimal
  trades: number
  paths?: number
  seed?: number
  ruin_threshold_pct?: Decimal
}

export type Band = {
  trade: number
  p5: Decimal
  p25: Decimal
  p50: Decimal
  p75: Decimal
  p95: Decimal
}

/** `variance_sim_report.v1`. */
export type VarianceReport = {
  source:
    | {
        kind: 'model'
        win_rate: Decimal
        avg_win_r: Decimal
        avg_loss_r: Decimal
        observed_trades: number | null
      }
    | { kind: 'r_values' | 'report'; count: number; block_len: number; report_id: string | null }
  risk_pct: Decimal
  trades: number
  paths: number
  seed: number
  ruin_threshold_pct: Decimal
  expectancy_r: Decimal
  bands: Band[]
  final_equity: {
    mean: Decimal
    p5: Decimal
    p25: Decimal
    p50: Decimal
    p75: Decimal
    p95: Decimal
  }
  max_drawdown: {
    summary: SimSummary
    histogram: { from: Decimal; to: Decimal; share: Decimal }[]
  }
  longest_losing_streak: SimSummary
  losing_streak_at_least: { k: number; probability: Decimal }[]
  risk_of_ruin: Probability
  win_rate_draws: SimSummary | null
  fixed_win_rate: {
    risk_of_ruin: Decimal
    final_equity_p5: Decimal
    final_equity_p50: Decimal
    max_drawdown_p50: Decimal
  } | null
  notes: string[]
}

// ── Simulador de cuentas fondeadas ───────────────────────────────────────────────────────

export type DrawdownMode = 'static' | 'trailing_eod' | 'trailing_intraday'

/** `prop_rules.v1`: en una plantilla los valores pueden ir en `null` (en blanco). */
export type PropRules = {
  id: string
  name: string
  source: { firm: string; url: string; read_at: string } | null
  account_size: Decimal | null
  currency: string | null
  day: { timezone: string; cutoff: string } | null
  phases: {
    name: string
    profit_target_pct: Decimal | null
    min_trading_days: number | null
    max_trading_days: number | null
    consistency_pct: Decimal | null
  }[]
  daily_loss: { limit_pct: Decimal | null; base: 'account_size' | 'day_start' } | null
  max_drawdown: { limit_pct: Decimal | null; mode: DrawdownMode; lock_at_initial: boolean } | null
  funded: {
    payout_min_trading_days: number | null
    payout_min_profit_pct: Decimal | null
    profit_split_pct: Decimal | null
    consistency_pct: Decimal | null
  } | null
  fees: {
    evaluation: Decimal | null
    activation: Decimal | null
    refund_with_first_payout: boolean
  }
}

export type PropSimSource =
  | { kind: 'trades'; trades: { closed_at: Utc; r: Decimal }[] }
  | { kind: 'report'; report_id: string }
  | {
      kind: 'model'
      win_rate: Decimal
      avg_win_r: Decimal
      avg_loss_r: Decimal
      observed_trades?: number | null
      trades_per_day: number
    }

/** `prop_sim_request.v1`. */
export type PropSimRequest = {
  rules: PropRules
  risk_per_trade_pct: Decimal
  source: PropSimSource
  monte_carlo?: {
    paths?: number
    seed?: number
    block_len_days?: number
    horizon_days?: number
    max_attempts?: number
  }
}

export type StageStatus = 'passed' | 'payout' | 'failed' | 'timeout' | 'unfinished' | 'not_reached'

export type Stage = {
  name: string
  status: StageStatus
  rule: 'daily_loss' | 'max_drawdown' | null
  trading_days: number
  end_day: string | null
  profit: Decimal
  best_day: Decimal | null
  min_balance: Decimal
  payout: Decimal | null
}

/** `prop_sim_result.v1`. */
export type PropSimResult = {
  rules: PropRules
  risk_per_trade_pct: Decimal
  risk_per_trade_amount: Decimal
  source: {
    kind: 'trades' | 'report' | 'model'
    trades: number | null
    days: number | null
    report_id: string | null
    trades_per_day: number | null
  }
  unsimulable: string[]
  upper_bound: boolean
  observed: {
    status: 'payout' | 'passed' | 'failed' | 'timeout' | 'unfinished'
    phases: Stage[]
    funded: Stage | null
  } | null
  monte_carlo: {
    paths: number
    seed: number
    block_len_days: number | null
    horizon_days: number
    confidence: Decimal
    per_attempt: {
      pass_evaluation: Probability
      payout: Probability | null
      failed: Probability
      timeout: Probability
      unfinished: Probability
    }
    phases: {
      name: string
      reached: Decimal
      passed: Probability
      failed_daily_loss: Decimal
      failed_max_drawdown: Decimal
      timeout: Decimal
      days_to_pass: SimSummary | null
      days_to_fail: SimSummary | null
    }[]
    economics: {
      max_attempts: number
      success_within_attempts: Decimal
      expected_attempts: Decimal
      expected_fees: Decimal
      expected_payout: Decimal | null
      expected_refund: Decimal
      expected_value: Decimal | null
    }
    zero_edge: {
      pass_evaluation: Probability
      payout: Probability | null
      expected_value: Decimal | null
    }
    beats_zero_edge: boolean
  } | null
  notes: string[]
}

/** Un error de reglas del motor (`path` = JSON Pointer dentro de `prop_rules.v1`). */
export type RuleError = { path: string; message: string }
