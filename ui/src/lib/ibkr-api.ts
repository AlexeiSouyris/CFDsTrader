/**
 * Lectura de los datos de IBKR desde el gateway del motor (#551, #556). **Solo datos**: estos
 * instrumentos no se operan desde la UI (el ticket sigue con los del broker simulado).
 * - `GET /api/v1/ibkr/watchlist` (`gateway_ibkr_watchlist.v1`): último precio, variación y atraso
 *   de cada instrumento, tal como los calcula el motor;
 * - `GET /api/v1/ibkr/bars?instrument=<id>&timeframe=<tf>[&limit=<n>][&before=<UTC>]`
 *   (`gateway_ibkr_bars.v1`): velas cerradas de una sola serie (un contrato).
 * La UI pide y muestra; no calcula nada. En desarrollo, Vite manda `/api` al gateway.
 */
import { REFRESH_MS, useRemote, type Remote } from '@/lib/liquidity-api'
import type { ChartBar } from '@/lib/liquidity-bars'
import type {
  GatewayIbkrBars,
  GatewayIbkrWatchlist,
  IbkrBar,
  IbkrTimeframe,
} from '@/lib/ibkr-contracts'

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as T
}

export const fetchIbkrWatchlist = (signal?: AbortSignal) =>
  getJson<GatewayIbkrWatchlist>('/api/v1/ibkr/watchlist', signal)

export const fetchIbkrBars = (
  instrument: string,
  timeframe: IbkrTimeframe,
  signal?: AbortSignal,
  /** velas que cerraron a más tardar en este instante (UTC): para ir al pasado */
  before?: string,
) =>
  getJson<GatewayIbkrBars>(
    `/api/v1/ibkr/bars?instrument=${encodeURIComponent(instrument)}&timeframe=${timeframe}` +
      (before ? `&before=${encodeURIComponent(before)}` : ''),
    signal,
  )

const DURATION_MS: Record<IbkrTimeframe, number> = {
  '1m': 60_000,
  '15m': 15 * 60_000,
  '1h': 60 * 60_000,
}

const iso = (ms: number) => new Date(ms).toISOString().replace('.000Z', 'Z')

/**
 * Las velas de IBKR en el formato del gráfico (`ChartBar`): los precios tal cual llegan y el
 * cierre de cada vela, que el contrato no trae (`start` + su tamaño: geometría de pantalla, como
 * en `liquidity-bars.ts`, no un cálculo de trading). Función pura.
 */
export function ibkrChartBars(bars: readonly IbkrBar[]): ChartBar[] {
  return bars.map((b) => ({
    start: b.start,
    end: iso(Date.parse(b.start) + DURATION_MS[b.bar_size]),
    open: b.open,
    high: b.high,
    low: b.low,
    close: b.close,
  }))
}

/**
 * Velas más viejas para el gráfico que se desplaza al pasado: las cerradas antes de `before`, solo
 * si son del mismo contrato que las que ya se ven. Si del otro lado hay otro contrato (un futuro
 * del mes anterior), no se suman: mezclar contratos dibujaría un salto de precio que no existió.
 */
export const fetchOlderIbkrBars = (
  instrument: string,
  timeframe: IbkrTimeframe,
  contract: string,
  before: string,
): Promise<ChartBar[]> =>
  fetchIbkrBars(instrument, timeframe, undefined, before).then((data) =>
    data.series?.contract === contract ? ibkrChartBars(data.bars) : [],
  )

/** Las velas de IBKR de un instrumento y timeframe; `null` mientras no se pida ninguna. */
export function useIbkrBars(
  sel: { instrument: string; timeframe: IbkrTimeframe } | null,
): Remote<GatewayIbkrBars> {
  return useRemote(
    sel && `ibkr|${sel.instrument}|${sel.timeframe}`,
    (signal) => fetchIbkrBars(sel!.instrument, sel!.timeframe, signal),
    REFRESH_MS,
  )
}

/** Los instrumentos de IBKR que sirve el gateway, en su orden; vacío sin gateway. */
export function useIbkrInstruments(): string[] {
  const remote = useRemote('ibkr-watchlist', fetchIbkrWatchlist)
  return remote.status === 'ok' ? remote.data.instruments.map((i) => i.instrument_id) : []
}
