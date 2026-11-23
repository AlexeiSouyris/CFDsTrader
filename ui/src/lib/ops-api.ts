/**
 * Estado de la operación desde el gateway (#314), solo `GET`:
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `GET /api/v1/ops/status` | `ops_status.v1`: compuerta, kill switch, pérdida diaria, apalancamiento y última reconciliación |
 * | `GET /api/v1/ops/events` | `ops_event.v1[]`: los últimos eventos del broker y alertas, el más nuevo primero |
 *
 * Todo lo calcula el motor. La UI lo muestra tal cual y **no** puede reiniciar el kill switch:
 * no existe una ruta para eso. Sin gateway no se inventa un estado: se dice que no hay conexión.
 */
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'
import { PAPER_REFRESH_MS } from '@/lib/paper-api'

/** Hora ISO 8601 en UTC (`...Z`). */
type Utc = string

export type Gate = 'starting' | 'ready' | 'stopping' | 'unavailable'

export type OpsKillSwitch = { active: false } | { active: true; reason: string; since: Utc }

/** `account` de `ops_status.v1`. */
export type OpsAccount = {
  kill_switch: OpsKillSwitch
  /** Parte usada de la pérdida máxima diaria, en % (puede pasar de 100). */
  daily_loss_used_pct: string
  /** La pérdida máxima diaria, en % del capital al empezar el día (`config/risk.toml`). */
  daily_loss_limit_pct: string
  leverage: string
  open_positions: number
  max_positions: number
}

export type OpsReconcile = {
  kind: 'startup' | 'periodic'
  at: Utc
  clean: boolean
  matched: number
  mismatches: number
  drifts: number
}

/** `contracts/ops_status.v1`. */
export type OpsStatus = {
  as_of: Utc
  mode: 'paper'
  gate: Gate
  accepting_orders: boolean
  account: OpsAccount | null
  last_reconcile: OpsReconcile | null
}

/** `contracts/ops_event.v1`: una línea del log ya sin secretos. */
export type OpsEvent = {
  ts: Utc
  level: 'info' | 'warn' | 'error'
  target: string
  event: string
  fields: Record<string, unknown>
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as T
}

export const fetchOpsStatus = (signal?: AbortSignal) =>
  getJson<OpsStatus>('/api/v1/ops/status', signal)

export const fetchOpsEvents = (signal?: AbortSignal) =>
  getJson<OpsEvent[]>('/api/v1/ops/events', signal)

/** El estado se vuelve a pedir tan seguido como la cuenta del broker. */
export const useOpsStatus = (refresh = 0) =>
  useRemote('ops-status', fetchOpsStatus, PAPER_REFRESH_MS, refresh)

export const useOpsEvents = () => useRemote('ops-events', fetchOpsEvents, REFRESH_MS / 4)
