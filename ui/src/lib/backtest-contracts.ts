/**
 * Tipos de `contracts/backtest_run.v1.schema.json` (spec §15b, #200) para la UI. Son la misma forma
 * del contrato: si cambia el contrato, se cambia acá. Montos y fracciones llegan como texto decimal.
 */

import type { Proposal } from '@/mocks'

/** Texto decimal (sin float), como en el contrato. */
type Decimal = string
/** Instante UTC con zona, como "2026-11-20T21:00:00Z". */
type Utc = string

export type BacktestCommission =
  | { kind: 'none' }
  | { kind: 'per_share'; per_share: Decimal; min: Decimal }
  | { kind: 'percent'; pct: Decimal; min: Decimal }

export type BacktestRun = {
  run_id: string
  instruments: string[]
  timeframe: '15m' | '1h'
  from: Utc
  to: Utc
  config_sha256: string
  initial_equity: Decimal
  costs: {
    slippage_spread_fraction: Decimal
    slippage_ticks: Decimal
    spread_source?: 'measured' | 'config'
    instruments: {
      instrument_id: string
      spread: Decimal
      tick: Decimal
      commission: BacktestCommission
      funding_admin_fee_pct: Decimal
      funding_day_count: Decimal
    }[]
  }
  news: null | {
    interpreter_name: string
    interpreter_version: string
    interpreter_model: string | null
    published_from: Utc
    interpretations: number
  }
  metrics: {
    trades: number
    final_equity: Decimal
    /** fracción: 0.1 = +10 % */
    total_return: Decimal
    /** fracción positiva */
    max_drawdown: Decimal
    sharpe: Decimal | null
    sortino: Decimal | null
    win_rate: Decimal | null
    profit_factor: Decimal | null
    exposure: Decimal | null
    kill_switch_at?: Utc
    /** solo backtest (#277): días hábiles hasta reactivar el recorrido */
    kill_switch_reset_business_days?: number
    /** cada activación y reactivación del kill switch, en orden (#277) */
    kill_switch_events?: KillSwitchEvent[]
  }
  started_at: Utc
  finished_at: Utc
}

export type KillSwitchEvent = { event: 'activated' | 'reset'; at: Utc; equity: Decimal }

/** `contracts/backtest_run_curve.v1` (#299): la curva diaria y comprar y mantener. */
export type RunCurve = {
  run_id: string
  /** un punto por día; el drawdown lo calcula el motor */
  points: { date: string; equity: Decimal; drawdown: Decimal }[]
  /** vacío si la corrida no lo calculó */
  buy_and_hold: {
    instrument_id: string
    size: Decimal
    entry_price: Decimal
    exit_price: Decimal
    nights: number
    commission: Decimal
    funding: Decimal
    net_pnl: Decimal
    total_return: Decimal
  }[]
}

export type ExitReason =
  'stop' | 'tp1' | 'break_even' | 'trailing' | 'time' | 'forced' | 'end_of_data'

/** `contracts/outcome.v1`: el resultado de una propuesta (montos en la moneda del instrumento). */
export type Outcome = {
  proposal_id: string
  kind: 'simulated' | 'real'
  entry_at: Utc
  entry_price: Decimal
  exits: { at: Utc; price: Decimal; fraction: Decimal; reason: ExitReason }[]
  commission: Decimal
  funding: Decimal
  gross_pnl: Decimal
  net_pnl: Decimal
  hit_tp1: boolean
  hit_stop: boolean
  exit_reason: ExitReason
}

/** `contracts/backtest_run_detail.v1` (#299): `GET /api/v1/backtest_runs/{run_id}`. */
export type RunDetail = {
  run: BacktestRun
  /** null: la corrida es anterior a #299 y no guardó la curva */
  curve: RunCurve | null
  trades: { proposal: Proposal; outcome: Outcome | null }[]
}
