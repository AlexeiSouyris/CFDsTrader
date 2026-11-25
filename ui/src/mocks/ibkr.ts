/**
 * Respuestas de prueba del gateway con datos de IBKR (#556), con la forma de
 * `contracts/gateway_ibkr_watchlist.v1.schema.json` y `contracts/gateway_ibkr_bars.v1.schema.json`.
 * Solo para los tests: no son precios reales y la UI no las muestra como si lo fueran.
 */
import type { GatewayIbkrBars, GatewayIbkrWatchlist, IbkrTimeframe } from '@/lib/ibkr-contracts'

export const IBKR_WATCHLIST: GatewayIbkrWatchlist = {
  generated_at: '2026-11-24T15:15:00Z',
  source: 'ibkr',
  instruments: [
    {
      instrument_id: 'MNQ',
      quote: {
        contract: 'MNQZ6',
        kind: 'future',
        what_to_show: 'TRADES',
        timeframe: '1m',
        last: '21000.25',
        bar_start: '2026-11-24T15:00:00Z',
        bar_end: '2026-11-24T15:01:00Z',
        delay_seconds: 840,
        session: '2026-11-24',
        previous_session: '2026-11-23',
        previous_close: '21100.5',
        change: '-100.25',
        change_pct: '-0.48',
      },
    },
    {
      instrument_id: 'MES',
      quote: {
        contract: 'MESZ6',
        kind: 'future',
        what_to_show: 'TRADES',
        timeframe: '1m',
        last: '6000.5',
        bar_start: '2026-11-24T15:00:00Z',
        bar_end: '2026-11-24T15:01:00Z',
        delay_seconds: 840,
        session: '2026-11-24',
        previous_session: null,
        previous_close: null,
        change: null,
        change_pct: null,
      },
    },
    { instrument_id: 'QQQ', quote: null },
  ],
}

const BAR_MS: Record<IbkrTimeframe, number> = { '1m': 60_000, '15m': 900_000, '1h': 3_600_000 }

/** `count` velas cerradas de MNQ (contrato `contract`) que terminan antes de `end`. */
export function ibkrBars(
  timeframe: IbkrTimeframe,
  opts: { count?: number; end?: string; contract?: string; instrument?: string } = {},
): GatewayIbkrBars {
  const { count = 3, end = '2026-11-24T15:00:00Z', contract = 'MNQZ6', instrument = 'MNQ' } = opts
  const last = Date.parse(end) - BAR_MS[timeframe]
  const bars = Array.from({ length: count }, (_, i) => {
    const start = new Date(last - (count - 1 - i) * BAR_MS[timeframe])
      .toISOString()
      .replace('.000Z', 'Z')
    const base = 21000 + i
    return {
      source: 'ibkr',
      instrument_id: instrument,
      kind: 'future' as const,
      contract,
      bar_size: timeframe,
      what_to_show: 'TRADES' as const,
      start,
      open: `${base}.25`,
      high: `${base + 2}.5`,
      low: `${base - 1}.75`,
      close: `${base + 1}`,
      volume: '120',
    }
  })
  return {
    instrument_id: instrument,
    timeframe,
    source: 'ibkr',
    as_of: end,
    generated_at: end,
    series: count ? { contract, kind: 'future', what_to_show: 'TRADES' } : null,
    bars,
  }
}
