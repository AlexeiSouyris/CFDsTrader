/**
 * La evaluación de la Fase 5 desde el gateway (#298, #316), solo `GET`:
 *
 * | Ruta | Respuesta |
 * |---|---|
 * | `GET /api/v1/walk_forward?instrument=<id>` | el último `walk_forward_report.v1` del instrumento, o `null` |
 *
 * Todo lo calcula el motor (walk-forward, sensibilidad, comprar y mantener y fuera de muestra).
 * La UI no recalcula nada: muestra lo que llega. Sin gateway no hay datos de ejemplo.
 */
import { useRemote } from '@/lib/liquidity-api'

/** `metrics` de `walk_forward_report.v1`. Fracciones como texto; `null` = no definida. */
export type WfMetrics = {
  trades: number
  total_return: string
  max_drawdown: string
  sharpe: string | null
  sortino: string | null
  win_rate: string | null
  profit_factor: string | null
  exposure: string | null
}

export type WfBuyAndHold = {
  /** Sobre el capital inicial, moneda de la cuenta. */
  return: string
  net_pnl: string
  commission: string
  funding: string
  nights: number
}

export type WfPeriod = {
  from: string
  to: string
  candles: number
  /** `null`: el tramo no tiene velas. */
  metrics: WfMetrics | null
  final_equity: string
  buy_and_hold: WfBuyAndHold | null
}

export type WfStability = {
  file: 'strategy.toml' | 'backtest.toml' | 'risk.toml'
  key: string
  by_value: {
    value: string
    mean_return: string | null
    mean_sharpe: string | null
    trades: number
  }[]
  return_spread: string | null
  same_sign: boolean
}

/** `contracts/walk_forward_report.v1`. */
export type WalkForwardReport = {
  report_id: string
  instrument_id: string
  candle_source: 'ig' | 'futures'
  config_sha256: string
  split: {
    train_months: number
    validation_months: number
    step_months: number
    out_of_sample_months: number
  }
  warmup_days: number
  initial_equity: string
  buy_and_hold_size: string
  folds: { index: number; train: WfPeriod; test: WfPeriod }[]
  test_aggregate: {
    periods: number
    with_data: number
    trades: number
    mean_return: string | null
    median_return: string | null
    positive: number
    worst_drawdown: string | null
    mean_sharpe: string | null
    sharpe_defined: number
    beat_buy_and_hold: number
    with_buy_and_hold: number
  }
  sensitivity: {
    file: string
    key: string
    value: string
    fold: number
    metrics: WfMetrics | null
  }[]
  stability: WfStability[]
  holdout: WfPeriod
  computed_at: string
}

export async function fetchWalkForward(
  instrument: string,
  signal?: AbortSignal,
): Promise<WalkForwardReport | null> {
  const res = await fetch(`/api/v1/walk_forward?instrument=${encodeURIComponent(instrument)}`, {
    signal,
    headers: { Accept: 'application/json' },
  })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as WalkForwardReport | null
}

/** El último reporte del instrumento; `null` en `key` no pide nada (datos de ejemplo). */
export const useWalkForward = (instrument: string | null) =>
  useRemote(instrument && `walk-forward|${instrument}`, (signal) =>
    fetchWalkForward(instrument!, signal),
  )
