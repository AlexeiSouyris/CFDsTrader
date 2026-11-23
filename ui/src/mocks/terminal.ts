/**
 * DATOS DE EJEMPLO de la terminal de trading (fase 10, #275). No son precios reales y **solo hay
 * modo paper**: no hay broker ni órdenes.
 *
 * Todo llega **ya calculado**, como lo mandaría el motor: el P/L, el margen, el apalancamiento y
 * la validación de riesgo del ticket. La UI no calcula nada (CLAUDE.md §1).
 *
 * TODO(#275): no existen todavía contratos de cuenta, posición ni validación de orden para la UI.
 * Estos tipos son provisorios; cuando el motor los publique (`contracts/`), se reemplazan por los
 * del contrato. Las propuestas sí usan `proposal.v1` (ver `PROPOSALS`).
 */

const iso = (ms: number) => new Date(ms).toISOString().replace('.000Z', 'Z')
const now = Date.now()

/** Estado de la cuenta de simulación (futuro contrato de cuenta). */
export type PaperAccount = {
  mode: 'paper'
  currency: 'EUR'
  as_of: string
  /** capital realizado + no realizado */
  equity: string
  /** P/L no realizado de las posiciones abiertas */
  unrealized_pl: string
  /** P/L realizado del día */
  realized_pl_today: string
  margin_used: string
  margin_free: string
  /** nocional total / capital */
  leverage: string
  /** parte usada de la pérdida máxima diaria (spec §10), en % */
  daily_loss_used_pct: string
  open_positions: number
  max_positions: number
  kill_switch: { active: false } | { active: true; reason: string; since: string }
}

/** Una posición abierta (futuro contrato de posición). */
export type PaperPosition = {
  id: string
  instrument_id: string
  side: 'long' | 'short'
  /** € por punto */
  size: string
  entry: string
  stop: string
  tp1: string
  last: string
  unrealized_pl: string
  /** en múltiplos de R */
  unrealized_r: string
  margin: string
  leverage: string
  opened_at: string
}

/** Un chequeo de riesgo del motor (spec §7 y §8). */
export type RiskCheck = { rule: string; ok: boolean; detail: string }

/** Respuesta del motor a un ticket (futuro contrato de validación de orden). */
export type TicketVerdict = {
  accepted: boolean
  risk_eur: string
  risk_pct: string
  leverage: string
  margin_required: string
  checks: RiskCheck[]
}

/** Un ticket tal como lo arma la persona. */
export type TicketDraft = {
  instrument_id: string
  side: 'long' | 'short'
  size: string
  stop: string
}

/** Instrumentos del ticket de ejemplo (en la app real, los del catálogo que mande el gateway). */
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

export const PAPER_POSITIONS: PaperPosition[] = [
  {
    id: 'paper-demo:US500:long:1',
    instrument_id: 'US500',
    side: 'long',
    size: '1.20',
    entry: '6512.4',
    stop: '6490.1',
    tp1: '6557.0',
    last: '6531.8',
    unrealized_pl: '23.28',
    unrealized_r: '0.87',
    margin: '781.49',
    leverage: '0.76',
    opened_at: iso(now - 3 * 3_600_000),
  },
  {
    id: 'paper-demo:DE40:short:1',
    instrument_id: 'DE40',
    side: 'short',
    size: '0.30',
    entry: '23640.0',
    stop: '23701.5',
    tp1: '23517.0',
    last: '23429.6',
    unrealized_pl: '63.12',
    unrealized_r: '3.42',
    margin: '265.01',
    leverage: '0.69',
    opened_at: iso(now - 26 * 3_600_000),
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

const key = (t: TicketDraft) => `${t.instrument_id}|${t.side}|${t.size}|${t.stop}`

/**
 * Respuestas de ejemplo del motor para los tickets de ejemplo. La maqueta **no** valida otros
 * valores: la UI no calcula riesgo, y sin motor no hay respuesta.
 */
export const TICKET_VERDICTS: Record<string, TicketVerdict> = {
  [key({ instrument_id: 'US100', side: 'long', size: '0.19', stop: '28150.2' })]: {
    accepted: true,
    risk_eur: '103.36',
    risk_pct: '1.00',
    leverage: '5.15',
    margin_required: '1071.67',
    checks: [
      { rule: 'Stop obligatorio', ok: true, detail: 'Stop en 28.150,2.' },
      { rule: 'Distancia al stop (§7.7)', ok: true, detail: '1,4 × ATR: entre 0,5 y 3.' },
      { rule: 'Riesgo por operación', ok: true, detail: '1,00 % del capital.' },
      { rule: 'Apalancamiento del instrumento', ok: true, detail: '5,15:1, tope 10:1.' },
      { rule: 'Posiciones abiertas', ok: true, detail: '2 de 5; ninguna en US100.' },
      { rule: 'Margen libre', ok: true, detail: 'Alcanza.' },
    ],
  },
  [key({ instrument_id: 'US100', side: 'long', size: '0.19', stop: '28195.0' })]: {
    accepted: false,
    risk_eur: '1.75',
    risk_pct: '0.02',
    leverage: '5.15',
    margin_required: '1071.67',
    checks: [
      { rule: 'Stop obligatorio', ok: true, detail: 'Stop en 28.195,0.' },
      {
        rule: 'Distancia al stop (§7.7)',
        ok: false,
        detail: '0,2 × ATR: menor que 0,5 × ATR. No se abre.',
      },
      { rule: 'Riesgo por operación', ok: true, detail: '0,02 % del capital.' },
      { rule: 'Apalancamiento del instrumento', ok: true, detail: '5,15:1, tope 10:1.' },
    ],
  },
}

export const TICKET_EXAMPLES: { label: string; draft: TicketDraft }[] = [
  {
    label: 'Ejemplo que pasa',
    draft: { instrument_id: 'US100', side: 'long', size: '0.19', stop: '28150.2' },
  },
  {
    label: 'Ejemplo con el stop muy cerca',
    draft: { instrument_id: 'US100', side: 'long', size: '0.19', stop: '28195.0' },
  },
]

/** La "respuesta del motor" de la maqueta: solo busca en las respuestas de ejemplo. */
export const mockEngineVerdict = (draft: TicketDraft): TicketVerdict | null =>
  TICKET_VERDICTS[key(draft)] ?? null
