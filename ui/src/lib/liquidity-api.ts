/**
 * Lectura de la vista de liquidez desde el gateway del motor (#231): `GET /api/v1/liquidity`,
 * que responde `contracts/liquidity_view.v1`. La UI solo pide y muestra; no calcula nada.
 * En desarrollo, Vite manda `/api` al gateway (ver `vite.config.ts`).
 */
import { useEffect, useState } from 'react'

import type { LiquidityViewData } from '@/lib/liquidity-contracts'

/** Cada cuánto se vuelve a pedir: las velas son de 15 minutos, así que con un minuto alcanza. */
export const REFRESH_MS = 60_000

export async function fetchLiquidityView(
  instrument: string,
  signal?: AbortSignal,
): Promise<LiquidityViewData> {
  const res = await fetch(`/api/v1/liquidity?instrument=${encodeURIComponent(instrument)}`, {
    signal,
    headers: { Accept: 'application/json' },
  })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as LiquidityViewData
}

export type LiquidityState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ok'; data: LiquidityViewData }

/** Pide la vista al montar y cada `REFRESH_MS`. Un error no borra lo último que llegó. */
export function useLiquidityView(instrument: string): LiquidityState {
  const [state, setState] = useState<LiquidityState>({ status: 'loading' })
  useEffect(() => {
    let ctrl = new AbortController()
    const load = () => {
      ctrl.abort()
      ctrl = new AbortController()
      fetchLiquidityView(instrument, ctrl.signal).then(
        (data) => setState({ status: 'ok', data }),
        (e: unknown) => {
          if (ctrl.signal.aborted) return
          const message = e instanceof Error ? e.message : String(e)
          setState((prev) => (prev.status === 'ok' ? prev : { status: 'error', message }))
        },
      )
    }
    load()
    const timer = window.setInterval(load, REFRESH_MS)
    return () => {
      window.clearInterval(timer)
      ctrl.abort()
    }
  }, [instrument])
  return state
}
