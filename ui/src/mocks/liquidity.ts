/**
 * DATOS DE EJEMPLO de la vista de liquidez en vivo (#231, spec §5.4.1). No son precios reales.
 *
 * Los tipos siguen **exactamente** los contratos de `contracts/` (mismos campos, decimales como
 * texto y los mismos enums): `liquidity_level.v1`, `fvg_event.v1`, `liquidity_stats.v1` y
 * `imbalance.v1`. Todo llega **ya calculado**, como lo manda el motor (`cfds-liquidity`); la UI
 * no calcula nada (CLAUDE.md §1). El JSON se generó una vez fuera de la app, y un test de
 * `engine/crates/core/tests/contracts.rs` lo valida contra los schemas.
 *
 * TODO(#231): cuando el `gateway` publique estos mensajes en vivo, cambiar la fuente de la vista
 * por la suscripción. `imbalance.v1` sigue el schema del PR #236 (todavía sin mergear).
 */
import sample from './liquidity-us100-15m.json'

type Utc = string
/** Decimal exacto como texto, nunca como número JSON. */
type Decimal = string

/** `candle.v1` (IG no informa volumen en sus CFD: llega null). */
export type LiquidityBar = {
  instrument_id: string
  timeframe: '15m' | '1h'
  start: Utc
  open: Decimal
  high: Decimal
  low: Decimal
  close: Decimal
  volume: Decimal | null
}

export type LevelKind =
  'eqh' | 'eql' | 'pdh' | 'pdl' | 'pwh' | 'pwl' | 'session_high' | 'session_low'
export type TradingSession = 'asia' | 'london' | 'ny'
/** intact; swept = barrido con reversión (el sweep de §5.4); broken = roto. */
export type LevelState = 'intact' | 'swept' | 'broken'

/** `liquidity_level.v1` */
export type LiquidityLevel = {
  instrument_id: string
  kind: LevelKind
  /** solo para session_high y session_low; null en los demás */
  session: TradingSession | null
  price: Decimal
  exists_from: Utc
  state: LevelState
  /** cierre de la vela que barrió o rompió el nivel; null si está intacto */
  state_changed_at: Utc | null
  /**
   * Opcional (PR #237, todavía sin mergear): cierre de la vela en que el nivel intacto venció
   * (`max_age_bars`). Ausente o null si no venció.
   */
  expired_at?: Utc | null
}

export type Direction = 'bullish' | 'bearish'
export type FvgPoint = { at: Utc; bars: number } | null

/** `fvg_event.v1` */
export type FvgEvent = {
  instrument_id: string
  timeframe: '15m' | '1h' | '1d'
  direction: Direction
  created_at: Utc
  bottom: Decimal
  top: Decimal
  atr: Decimal
  size_atr: Decimal
  touch: FvgPoint
  mid: FvgPoint
  fill: FvgPoint
  expiry: FvgPoint
  reactions: { bars: number; return_atr: Decimal | null }[]
  status: 'open' | 'filled' | 'expired'
  as_of: Utc
}

/** Una tasa de `liquidity_stats.v1`: fracción de 0 a 1 con su IC 95 % de Wilson. */
export type StatsRate = {
  metric: 'touch' | 'mid' | 'fill' | 'same_session_fill'
  successes: number
  n: number
  /** true si n < min_cases: "datos insuficientes", y los valores llegan en null */
  insufficient: boolean
  rate: Decimal | null
  ci_low: Decimal | null
  ci_high: Decimal | null
}

export type StatsDistribution = {
  metric: 'bars_to_touch' | 'bars_to_mid' | 'bars_to_fill' | 'sessions_to_fill' | 'reaction'
  horizon_bars: number | null
  n: number
  insufficient: boolean
  median: Decimal | null
  p25: Decimal | null
  p75: Decimal | null
  iqr: Decimal | null
}

/** `liquidity_stats.v1`: agregados de un grupo de FVG o de gaps de apertura. */
export type LiquidityStats = {
  instrument_id: string
  subject: 'fvg' | 'opening_gap'
  timeframe: '15m' | '1h' | '1d'
  direction: Direction
  size_tercile: 'low' | 'mid' | 'high' | 'all'
  size_cuts_atr: { low_max: Decimal; mid_max: Decimal }
  min_cases: number
  n: number
  rates: StatsRate[]
  distributions: StatsDistribution[]
  as_of: Utc
}

/** `imbalance.v1` (PR #236). No es flujo de órdenes: viaja siempre con su rótulo. */
export type Imbalance = {
  instrument_id: string
  timeframe: '15m' | '1h' | '1d'
  at: Utc
  window_bars: number
  displacement: Direction | null
  bullish_body_sum: Decimal
  bearish_body_sum: Decimal
  range_sum: Decimal
  index: string
  label: 'aproximación desde el precio'
}

/** Lo que recibe la vista: cada lista son mensajes de su contrato, tal cual. */
export type LiquiditySnapshot = {
  note: string
  instrument_id: string
  as_of: Utc
  candles: LiquidityBar[]
  levels: LiquidityLevel[]
  fvgs: FvgEvent[]
  /** un valor por vela cerrada; el último es el actual */
  imbalance: Imbalance[]
  stats: LiquidityStats[]
}

export const LIQUIDITY_US100 = sample as LiquiditySnapshot
