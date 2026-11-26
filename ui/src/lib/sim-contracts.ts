/**
 * Tipos de los contratos de los simuladores para la UI: `variance_sim_request.v1` y
 * `variance_sim_report.v1` (#801). Son la misma forma del contrato: si cambia el contrato, se
 * cambia acá. Números como texto decimal (sin float); `null` cuando el motor no lo define.
 */
import type { Decimal } from '@/lib/lab-contracts'

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
