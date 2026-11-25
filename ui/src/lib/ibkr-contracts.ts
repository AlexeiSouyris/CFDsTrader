/**
 * Tipos de lo que sirve el gateway con los datos de IBKR (#551, #556), copiados de los contratos:
 * - `contracts/gateway_ibkr_watchlist.v1.schema.json` (`GET /api/v1/ibkr/watchlist`);
 * - `contracts/gateway_ibkr_bars.v1.schema.json` (`GET /api/v1/ibkr/bars`), con sus velas
 *   `contracts/ibkr_bar.v1.schema.json`.
 * Los precios son decimales exactos en texto y las horas, UTC terminadas en Z. Si un contrato
 * cambia, cambian estos tipos en el mismo PR.
 */
import type { Decimal, Utc } from '@/lib/liquidity-contracts'

/** Tamaños de vela de IBKR (enum de `ibkr_bar.v1` `bar_size`). */
export type IbkrTimeframe = '1m' | '15m' | '1h'
export const IBKR_TIMEFRAMES: readonly IbkrTimeframe[] = ['1m', '15m', '1h']

export type IbkrKind = 'future' | 'stock' | 'forex'
export type IbkrWhatToShow = 'TRADES' | 'MIDPOINT'

/** `ibkr_bar.v1`: una vela cerrada, con solo su inicio. */
export type IbkrBar = {
  source: string
  instrument_id: string
  kind: IbkrKind
  contract: string
  bar_size: IbkrTimeframe
  what_to_show: IbkrWhatToShow
  start: Utc
  open: Decimal
  high: Decimal
  low: Decimal
  close: Decimal
  /** `null` si IBKR no lo da (por ejemplo, MIDPOINT) */
  volume: Decimal | null
}

export type IbkrSeries = { contract: string; kind: IbkrKind; what_to_show: IbkrWhatToShow }

/** `gateway_ibkr_bars.v1`. */
export type GatewayIbkrBars = {
  instrument_id: string
  timeframe: IbkrTimeframe
  source: string
  as_of: Utc
  generated_at: Utc
  /** `null` si no hay ninguna vela cerrada a `as_of` (y entonces `bars` viene vacío) */
  series: IbkrSeries | null
  bars: IbkrBar[]
}

/** `quote` de `gateway_ibkr_watchlist.v1`. */
export type IbkrQuote = {
  contract: string
  kind: IbkrKind
  what_to_show: IbkrWhatToShow
  timeframe: IbkrTimeframe
  last: Decimal
  bar_start: Utc
  bar_end: Utc
  delay_seconds: number
  session: string
  previous_session: string | null
  previous_close: Decimal | null
  change: Decimal | null
  change_pct: Decimal | null
}

/** `gateway_ibkr_watchlist.v1`. */
export type GatewayIbkrWatchlist = {
  generated_at: Utc
  source: string
  instruments: { instrument_id: string; quote: IbkrQuote | null }[]
}
