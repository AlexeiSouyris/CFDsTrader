/**
 * DATOS DE EJEMPLO de la vista de liquidez en vivo (#231, spec §5.4.1). No son precios reales.
 *
 * Todo llega **ya calculado**, como lo mandaría el motor (`engine/crates/liquidity`): los niveles
 * con su estado, los FVG con sus eventos, los agregados con su IC 95 % de Wilson y el índice de
 * desequilibrio. La UI no calcula nada (CLAUDE.md §1). El JSON se generó una vez fuera de la app.
 *
 * TODO(#231): estos tipos se derivaron de la spec porque los contratos todavía no están en
 * `main`. Cuando se mergeen `contracts/liquidity_level.v1`, `contracts/fvg_event.v1` y
 * `contracts/liquidity_stats.v1`, reemplazar cada tipo por el del contrato (nombres de campos,
 * enums y unidades) y borrar los que sobren. Los datos en vivo llegarán por el WebSocket del
 * `gateway`; hasta entonces la vista usa este archivo.
 */
import sample from './liquidity-us100-15m.json'

/** Vela de 15m (forma de `candle.v1`, sin volumen: IG no lo informa en sus CFD). */
export type LiquidityBar = { start: string; open: string; high: string; low: string; close: string }

export type LevelKind =
  'eqh' | 'eql' | 'pdh' | 'pdl' | 'pwh' | 'pwl' | 'session_high' | 'session_low'
export type TradingSession = 'asia' | 'london' | 'new_york'
/** §5.4.1: intacto, barrido con reversión (sweep) o roto. Se guarda con la vela que lo cambió. */
export type LevelState = 'intact' | 'swept_reversal' | 'broken'

/** Nivel de liquidez (futuro `liquidity_level.v1`). */
export type LiquidityLevel = {
  id: string
  kind: LevelKind
  /** solo para `session_high` y `session_low` */
  session: TradingSession | null
  price: string
  /** desde cuándo existe (UTC): antes de esa hora el nivel no se muestra */
  exists_from: string
  state: LevelState
  /** cierre de la vela que cambió el estado; null si sigue intacto */
  state_changed_at: string | null
  /** pivotes que forman EQH o EQL; null en los demás */
  touches: number | null
}

export type Direction = 'bullish' | 'bearish'
export type FvgMilestone = { at: string; bars: number } | null

/** FVG con sus eventos (futuro `fvg_event.v1`). */
export type FvgEvent = {
  id: string
  timeframe: '15m' | '1h' | '1d'
  direction: Direction
  created_at: string
  zone_low: string
  zone_high: string
  /** tamaño de la zona en múltiplos del ATR14 de su timeframe */
  size_atr: string
  first_touch: FvgMilestone
  midpoint: FvgMilestone
  filled: FvgMilestone
  /** retorno del cierre a 4 y 8 velas del primer toque, en ATR, a favor del FVG */
  reaction_atr: { bars_4: string | null; bars_8: string | null }
  status: 'open' | 'filled' | 'expired'
}

/** Una tasa con su IC 95 % de Wilson, o "datos insuficientes" si hay menos de `min_cases`. */
export type RateStat =
  | { status: 'ok'; rate_pct: string; ci95_low_pct: string; ci95_high_pct: string; n: number }
  | { status: 'insufficient'; n: number; min_cases: number }

export type FvgStatsRow = {
  timeframe: '15m' | '1h' | '1d'
  direction: Direction
  size_tercile: 'small' | 'medium' | 'large'
  cases: number
  touch: RateStat
  midpoint: RateStat
  fill: RateStat
  median_bars: { touch: number | null; midpoint: number | null; fill: number | null }
  reaction_atr_8: { median: string; q1: string; q3: string } | null
}

export type GapStatsRow = {
  direction: 'up' | 'down'
  cases: number
  fill_same_session: RateStat
  fill_within_max: RateStat
  median_sessions: string | null
}

/** Agregados (futuro `liquidity_stats.v1`). */
export type LiquidityStats = {
  fvg: FvgStatsRow[]
  gaps: GapStatsRow[]
  min_cases: number
  sample_from: string
  sample_to: string
}

/** Índice de desequilibrio aproximado desde el precio (−1 a +1). No es flujo de órdenes. */
export type Imbalance = { window_bars: number; index: string; as_of: string }

export type LiquiditySnapshot = {
  note: string
  instrument_id: string
  timeframe: '15m'
  as_of: string
  candles: LiquidityBar[]
  levels: LiquidityLevel[]
  fvgs: FvgEvent[]
  /** velas de desplazamiento, marcadas por el motor */
  displacement: { at: string; direction: Direction }[]
  imbalance: Imbalance
  stats: LiquidityStats
}

export const LIQUIDITY_US100 = sample as LiquiditySnapshot
