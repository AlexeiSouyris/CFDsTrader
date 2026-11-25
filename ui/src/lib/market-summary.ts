/**
 * El resumen de cada instrumento para la lista de mercados de la Terminal (#552): último precio,
 * hora, variación y atraso, **tal como los manda una fuente**. La UI no calcula nada: si la fuente
 * no trae la variación o el atraso, no se muestran.
 *
 * Una fuente es una función `SummarySource` que recibe los instrumentos y devuelve un
 * `MarketSummary` por cada uno. Hoy hay una sola:
 *
 * - `liquiditySummaries`: el cierre de la última vela cerrada de 15 m que ya sirve el gateway
 *   (`GET /api/v1/liquidity`, `liquidity_view.v1`). No trae variación ni atraso.
 *
 * El gateway va a servir un resumen por instrumento con precio, variación y atraso (Issue #551,
 * con su contrato en `contracts/`). Cuando exista, se agrega acá otra `SummarySource` que lo lea
 * y lo pase a `MarketSummary`, y `useMarketSummaries` la usa en lugar de esta. La lista de
 * mercados no cambia. No se inventa la ruta antes de que exista.
 */
import { fetchLiquidityView, useRemote } from '@/lib/liquidity-api'
import { chartBars } from '@/lib/liquidity-bars'
import type { Decimal, Utc, ViewTimeframe } from '@/lib/liquidity-contracts'
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
 * `source: 'ejemplo'` para decirlo en pantalla.
 */
export function useMarketSummaries(instruments: string[], source = liquiditySummaries) {
  const remote = useRemote(
    instruments.length ? `summaries|${source.label}|${instruments.join(',')}` : null,
    (signal) => source.load(instruments, signal),
    SUMMARY_REFRESH_MS,
  )
  if (remote.status === 'ok')
    return { status: 'ok' as const, label: source.label, rows: remote.data }
  if (remote.status === 'loading')
    return { status: 'loading' as const, label: source.label, rows: instruments.map(empty) }
  return {
    status: 'ejemplo' as const,
    label: 'datos de ejemplo',
    rows: exampleSummaries(instruments),
  }
}
