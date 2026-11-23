/**
 * Lectura de la vista de liquidez desde el gateway del motor (#231):
 * - `GET /api/v1/instruments` (`contracts/gateway_instruments.v1`): instrumentos del catálogo,
 *   timeframes y lo que se abre primero;
 * - `GET /api/v1/liquidity?instrument=<id>&timeframe=<tf>[&source=futures]`
 *   (`contracts/liquidity_view.v1`). Sin `source`, las velas del CFD de IG.
 * La UI solo pide y muestra; no calcula nada. En desarrollo, Vite manda `/api` al gateway.
 */
import { useEffect, useState } from 'react'

import type {
  GatewayInstruments,
  LiquidityViewData,
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
  source: ViewSource['id'] = 'ig',
) =>
  getJson<LiquidityViewData>(
    `/api/v1/liquidity?instrument=${encodeURIComponent(instrument)}&timeframe=${timeframe}` +
      (source === 'ig' ? '' : `&source=${source}`),
    signal,
  )

export type Remote<T> =
  { status: 'loading' } | { status: 'error'; message: string } | { status: 'ok'; data: T }

/**
 * Pide `load` al montar y, si `every` está, cada `every` ms. Un error no borra lo último que
 * llegó. Cambiar `key` vuelve a empezar.
 */
function useRemote<T>(
  key: string | null,
  load: (signal: AbortSignal) => Promise<T>,
  every?: number,
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
  }, [key, every])
  return state.key === key ? state.value : { status: 'loading' }
}

export const useInstruments = () => useRemote('instruments', fetchInstruments)

/** La vista de un instrumento, timeframe y fuente; `null` mientras no se sepa cuál. */
export function useLiquidityView(
  sel: { instrument: string; timeframe: ViewTimeframe; source?: ViewSource['id'] } | null,
): Remote<LiquidityViewData> {
  return useRemote(
    sel && `${sel.instrument}|${sel.timeframe}|${sel.source ?? 'ig'}`,
    (signal) => fetchLiquidityView(sel!.instrument, sel!.timeframe, signal, sel!.source),
    REFRESH_MS,
  )
}
