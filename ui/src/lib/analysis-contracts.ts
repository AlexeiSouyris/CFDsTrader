/**
 * Bloque `analysis` de `chart_export.v1` (#491), que la vista de liquidez trae en 15m con las
 * velas de IG (`liquidity_view.v1`, #501). Lo arma el motor (`cfds_backtest::analysis`): la UI
 * solo lo dibuja. Los decimales llegan como texto y se muestran tal cual.
 */

type Decimal = string

export type FibLevel = { ratio: Decimal; price: Decimal }

export type FibLeg = {
  direction: 'up' | 'down'
  start_index: number
  start_price: Decimal
  end_index: number
  end_price: Decimal
  /** el tramo existe desde esta vela (la confirmación de su pivote final) */
  confirmed_index: number
  retracements: FibLevel[]
  extensions: FibLevel[]
  golden_pocket_low: Decimal
  golden_pocket_high: Decimal
}

export type PocketEvent = {
  index: number
  /** índice en `fibonacci` */
  leg: number
  touches: boolean
  closes_inside: boolean
}

export type WyckoffEvent = {
  kind: 'spring' | 'upthrust'
  break_index: number
  /** la señal: la vela que vuelve a cerrar dentro del rango */
  confirmed_index: number
  level: Decimal
  touches: number
  extreme: Decimal
}

export type BottomEvent = {
  index: number
  price: Decimal
  prior_high: Decimal
  rebound_index: number
  /** la señal */
  confirmed_index: number
}

export type MaCross = {
  kind: 'sma' | 'ema'
  fast_period: number
  slow_period: number
  fast: (Decimal | null)[]
  slow: (Decimal | null)[]
  crosses: { kind: 'golden' | 'death'; day: string; known_from_index: number }[]
}

export type FloorPivots = {
  day: string
  /** vale desde esta vela hasta la entrada siguiente */
  from_index: number
  pp: Decimal
  r1: Decimal
  r2: Decimal
  r3: Decimal
  s1: Decimal
  s2: Decimal
  s3: Decimal
}

export type VixRegime = 'calm' | 'normal' | 'elevated' | 'stress'

export type ChartAnalysis = {
  ema_daily_period: number
  /** un valor por vela de `candles`; null en el calentamiento */
  ema_daily: (Decimal | null)[]
  fibonacci: FibLeg[]
  golden_pocket: PocketEvent[]
  wyckoff: WyckoffEvent[]
  bottoms: BottomEvent[]
  ma_cross: MaCross
  floor_pivots: FloorPivots[]
  /** un valor por vela de `candles`; null sin dato */
  vix: ({ close: Decimal; regime: VixRegime } | null)[]
}
