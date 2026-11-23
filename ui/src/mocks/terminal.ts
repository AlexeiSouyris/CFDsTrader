/**
 * DATOS DE EJEMPLO de la terminal de trading (fase 10, #275). No son precios reales y **solo hay
 * modo paper**: no hay broker real ni órdenes reales.
 *
 * Todo llega **ya calculado**, como lo manda el motor: el P/L, el margen, el apalancamiento y
 * la validación de riesgo del ticket. La UI no calcula nada (CLAUDE.md §1).
 *
 * Tienen la forma de los contratos del broker simulado (`paper_account.v1`, `position.v1` y
 * `ticket_verdict.v1`). La terminal los usa solo cuando el gateway no responde (y los tests).
 */
import type { Order, PaperAccount, Position, TicketVerdict } from '@/lib/paper-contracts'

export type {
  Order,
  OrderReport,
  PaperAccount,
  Position,
  TicketVerdict,
} from '@/lib/paper-contracts'

const iso = (ms: number) => new Date(ms).toISOString().replace('.000Z', 'Z')
const now = Date.now()

/** Un ticket tal como lo arma la persona: el tamaño no va, lo decide el módulo de riesgo. */
export type TicketDraft = {
  instrument_id: string
  side: 'long' | 'short'
  stop: string
  /** primer objetivo; vacío = sin TP1 */
  tp1: string
}

/** Instrumentos del ticket sin gateway (con gateway, los del catálogo). */
export const TERMINAL_INSTRUMENTS = ['US100', 'US500', 'US30', 'DE40']

export const PAPER_ACCOUNT: PaperAccount = {
  mode: 'paper',
  currency: 'EUR',
  as_of: iso(now),
  equity: '10342.18',
  unrealized_pl: '86.40',
  realized_pl_today: '-23.10',
  margin_used: '1046.50',
  margin_free: '9295.68',
  leverage: '2.34',
  daily_loss_used_pct: '11.6',
  open_positions: 2,
  max_positions: 5,
  kill_switch: { active: false },
}

/** El mismo estado con el kill switch activo, para ver el aviso. */
export const PAPER_ACCOUNT_HALTED: PaperAccount = {
  ...PAPER_ACCOUNT,
  kill_switch: {
    active: true,
    reason: 'Se alcanzó la pérdida máxima del día (spec §10).',
    since: iso(now - 12 * 60_000),
  },
  daily_loss_used_pct: '100.0',
}

export const PAPER_POSITIONS: Position[] = [
  {
    position_id: 'paper-demo:US500:long:1',
    order_id: 'ejemplo-1',
    instrument_id: 'US500',
    side: 'long',
    size: '1.20',
    entry: '6512.4',
    stop: '6490.1',
    tp1: '6557.0',
    opened_at: iso(now - 3 * 3_600_000),
    last: '6531.8',
    unrealized_pl: '23.28',
    unrealized_r: '0.87',
    margin: '781.49',
    as_of: iso(now),
  },
  {
    position_id: 'paper-demo:DE40:short:1',
    order_id: 'ejemplo-2',
    instrument_id: 'DE40',
    side: 'short',
    size: '0.30',
    entry: '23640.0',
    stop: '23701.5',
    tp1: '23517.0',
    opened_at: iso(now - 26 * 3_600_000),
    last: '23429.6',
    unrealized_pl: '63.12',
    unrealized_r: '3.42',
    margin: '265.01',
    as_of: iso(now),
  },
]

/**
 * Señales del motor sobre el gráfico de ejemplo (`LIQUIDITY_US100`): cierre de la vela de la
 * señal y dirección. Son marcas de ejemplo, como las que mandaría el motor con cada propuesta.
 */
export const CHART_SIGNALS: { at: string; side: 'long' | 'short'; label: string }[] = [
  { at: '2026-11-19T14:45:00Z', side: 'long', label: 'Sweep + FVG' },
  { at: '2026-11-20T09:30:00Z', side: 'short', label: 'Rechazo' },
  { at: '2026-11-20T19:15:00Z', side: 'long', label: 'CHoCH' },
]

const key = (t: { instrument_id: string; side: string; stop: string }) =>
  `${t.instrument_id}|${t.side}|${t.stop}`

/**
 * Respuestas de ejemplo del riesgo (`ticket_verdict.v1`, sin `order_id`) para los tickets de
 * ejemplo. La maqueta **no** valida otros valores: la UI no calcula riesgo, y sin motor no hay
 * respuesta.
 */
const TICKET_VERDICTS: Record<string, Omit<TicketVerdict, 'order_id'>> = {
  [key({ instrument_id: 'US100', side: 'long', stop: '28150.2' })]: {
    accepted: true,
    rejection: null,
    reference: '28204.6',
    size: '0.19',
    risk: '103.36',
    risk_pct: '1.00',
    leverage: '5.15',
    margin: '1071.67',
    leverage_capped: false,
    as_of: iso(now),
  },
  [key({ instrument_id: 'US100', side: 'long', stop: '28195.0' })]: {
    accepted: false,
    rejection: 'stop_too_close',
    reference: '28204.6',
    size: null,
    risk: null,
    risk_pct: null,
    leverage: null,
    margin: null,
    leverage_capped: false,
    as_of: iso(now),
  },
}

export const TICKET_EXAMPLES: { label: string; draft: TicketDraft }[] = [
  {
    label: 'Ejemplo que pasa',
    draft: { instrument_id: 'US100', side: 'long', stop: '28150.2', tp1: '28290.0' },
  },
  {
    label: 'Ejemplo con el stop muy cerca',
    draft: { instrument_id: 'US100', side: 'long', stop: '28195.0', tp1: '' },
  },
]

/** La "respuesta del motor" de la maqueta: solo busca en las respuestas de ejemplo. */
export const mockPreview = async (order: Order): Promise<TicketVerdict | null> => {
  const v = TICKET_VERDICTS[key(order)]
  return v ? { order_id: order.order_id, ...v } : null
}
