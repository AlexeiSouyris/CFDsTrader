/**
 * El resumen de cada instrumento para la lista de mercados de la Terminal (#552): último precio,
 * hora, variación y atraso, **tal como los manda una fuente**. La UI no calcula nada: si la fuente
 * no trae la variación o el atraso, no se muestran.
 *
 * Una fuente es una función `SummarySource` que recibe los instrumentos y devuelve un
 * `MarketSummary` por cada uno. Hay dos:
 *
 * - `liquiditySummaries`: el cierre de la última vela cerrada de 15 m que ya sirve el gateway
 *   (`GET /api/v1/liquidity`, `liquidity_view.v1`) para los instrumentos del broker simulado. No
 *   trae variación ni atraso.
 * - `ibkrSummaries` (#556): el resumen de IBKR del gateway (`GET /api/v1/ibkr/watchlist`,
 *   `gateway_ibkr_watchlist.v1`) con último precio, variación, hora y atraso, tal como los manda
 *   el motor. Trae su propia lista de instrumentos (`own`). Son solo datos: no se operan.
 */
import { fetchLiquidityView, useRemote } from '@/lib/liquidity-api'
import { chartBars } from '@/lib/liquidity-bars'
import type { Decimal, Utc, ViewTimeframe } from '@/lib/liquidity-contracts'
import { fetchIbkrWatchlist } from '@/lib/ibkr-api'
import { LIQUIDITY_US100 } from '@/mocks'

export type MarketSummary = {
  instrument_id: string
  /** último precio (o cierre) tal como llega; `null` si la fuente no tiene */
  last: Decimal | null
  /** hora (UTC) de ese precio: el cierre de su vela */
  at: Utc | null
  /** variación en %, solo si la fuente la manda */
  change_pct: Decimal | null
  /** atraso que declara la fuente, en segundos; `null` si no lo declara */
  delay_s: number | null
}

/** Una fuente de resúmenes: un resumen por instrumento, en el mismo orden. */
export type SummarySource = {
  /** cómo se nombra la fuente en la lista (por ejemplo, "último cierre de 15 m del motor") */
  label: string
  load: (instruments: string[], signal: AbortSignal) => Promise<MarketSummary[]>
  /**
   * La fuente trae su propia lista: con `instruments` vacío devuelve todos los suyos, en su orden.
   * Sin gateway no tiene datos de ejemplo.
   */
  own?: boolean
}

const TIMEFRAME: ViewTimeframe = '15m'

const empty = (instrument_id: string): MarketSummary => ({
  instrument_id,
  last: null,
  at: null,
  change_pct: null,
  delay_s: null,
})

/**
 * El cierre de la última vela cerrada de cada instrumento, de la vista de liquidez del gateway.
 * Un instrumento sin velas queda vacío; si no responde ninguno, es un error (sin gateway).
 */
export const liquiditySummaries: SummarySource = {
  label: 'último cierre de 15 m del motor',
  async load(instruments, signal) {
    const results = await Promise.allSettled(
      instruments.map((id) => fetchLiquidityView(id, TIMEFRAME, signal)),
    )
    if (results.length > 0 && results.every((r) => r.status === 'rejected'))
      throw (results[0] as PromiseRejectedResult).reason
    return results.map((r, i) => {
      if (r.status === 'rejected') return empty(instruments[i])
      const last = chartBars(r.value).at(-1)
      return last
        ? { ...empty(instruments[i]), last: last.close, at: last.end }
        : empty(instruments[i])
    })
  },
}

/**
 * El resumen de IBKR, tal cual llega: último, hora del cierre de esa vela, variación en % y atraso
 * en segundos. Un instrumento sin velas (`quote: null`) queda vacío. Con `instruments` vacío,
 * todos los del gateway; si no, esos, en ese orden.
 */
export const ibkrSummaries: SummarySource = {
  label: 'IBKR · solo datos',
  own: true,
  async load(instruments, signal) {
    const data = await fetchIbkrWatchlist(signal)
    const ids = instruments.length ? instruments : data.instruments.map((i) => i.instrument_id)
    return ids.map((id) => {
      const q = data.instruments.find((i) => i.instrument_id === id)?.quote
      return q
        ? {
            instrument_id: id,
            last: q.last,
            at: q.bar_end,
            change_pct: q.change_pct,
            delay_s: q.delay_seconds,
          }
        : empty(id)
    })
  },
}

/** Sin gateway: el cierre de la última vela del ejemplo (solo US100), marcado como ejemplo. */
export function exampleSummaries(instruments: string[]): MarketSummary[] {
  const last = chartBars(LIQUIDITY_US100).at(-1)
  return instruments.map((id) =>
    id === LIQUIDITY_US100.instrument_id && last
      ? { ...empty(id), last: last.close, at: last.end }
      : empty(id),
  )
}

/** Cada cuánto se vuelven a pedir los resúmenes. */
export const SUMMARY_REFRESH_MS = 60_000

/**
 * Los resúmenes de la lista de mercados. Con la fuente caída, los de ejemplo, con
 * `status: 'ejemplo'` para decirlo en pantalla; una fuente con lista propia (`own`) no tiene
 * ejemplo: `status: 'error'` y sin renglones.
 */
export function useMarketSummaries(instruments: string[], source = liquiditySummaries) {
  const remote = useRemote(
    source.own || instruments.length ? `summaries|${source.label}|${instruments.join(',')}` : null,
    (signal) => source.load(instruments, signal),
    SUMMARY_REFRESH_MS,
  )
  if (remote.status === 'ok')
    return { status: 'ok' as const, label: source.label, rows: remote.data }
  if (remote.status === 'loading')
    return {
      status: 'loading' as const,
      label: source.label,
      rows: source.own && !instruments.length ? [] : instruments.map(empty),
    }
  if (source.own) return { status: 'error' as const, label: source.label, rows: [] }
  return {
    status: 'ejemplo' as const,
    label: 'datos de ejemplo',
    rows: exampleSummaries(instruments),
  }
}
