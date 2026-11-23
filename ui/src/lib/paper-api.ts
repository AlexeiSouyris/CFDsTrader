/**
 * El broker simulado del gateway (fase 7 y 10, #278 y #275). **Solo paper**: no existe el modo
 * real. La UI pide y muestra; el riesgo, el tamaño, el P/L y el margen los calcula el motor.
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `POST /api/v1/paper/preview` | `order.v1` → `ticket_verdict.v1` (sin efectos) |
 * | `POST /api/v1/paper/orders` | `order.v1` → `order_report.v1` (idempotente por `order_id`) |
 * | `GET /api/v1/paper/positions` | `position.v1[]` |
 * | `GET /api/v1/paper/account` | `paper_account.v1` |
 * | `GET /api/v1/backtest_runs` | `backtest_run.v1[]` |
 * | `GET /api/v1/backtest_runs/{run_id}` | `backtest_run_detail.v1` (#299) |
 *
 * Los `POST` llevan la cabecera anti-CSRF `x-cfds-token`, pero **no la pone el navegador**: la
 * agrega el proxy de desarrollo de Vite desde la variable `CFDS_GATEWAY_TOKEN` (ver
 * `vite.config.ts`). El token nunca está en el código del cliente.
 */
import type { BacktestRun, RunDetail } from '@/lib/backtest-contracts'
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'
import type {
  Order,
  OrderReport,
  PaperAccount,
  Position,
  TicketVerdict,
} from '@/lib/paper-contracts'

/** Un error del gateway con su código y el texto que mandó (por ejemplo, 403 sin token). */
export class GatewayError extends Error {
  readonly status: number
  constructor(status: number, detail: string) {
    super(detail ? `el gateway respondió ${status}: ${detail}` : `el gateway respondió ${status}`)
    this.status = status
  }
}

async function request<T>(url: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(url, {
    ...init,
    headers: { Accept: 'application/json', ...init.headers },
  })
  if (!res.ok) {
    const detail = await res.text().catch(() => '')
    throw new GatewayError(res.status, detail.trim().slice(0, 200))
  }
  return (await res.json()) as T
}

const post = <T>(url: string, order: Order) =>
  request<T>(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(order),
  })

/** Lo que respondería el riesgo a la orden, sin enviarla. */
export const previewOrder = (order: Order) => post<TicketVerdict>('/api/v1/paper/preview', order)

/** Envía la orden al broker simulado. Repetir el mismo `order_id` no abre dos posiciones. */
export const submitOrder = (order: Order) => post<OrderReport>('/api/v1/paper/orders', order)

export const fetchPositions = (signal?: AbortSignal) =>
  request<Position[]>('/api/v1/paper/positions', { signal })
export const fetchAccount = (signal?: AbortSignal) =>
  request<PaperAccount>('/api/v1/paper/account', { signal })
export const fetchBacktestRuns = (signal?: AbortSignal) =>
  request<BacktestRun[]>('/api/v1/backtest_runs', { signal })

/** Cada cuánto se vuelven a pedir la cuenta y las posiciones (las valúa el motor por vela). */
export const PAPER_REFRESH_MS = 15_000

/** Cambiar `refresh` vuelve a pedir enseguida (después de enviar una orden). */
export const usePaperAccount = (refresh = 0) =>
  useRemote('paper-account', fetchAccount, PAPER_REFRESH_MS, refresh)
export const usePaperPositions = (refresh = 0) =>
  useRemote('paper-positions', fetchPositions, PAPER_REFRESH_MS, refresh)
export const useBacktestRuns = () => useRemote('backtest_runs', fetchBacktestRuns, REFRESH_MS)

/** El detalle de una corrida (#299). Una corrida terminada no cambia: se pide una vez. */
export const fetchRunDetail = (runId: string, signal?: AbortSignal) =>
  request<RunDetail>(`/api/v1/backtest_runs/${encodeURIComponent(runId)}`, { signal })
export const useRunDetail = (runId: string | null) =>
  useRemote(runId && `run-detail|${runId}`, (signal) => fetchRunDetail(runId!, signal))

/** Una orden nueva de la persona (ticket manual): id propio, modo paper y hora de ahora. */
export function newOrder(ticket: {
  instrument_id: string
  side: 'long' | 'short'
  stop: string
  tp1: string | null
}): Order {
  const rand = Math.random().toString(36).slice(2, 10)
  return {
    order_id: `ui-${Date.now().toString(36)}-${rand}`,
    mode: 'paper',
    instrument_id: ticket.instrument_id,
    side: ticket.side,
    stop: ticket.stop,
    tp1: ticket.tp1,
    proposal_id: null,
    created_at: new Date().toISOString(),
  }
}
