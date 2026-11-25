/**
 * Los lugares de una grilla de gráficos (#286, #540): qué instrumento y timeframe muestra cada uno
 * y de dónde salen sus velas (gateway o, sin gateway, los datos de ejemplo). Lo comparten la
 * Terminal y el Multigráfico; el dibujo está en `components/ui/chart-slot.tsx`. No calcula nada.
 */
import { useInstruments, useLiquidityView } from '@/lib/liquidity-api'
import type { LiquidityViewData, ViewTimeframe } from '@/lib/liquidity-contracts'
import { LIQUIDITY_US100, TERMINAL_INSTRUMENTS } from '@/mocks'

export type ChartSel = { instrument: string; timeframe: ViewTimeframe }
export const TIMEFRAMES: ViewTimeframe[] = ['15m', '1h', '1d']
/** Lo que abre cada lugar de la grilla la primera vez. */
export const DEFAULT_SLOTS: ChartSel[] = [
  { instrument: 'US100', timeframe: '15m' },
  { instrument: 'DE40', timeframe: '15m' },
  { instrument: 'US500', timeframe: '1h' },
  { instrument: 'US30', timeframe: '1h' },
]

/** Los lugares guardados que sirven; los que faltan o están rotos vuelven a los de siempre. */
export function parseSlots(raw: unknown): ChartSel[] {
  const list = Array.isArray(raw) ? (raw as Partial<ChartSel>[]) : []
  return DEFAULT_SLOTS.map((d, i) => {
    const s = list[i]
    return s &&
      typeof s.instrument === 'string' &&
      TIMEFRAMES.includes(s.timeframe as ViewTimeframe)
      ? { instrument: s.instrument, timeframe: s.timeframe as ViewTimeframe }
      : d
  })
}

/** Instrumentos y timeframes del catálogo del gateway; sin gateway, los de ejemplo. */
export function useChartCatalog() {
  const catalog = useInstruments()
  return catalog.status === 'ok'
    ? {
        instruments: catalog.data.instruments.map((i) => i.id),
        timeframes: catalog.data.timeframes,
      }
    : { instruments: TERMINAL_INSTRUMENTS, timeframes: TIMEFRAMES }
}

export type SlotData = ReturnType<typeof useChartSlot>

/**
 * Las velas de un lugar: las del motor o, sin gateway, las de ejemplo de US100 en 15 m. Con
 * `enabled` en falso (un lugar que la distribución no muestra) no se pide nada.
 */
export function useChartSlot(sel: ChartSel, enabled = true) {
  const remote = useLiquidityView(enabled ? sel : null)
  const example =
    sel.instrument === LIQUIDITY_US100.instrument_id && sel.timeframe === '15m'
      ? LIQUIDITY_US100
      : null
  const live: LiquidityViewData | null =
    remote.status === 'ok' && remote.data.candles.length > 0 ? remote.data : null
  const data = live ?? example
  return {
    sel,
    loading: remote.status === 'loading',
    live,
    data,
    source: live ? ('motor' as const) : example ? ('ejemplo' as const) : null,
  }
}
