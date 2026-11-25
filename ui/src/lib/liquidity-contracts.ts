/**
 * Tipos de los mensajes de liquidez que manda el motor (#231). Siguen **exactamente** los
 * contratos de `contracts/`: `candle.v1`, `liquidity_level.v1`, `fvg_event.v1`,
 * `liquidity_stats.v1`, `imbalance.v1` y el sobre `liquidity_view.v1` (respuesta del gateway).
 * Mismos campos, decimales como texto y los mismos enums. La UI no calcula nada (CLAUDE.md §1).
 */

import type { ChartAnalysis } from '@/lib/analysis-contracts'

export type Utc = string
/** Decimal exacto como texto, nunca como número JSON. */
export type Decimal = string

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
  | 'eqh'
  | 'eql'
  | 'pdh'
  | 'pdl'
  | 'pwh'
  | 'pwl'
  | 'session_high'
  | 'session_low'
  | 'round_high'
  | 'round_low'
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
   * Opcional (#237): cierre de la vela en que el nivel, todavía intacto, venció (`max_age_bars`).
   * Ausente o null si no venció.
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
  /** `at` (opcional): desde cuándo se conoce la reacción */
  reactions: { bars: number; return_atr: Decimal | null; at?: Utc | null }[]
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

/** `imbalance.v1`. No es flujo de órdenes: viaja siempre con su rótulo. */
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

/** Los timeframes de la vista. */
export type ViewTimeframe = '15m' | '1h' | '1d'

/**
 * De qué velas sale la vista (#259): `ig` = las del CFD de IG; `futures` = las armadas desde los
 * futuros de Databento, con la forma del precio del futuro y una base distinta a la del CFD.
 * `label` es el texto que manda el motor (`config/candle_sources.toml`).
 */
export type ViewSource = { id: 'ig' | 'futures'; label: string }

/**
 * La fuente que se pide (#628): una de las de arriba o `auto` = IG y, si IG no tiene velas
 * suficientes, la primera de reemplazo con velas. Una vista siempre dice la que de verdad usó.
 */
export type SourceChoice = ViewSource['id'] | 'auto'

/** Por qué la vista no sale de IG con `source=auto` (#628); `note` se muestra tal cual. */
export type SourceFallback = { from: 'ig'; note: string }

/** `daily_candle.v1`: día local de medianoche a medianoche, con `start` y `end` explícitos. */
export type DailyBar = {
  instrument_id: string
  day: string
  start: Utc
  end: Utc
  open: Decimal
  high: Decimal
  low: Decimal
  close: Decimal
  volume: Decimal | null
}

/** `liquidity_view.v1`: `GET /api/v1/liquidity?instrument=<id>&timeframe=<tf>` del gateway. */
export type LiquidityViewData = {
  instrument_id: string
  /** opcional en el contrato; ausente = 15m */
  timeframe?: ViewTimeframe
  /** opcional en el contrato (#259); ausente = velas del CFD de IG */
  source?: ViewSource
  /** opcional en el contrato (#628): solo si `auto` pasó a una fuente de reemplazo */
  fallback?: SourceFallback
  generated_at: Utc
  /** cierre de la última vela cerrada guardada; null si no hay velas ("sin datos todavía") */
  as_of: Utc | null
  /** `candle.v1` en 15m y 1h; `daily_candle.v1` en 1d */
  candles: LiquidityBar[] | DailyBar[]
  levels: LiquidityLevel[]
  fvgs: FvgEvent[]
  /** un valor por vela de la ventana; el último es el actual */
  imbalance: Imbalance[]
  stats: LiquidityStats[]
  /**
   * opcional en el contrato (#501): las herramientas de análisis del motor, solo en 15m con las
   * velas de IG; los índices se refieren a `candles`
   */
  analysis?: ChartAnalysis
}

/** `instrument.v1` */
export type InstrumentInfo = {
  id: string
  name: string
  asset_class: string
  currency: string
  exchange_timezone: string
}

/** `gateway_instruments.v1`: `GET /api/v1/instruments` del gateway. */
export type GatewayInstruments = {
  default_instrument: string
  default_timeframe: ViewTimeframe
  timeframes: ViewTimeframe[]
  instruments: InstrumentInfo[]
  /** opcional en el contrato (#259); la primera es la que abre la vista (#628: `auto`) */
  sources?: { id: SourceChoice; label: string }[]
}
