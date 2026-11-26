/**
 * Tipos de los contratos del replay de la Academia (#720) para la UI: `replay_session.v1`,
 * `replay_step.v1`, `replay_mark.v1`, `replay_order.v1` y `exam_result.v1` de `contracts/`. Son la misma forma del contrato. Las velas no traen la fecha real (día de la sesión y hora UTC): se
 * revela recién en `exam_result.v1`. Precios como texto decimal.
 */

type Decimal = string
type Utc = string

export type ReplayKind = 'practice' | 'levels' | 'trades'

/** Una vela cerrada sin su fecha real. */
export type ReplayBar = {
  index: number
  /** día de la sesión (1 = el de la primera vela), en días UTC */
  day: number
  /** hora UTC del inicio de la vela, HH:MM */
  time: string
  open: Decimal
  high: Decimal
  low: Decimal
  close: Decimal
}

export type ReplayRules = {
  trades_per_exam: number
  level_tolerance_atr: Decimal
  stop_min_atr: Decimal
  stop_max_atr: Decimal
  max_risk_pct: Decimal
  provisional: boolean
}

/** `replay_session.v1` (`POST /api/v1/academy/replay`). */
export type ReplaySession = {
  session_id: string
  kind: ReplayKind
  lesson_id: string | null
  seed: string
  instrument_id: string
  timeframe: '15m' | '1h'
  source: 'futures' | 'ig'
  warmup_bars: number
  exam_bars: number
  cursor: number
  bars: ReplayBar[]
  rules: ReplayRules
}

/** `replay_session.v1#/$defs/request`. */
export type SessionRequest = { kind: ReplayKind; lesson_id?: string; seed?: string }

/** `replay_step.v1` (`POST /api/v1/academy/replay/{id}/step`): solo las velas nuevas. */
export type ReplayStep = { session_id: string; cursor: number; bars: ReplayBar[]; done: boolean }

/** level: soporte o resistencia; pivot_high / pivot_low: máximo o mínimo local en una vela. */
export type MarkKind = 'level' | 'pivot_high' | 'pivot_low'

/** `replay_mark.v1#/$defs/request` (`POST /replay/{id}/marks`); el cursor lo pone el motor. */
export type MarkRequest = { kind: MarkKind; price: Decimal; bar_index?: number }

/** `replay_mark.v1`: la marca como la guardó el motor. */
export type ReplayMark = {
  mark_id: number
  kind: MarkKind
  price: Decimal
  bar_index: number | null
  cursor: number
}

/** `replay_order.v1#/$defs/request` (`POST /replay/{id}/orders`): stop y objetivo pueden faltar. */
export type OrderRequest = {
  side: 'long' | 'short'
  stop?: Decimal
  target?: Decimal
  risk_pct: Decimal
  plan: string
}

/** `replay_order.v1`: decidida al cierre de la vela `cursor`, entra en la apertura siguiente. */
export type ReplayOrder = {
  order_id: number
  side: 'long' | 'short'
  stop: Decimal | null
  target: Decimal | null
  risk_pct: Decimal
  plan: string
  cursor: number
}

/** Una regla de proceso: cumplida o no (`exam_result.v1#/$defs/rule`). */
export type ExamRule = {
  rule:
    | 'stop_present'
    | 'stop_side'
    | 'stop_in_range'
    | 'risk_within_limit'
    | 'plan_written'
    | 'one_position'
    | 'enough_trades'
    | 'level_precision'
  passed: boolean
}

/** Lo que pasó con una entrada; los R son null sin stop (sin stop no hay R). */
export type Outcome = {
  entry_index: number
  entry_price: Decimal
  exit_index: number
  exit_price: Decimal
  exit: 'stop' | 'stop_gap' | 'target' | 'end'
  r_gross: Decimal | null
  costs_r: Decimal | null
  r_net: Decimal | null
}

/** `exam_result.v1` (`POST /replay/{id}/finish`): proceso, niveles, R y recién ahí la fecha. */
export type ExamResult = {
  session_id: string
  kind: ReplayKind
  lesson_id: string | null
  catalog_version: string
  /** null en una práctica (sin nota) */
  passed: boolean | null
  rules: ExamRule[]
  trades: { order_id: number; rules: ExamRule[]; outcome: Outcome | null }[]
  levels: {
    marked: number
    engine: number
    matched_marks: number
    covered_levels: number
    precision_pct: Decimal | null
    coverage_pct: Decimal | null
    tolerance: Decimal
    engine_levels: Decimal[]
  } | null
  revealed: {
    instrument_id: string
    timeframe: '15m' | '1h'
    source: 'futures' | 'ig'
    first_bar_at: Utc
    last_bar_at: Utc
  }
  graded_at: Utc
}
