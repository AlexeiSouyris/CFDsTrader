/**
 * Tipos del contexto de mercado que muestra el Panel (#294). Siguen **exactamente** los
 * contratos de `contracts/`: `sentiment.v1` y `analog_report.v1`. Decimales como texto y horas
 * UTC. Los calcula Python; la UI solo los muestra.
 */

type Utc = string
type Decimal = string

export type SentimentComponent = {
  name: string
  status: 'valid' | 'warming_up' | 'expired' | 'missing' | 'pending_data'
  weight: Decimal
  observed_for: Utc | null
  published_at: Utc | null
  raw: Decimal | null
  percentile: Decimal | null
  component: Decimal | null
}

/** `sentiment.v1`: de −1 (miedo) a +1 (euforia). */
export type SentimentIndex = {
  instrument_id: string
  as_of: Utc
  published_at: Utc
  value: Decimal
  warmup_complete: boolean
  min_components: number
  components: SentimentComponent[]
}

export type AnalogHorizon = {
  horizon: number
  /** retornos como fracción (0.012 = +1,2 %), de menor a mayor p */
  percentiles: { p: number; return: Decimal }[]
  /** % de vecinos con retorno positivo, de 0 a 100 */
  positive_pct: Decimal
}

export type AnalogGroup = {
  group: 'all' | 'same_context'
  count: number
  /** menos vecinos que `min_neighbors`: "datos insuficientes" */
  insufficient: boolean
  horizons: AnalogHorizon[]
  hits: { up: number; down: number; none: number }
  without_atr: number
}

/** `analog_report.v1`: distribución de resultados pasados, nunca un pronóstico. */
export type AnalogReport = {
  instrument_id: string
  source: 'ig' | 'futures'
  window_end_day: string
  as_of: Utc
  computed_at: Utc
  window: number
  min_neighbors: number
  context: { vol: 'low' | 'mid' | 'high' | null; rates: 'sube' | 'baja' | 'estable' | null }
  neighbors: { instrument_id: string; end_day: string; distance: Decimal }[]
  groups: AnalogGroup[]
}

/** Estados de `emotional_state.v1` (spec §9.1, #220). */
export type EmotionKind =
  'fomo' | 'selling_climax' | 'rejection' | 'extreme_fear' | 'extreme_euphoria' | 'complacency'

/**
 * `emotional_state.v1`: los estados activos de un instrumento con los valores de su regla, tal
 * como los calcularon el motor (precio) y Python (índice). Un dato, nunca un pronóstico.
 */
export type EmotionalState = {
  instrument_id: string
  source: 'price' | 'sentiment'
  basis: '15m' | '1h' | 'sentiment_index'
  as_of: Utc
  states: { state: EmotionKind; values: Record<string, string> }[]
}
