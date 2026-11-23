/**
 * Velas para dibujar (#231): `candle.v1` trae solo el inicio y `daily_candle.v1` trae inicio y
 * cierre. El gráfico necesita los dos para ubicar cada evento en la vela que ya cerró. Sumar la
 * duración del timeframe al inicio es geometría de pantalla, no un cálculo de trading.
 */
import type { LiquidityViewData } from '@/lib/liquidity-contracts'

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
