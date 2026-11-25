/**
 * Lectura de la vista de liquidez desde el gateway del motor (#231):
 * - `GET /api/v1/instruments` (`contracts/gateway_instruments.v1`): instrumentos del catálogo,
 *   timeframes y lo que se abre primero;
 * - `GET /api/v1/liquidity?instrument=<id>&timeframe=<tf>[&source=futures][&before=<UTC>]`
 *   (`contracts/liquidity_view.v1`). Sin `source`, las velas del CFD de IG; con `auto` (#628), IG
 *   o, si IG no tiene velas, la fuente de reemplazo que diga el motor.
 * La UI solo pide y muestra; no calcula nada. En desarrollo, Vite manda `/api` al gateway.
 */
import { useEffect, useState } from 'react'

import type { ChartAnalysis } from '@/lib/analysis-contracts'
import { chartBars, type ChartBar } from '@/lib/liquidity-bars'
import type {
  GatewayInstruments,
  LiquidityViewData,
  SourceChoice,
  ViewSource,
  ViewTimeframe,
} from '@/lib/liquidity-contracts'

/** Cada cuánto se vuelve a pedir la vista. Con velas de 15 minutos o más, un minuto alcanza. */
export const REFRESH_MS = 60_000

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as T
}

export const fetchInstruments = (signal?: AbortSignal) =>
  getJson<GatewayInstruments>('/api/v1/instruments', signal)

export const fetchLiquidityView = (
  instrument: string,
  timeframe: ViewTimeframe,
  signal?: AbortSignal,
  source: SourceChoice = 'ig',
  /** la vista tal como se conocía en ese momento (UTC): velas más viejas (#401) */
  before?: string,
) =>
  getJson<LiquidityViewData>(
    `/api/v1/liquidity?instrument=${encodeURIComponent(instrument)}&timeframe=${timeframe}` +
      (source === 'ig' ? '' : `&source=${source}`) +
      (before ? `&before=${encodeURIComponent(before)}` : ''),
    signal,
  )

/** Una página de velas más viejas con su bloque `analysis` (índices de `bars`), si lo trae. */
export type OlderPage = { bars: ChartBar[]; analysis?: ChartAnalysis }

/**
 * Velas cerradas antes de `before` (#401), para el gráfico que se desplaza al pasado, con las
 * herramientas de análisis del motor para esas velas (#511).
 */
export const fetchOlderBars = (
  instrument: string,
  timeframe: ViewTimeframe,
  before: string,
  source: ViewSource['id'] = 'ig',
): Promise<OlderPage> =>
  fetchLiquidityView(instrument, timeframe, undefined, source, before).then((data) => ({
    bars: chartBars(data),
    analysis: data.analysis,
  }))

export type Remote<T> =
  { status: 'loading' } | { status: 'error'; message: string } | { status: 'ok'; data: T }

/**
 * Pide `load` al montar y, si `every` está, cada `every` ms. Un error no borra lo último que
 * llegó. Cambiar `key` vuelve a empezar; cambiar `nonce` vuelve a pedir sin borrar lo que hay.
 */
export function useRemote<T>(
  key: string | null,
  load: (signal: AbortSignal) => Promise<T>,
  every?: number,
  nonce = 0,
): Remote<T> {
  const [state, setState] = useState<{ key: string | null; value: Remote<T> }>({
    key,
    value: { status: 'loading' },
  })
  useEffect(() => {
    if (key === null) return
    let ctrl = new AbortController()
    const run = () => {
      ctrl.abort()
      ctrl = new AbortController()
      const signal = ctrl.signal
      load(signal).then(
        (data) => setState({ key, value: { status: 'ok', data } }),
        (e: unknown) => {
          if (signal.aborted) return
          const message = e instanceof Error ? e.message : String(e)
          setState((prev) =>
            prev.key === key && prev.value.status === 'ok'
              ? prev
              : { key, value: { status: 'error', message } },
          )
        },
      )
    }
    run()
    const timer = every ? window.setInterval(run, every) : undefined
    return () => {
      window.clearInterval(timer)
      ctrl.abort()
    }
    // `load` depende solo de `key`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, every, nonce])
  return state.key === key ? state.value : { status: 'loading' }
}

export const useInstruments = () => useRemote('instruments', fetchInstruments)

/** La vista de un instrumento, timeframe y fuente; `null` mientras no se sepa cuál. */
export function useLiquidityView(
  sel: { instrument: string; timeframe: ViewTimeframe; source?: SourceChoice } | null,
): Remote<LiquidityViewData> {
  return useRemote(
    sel && `${sel.instrument}|${sel.timeframe}|${sel.source ?? 'ig'}`,
    (signal) => fetchLiquidityView(sel!.instrument, sel!.timeframe, signal, sel!.source),
    REFRESH_MS,
  )
}
