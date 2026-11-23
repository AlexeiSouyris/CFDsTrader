/**
 * El fondo del grupo desde el gateway (fase 6b, #312), solo `GET`:
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `GET /api/v1/fund` | `fund_summary.v1`: NAV, valor total, unidades y la cuenta de cada miembro |
 * | `GET /api/v1/fund/movements` | `fund_movement.v1[]`: el historial, en orden de liquidación |
 *
 * Todo lo calcula el motor (`cfds-fund`). La UI no suma ni reparte nada: muestra lo que llega.
 * Sin gateway, el ejemplo de `mocks/`, marcado así.
 */
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'
import { FUND_EXAMPLE, FUND_HISTORY_EXAMPLE } from '@/mocks'

/** `contracts/fund_movement.v1`. */
export type FundMovement = {
  id: string
  settled_at: string
  member_id: string
  kind: 'contribution' | 'withdrawal'
  amount: string
  units: string
  nav: string
  currency: string
}

/** Una fila de `members` en `fund_summary.v1`. */
export type FundMemberSummary = {
  member_id: string
  units: string
  value: string
  net_contributions: string
  pnl: string
}

/** `contracts/fund_summary.v1`. */
export type FundSummary = {
  currency: string
  /** `last_settlement`: valor al cerrar la última liquidación del historial. */
  basis: 'last_settlement'
  valued_at: string | null
  nav: string
  total_value: string
  total_units: string
  members: FundMemberSummary[]
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as T
}

export const fetchFundSummary = (signal?: AbortSignal) =>
  getJson<FundSummary>('/api/v1/fund', signal)

export const fetchFundMovements = (signal?: AbortSignal) =>
  getJson<FundMovement[]>('/api/v1/fund/movements', signal)

export type FundData = {
  summary: FundSummary
  movements: FundMovement[]
  /** `false`: el gateway no respondió y se ve el ejemplo. */
  live: boolean
  /** Mientras llega la primera respuesta. */
  loading: boolean
}

/** El fondo del motor o, si el gateway no responde, el de ejemplo. Nunca mezcla los dos. */
export function useFund(): FundData {
  const summary = useRemote('fund-summary', fetchFundSummary, REFRESH_MS)
  const movements = useRemote('fund-movements', fetchFundMovements, REFRESH_MS)
  if (summary.status === 'ok' && movements.status === 'ok')
    return { summary: summary.data, movements: movements.data, live: true, loading: false }
  return {
    summary: FUND_EXAMPLE,
    movements: FUND_HISTORY_EXAMPLE,
    live: false,
    loading: summary.status === 'loading' || movements.status === 'loading',
  }
}

/** Moneda para mostrar: `€` para EUR, el código para las demás. */
export const currencySign = (c: string) => (c === 'EUR' ? '€' : c)
