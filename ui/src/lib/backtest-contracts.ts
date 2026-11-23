/**
 * Tipos de `contracts/backtest_run.v1.schema.json` (spec §15b, #200) para la UI. Son la misma forma
 * del contrato: si cambia el contrato, se cambia acá. Montos y fracciones llegan como texto decimal.
 */

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
  }
  started_at: Utc
  finished_at: Utc
}
