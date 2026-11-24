/**
 * Tipos de los contratos del broker simulado (fase 7, #278) para la UI: `order.v1`,
 * `ticket_verdict.v1`, `order_report.v1`, `position.v1` y `paper_account.v1` de `contracts/`.
 * Son la misma forma del contrato: si cambia el contrato, se cambia acá. Solo existe el modo
 * `paper`. Montos y precios viajan como texto decimal; la UI no hace cuentas con ellos.
 */

/** Texto decimal (sin float), como en el contrato. */
type Decimal = string
/** Instante UTC con zona, como "2026-11-20T21:00:00Z". */
type Utc = string

/** Motivos de rechazo del riesgo (los mismos en `ticket_verdict.v1` y `order_report.v1`). */
export type Rejection =
  | 'invalid_input'
  | 'kill_switch_active'
  | 'drawdown_limit'
  | 'daily_loss_limit'
  | 'max_open_positions'
  | 'instrument_already_open'
  | 'stop_too_close'
  | 'stop_too_far'
  | 'insufficient_margin'
  | 'below_min_deal_size'
  | 'min_deal_size_unknown'
  | 'no_price'
  | 'unknown_instrument'
  | 'stop_wrong_side'
  | 'no_recent_candles'
  | 'missing_fx_rate'

/** `order.v1`: una orden de mercado. El tamaño no va: lo decide el módulo de riesgo. */
export type Order = {
  order_id: string
  mode: 'paper'
  instrument_id: string
  side: 'long' | 'short'
  stop: Decimal
  tp1: Decimal | null
  proposal_id: string | null
  created_at: Utc
}

/** `ticket_verdict.v1`: lo que respondería el riesgo, sin enviar la orden. */
export type TicketVerdict = {
  order_id: string
  accepted: boolean
  rejection: Rejection | null
  reference: Decimal | null
  size: Decimal | null
  risk: Decimal | null
  risk_pct: Decimal | null
  leverage: Decimal | null
  margin: Decimal | null
  leverage_capped: boolean
  as_of: Utc
}

/** `order_report.v1`: qué pasó con una orden enviada. */
export type OrderReport = {
  order_id: string
  status: 'rejected' | 'filled'
  rejection: Rejection | null
  position_id: string | null
  size: Decimal | null
  fill_price: Decimal | null
  commission: Decimal | null
  at: Utc
}

/** `position.v1`: una posición abierta, valuada por el motor al cierre de la última vela. */
export type Position = {
  position_id: string
  order_id: string
  instrument_id: string
  side: 'long' | 'short'
  size: Decimal
  entry: Decimal
  stop: Decimal
  tp1: Decimal | null
  opened_at: Utc
  last: Decimal
  unrealized_pl: Decimal
  unrealized_r: Decimal
  margin: Decimal
  as_of: Utc
}

/** `paper_account.v1`: la cuenta del broker simulado. */
export type PaperAccount = {
  mode: 'paper'
  currency: 'EUR'
  as_of: Utc
  equity: Decimal
  unrealized_pl: Decimal
  realized_pl_today: Decimal
  margin_used: Decimal
  margin_free: Decimal
  leverage: Decimal
  daily_loss_used_pct: Decimal
  open_positions: number
  max_positions: number
  kill_switch: { active: false } | { active: true; reason: string; since: Utc }
}

/** Cómo se lee cada motivo de rechazo (texto de pantalla, no lógica). */
export const REJECTION_LABEL: Record<Rejection, string> = {
  invalid_input: 'Datos de la orden inválidos.',
  kill_switch_active: 'El kill switch está activo: no se abre nada.',
  drawdown_limit: 'Se alcanzó el límite de drawdown.',
  daily_loss_limit: 'Se alcanzó la pérdida máxima del día.',
  max_open_positions: 'Ya están abiertas todas las posiciones permitidas.',
  instrument_already_open: 'Ya hay una posición abierta en este instrumento.',
  stop_too_close: 'El stop está demasiado cerca.',
  stop_too_far: 'El stop está demasiado lejos.',
  insufficient_margin: 'No alcanza el margen libre.',
  below_min_deal_size: 'El tamaño queda por debajo del mínimo del broker.',
  min_deal_size_unknown: 'No se conoce el tamaño mínimo del broker para este instrumento.',
  no_price: 'No hay precio reciente para este instrumento.',
  unknown_instrument: 'Instrumento desconocido.',
  stop_wrong_side: 'El stop está del lado equivocado del precio.',
  no_recent_candles:
    'Sin velas en vivo suficientes para calcular el ATR (puede faltar cupo de IG). No se opera hasta que lleguen.',
  missing_fx_rate:
    'Falta el tipo de cambio EUR/USD de este momento para un contrato en dólares. No se opera sin él.',
}
