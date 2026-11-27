/**
 * Velas para dibujar (#231): `candle.v1` trae solo el inicio y `daily_candle.v1` trae inicio y
 * cierre. El gráfico necesita los dos para ubicar cada evento en la vela que ya cerró. Sumar la
 * duración del timeframe al inicio es geometría de pantalla, no un cálculo de trading.
 */
import type { LiquidityViewData, ViewTimeframe } from '@/lib/liquidity-contracts'

/** Timeframes que dibuja el gráfico: los de la vista de liquidez y `1m`, solo de IBKR (#556). */
export type ChartTimeframe = ViewTimeframe | '1m'

export type ChartBar = {
  start: string
  end: string
  open: string
  high: string
  low: string
  close: string
}

const DURATION_MS = { '15m': 15 * 60_000, '1h': 60 * 60_000 } as const

const iso = (ms: number) => new Date(ms).toISOString().replace('.000Z', 'Z')

export function chartBars(data: LiquidityViewData): ChartBar[] {
  const tf = data.timeframe === '1h' ? '1h' : '15m'
  return data.candles.map((c) => ({
    start: c.start,
    end: 'end' in c ? c.end : iso(Date.parse(c.start) + DURATION_MS[tf]),
    open: c.open,
    high: c.high,
    low: c.low,
    close: c.close,
  }))
}

/**
 * Une velas por su inicio (las de `next` reemplazan a las de `prev`), en orden (#401): las más
 * viejas que se piden al desplazarse al pasado con las que llegan en cada actualización.
 */
export function mergeBars(prev: ChartBar[], next: ChartBar[]): ChartBar[] {
  const by = new Map<string, ChartBar>()
  for (const b of prev) by.set(b.start, b)
  for (const b of next) by.set(b.start, b)
  return [...by.values()].sort((x, y) => Date.parse(x.start) - Date.parse(y.start))
}
