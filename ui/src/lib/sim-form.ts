/**
 * Formularios de los simuladores (#806, #807): del texto que escribe la persona al pedido del contrato.
 * Solo forma y formato (decimales del contrato, porcentaje → fracción moviendo la coma): los rangos
 * y todo cálculo los juzga el motor (CLAUDE.md §1).
 */
import type { DrawdownMode, PropRules, PropSimRequest, VarianceRequest } from '@/lib/sim-contracts'

/** Fuente oficial de la cifra de cuentas minoristas de CFD que pierden dinero. */
export const ESMA_URL =
  'https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf'

export const dec = (s: string) => s.trim().replace(',', '.')
export const isDecimal = (s: string) => /^\d+(\.\d+)?$/.test(s)
export const isInt = (s: string) => /^\d{1,9}$/.test(s.trim())

/**
 * Un porcentaje escrito ("45" o "45,5") como fracción del contrato ("0.45", "0.455"), moviendo la
 * coma dos lugares: exacto, sin float.
 */
export function pctToFraction(text: string): string | null {
  const s = dec(text)
  if (!isDecimal(s)) return null
  const [int, frac = ''] = s.split('.')
  const padded = int.padStart(3, '0')
  const whole = padded.slice(0, -2).replace(/^0+(?=\d)/, '')
  return `${whole}.${padded.slice(-2)}${frac}`
}

export type VarianceForm = {
  source: 'model' | 'report'
  win_rate: string
  avg_win_r: string
  avg_loss_r: string
  uncertain: boolean
  observed: string
  report: string
  risk_pct: string
  trades: string
  paths: string
  seed: string
  ruin: string
}

/** El pedido, o `null` mientras falte algo por escribir (los rangos los juzga el motor). */
export function toVarianceRequest(f: VarianceForm): VarianceRequest | null {
  if (!isDecimal(dec(f.risk_pct)) || !isInt(f.trades)) return null
  const optional = {
    ...(isInt(f.paths) && { paths: Number(f.paths) }),
    ...(isInt(f.seed) && { seed: Number(f.seed) }),
    ...(isDecimal(dec(f.ruin)) && { ruin_threshold_pct: dec(f.ruin) }),
  }
  const base = { risk_pct: dec(f.risk_pct), trades: Number(f.trades), ...optional }
  if (f.source === 'report') {
    return f.report ? { source: { kind: 'report', report_id: f.report }, ...base } : null
  }
  const win = pctToFraction(f.win_rate)
  if (!win || !isDecimal(dec(f.avg_win_r)) || !isDecimal(dec(f.avg_loss_r))) return null
  if (f.uncertain && !isInt(f.observed)) return null
  return {
    source: {
      kind: 'model',
      win_rate: win,
      avg_win_r: dec(f.avg_win_r),
      avg_loss_r: dec(f.avg_loss_r),
      observed_trades: f.uncertain ? Number(f.observed) : null,
    },
    ...base,
  }
}

// ── Simulador de cuentas fondeadas (#807) ─────────────────────────────────────────────────

export type PhaseForm = {
  name: string
  profit_target_pct: string
  min_trading_days: string
  max_trading_days: string
  consistency_pct: string
}

/** El formulario de reglas y operaciones: todo texto, como lo escribe la persona. */
export type PropForm = {
  base: Pick<PropRules, 'id' | 'name' | 'source'>
  account_size: string
  currency: string
  timezone: string
  cutoff: string
  phases: PhaseForm[]
  daily_loss_on: boolean
  daily_loss_pct: string
  daily_loss_base: 'account_size' | 'day_start'
  drawdown_on: boolean
  drawdown_pct: string
  drawdown_mode: DrawdownMode
  drawdown_lock: boolean
  funded_on: boolean
  payout_min_trading_days: string
  payout_min_profit_pct: string
  profit_split_pct: string
  funded_consistency_pct: string
  fee_evaluation: string
  fee_activation: string
  refund: boolean
  source: 'model' | 'report'
  win_rate: string
  avg_win_r: string
  avg_loss_r: string
  uncertain: boolean
  observed: string
  trades_per_day: string
  report: string
  risk_pct: string
  paths: string
  horizon_days: string
  max_attempts: string
  seed: string
}

const txt = (v: string | number | null | undefined) => (v == null ? '' : String(v))

/** El formulario con lo que trae la plantilla (en la genérica, todo en blanco). */
export function propFormFrom(t: PropRules): PropForm {
  return {
    base: { id: t.id, name: t.name, source: t.source },
    account_size: txt(t.account_size),
    currency: txt(t.currency),
    timezone: t.day?.timezone ?? '',
    cutoff: t.day?.cutoff ?? '',
    phases: t.phases.map((p) => ({
      name: p.name,
      profit_target_pct: txt(p.profit_target_pct),
      min_trading_days: txt(p.min_trading_days),
      max_trading_days: txt(p.max_trading_days),
      consistency_pct: txt(p.consistency_pct),
    })),
    daily_loss_on: t.daily_loss !== null,
    daily_loss_pct: txt(t.daily_loss?.limit_pct),
    daily_loss_base: t.daily_loss?.base ?? 'day_start',
    drawdown_on: t.max_drawdown !== null,
    drawdown_pct: txt(t.max_drawdown?.limit_pct),
    drawdown_mode: t.max_drawdown?.mode ?? 'static',
    drawdown_lock: t.max_drawdown?.lock_at_initial ?? false,
    funded_on: t.funded !== null,
    payout_min_trading_days: txt(t.funded?.payout_min_trading_days),
    payout_min_profit_pct: txt(t.funded?.payout_min_profit_pct),
    profit_split_pct: txt(t.funded?.profit_split_pct),
    funded_consistency_pct: txt(t.funded?.consistency_pct),
    fee_evaluation: txt(t.fees.evaluation),
    fee_activation: txt(t.fees.activation),
    refund: t.fees.refund_with_first_payout,
    source: 'model',
    win_rate: '',
    avg_win_r: '',
    avg_loss_r: '1',
    uncertain: true,
    observed: '',
    trades_per_day: '',
    report: '',
    risk_pct: '',
    paths: '',
    horizon_days: '',
    max_attempts: '',
    seed: '',
  }
}

/**
 * El pedido del contrato. Una regla en blanco va como `null` (el motor dice cuál falta, con su
 * ruta); un campo con texto que no es un número no se manda: queda en `bad` (su clave del
 * formulario). Lo de las operaciones y el riesgo, que el contrato exige, si falta va en `missing`.
 */
export function toPropSimRequest(f: PropForm): {
  req: PropSimRequest | null
  bad: string[]
  missing: string[]
} {
  const bad: string[] = []
  const decOrNull = (key: string, v: string) => {
    const s = dec(v)
    if (!s) return null
    if (!isDecimal(s)) bad.push(key)
    return s
  }
  const intOrNull = (key: string, v: string) => {
    if (!v.trim()) return null
    if (!isInt(v)) bad.push(key)
    return Number(v.trim())
  }
  const rules: PropRules = {
    ...f.base,
    account_size: decOrNull('account_size', f.account_size),
    currency: f.currency.trim() ? f.currency.trim().toUpperCase() : null,
    day:
      f.timezone.trim() || f.cutoff.trim()
        ? { timezone: f.timezone.trim(), cutoff: f.cutoff.trim() }
        : null,
    phases: f.phases.map((p, i) => ({
      name: p.name,
      profit_target_pct: decOrNull(`phases.${i}.profit_target_pct`, p.profit_target_pct),
      min_trading_days: intOrNull(`phases.${i}.min_trading_days`, p.min_trading_days),
      max_trading_days: intOrNull(`phases.${i}.max_trading_days`, p.max_trading_days),
      consistency_pct: decOrNull(`phases.${i}.consistency_pct`, p.consistency_pct),
    })),
    daily_loss: f.daily_loss_on
      ? { limit_pct: decOrNull('daily_loss_pct', f.daily_loss_pct), base: f.daily_loss_base }
      : null,
    max_drawdown: f.drawdown_on
      ? {
          limit_pct: decOrNull('drawdown_pct', f.drawdown_pct),
          mode: f.drawdown_mode,
          lock_at_initial: f.drawdown_lock,
        }
      : null,
    funded: f.funded_on
      ? {
          payout_min_trading_days: intOrNull('payout_min_trading_days', f.payout_min_trading_days),
          payout_min_profit_pct: decOrNull('payout_min_profit_pct', f.payout_min_profit_pct),
          profit_split_pct: decOrNull('profit_split_pct', f.profit_split_pct),
          consistency_pct: decOrNull('funded_consistency_pct', f.funded_consistency_pct),
        }
      : null,
    fees: {
      evaluation: decOrNull('fee_evaluation', f.fee_evaluation),
      activation: decOrNull('fee_activation', f.fee_activation),
      refund_with_first_payout: f.refund,
    },
  }
  // Lo de las operaciones lo exige el contrato: en blanco falta (`missing`), mal escrito es `bad`.
  const missing: string[] = []
  const need = (key: string, v: string, ok: (s: string) => boolean) => {
    if (!v.trim()) missing.push(key)
    else if (!ok(v)) bad.push(key)
  }
  const decimalText = (v: string) => isDecimal(dec(v))
  need('risk_pct', f.risk_pct, decimalText)
  let source: PropSimRequest['source'] | null = null
  if (f.source === 'report') {
    if (f.report) source = { kind: 'report', report_id: f.report }
    else missing.push('report')
  } else {
    need('win_rate', f.win_rate, (v) => pctToFraction(v) !== null)
    need('avg_win_r', f.avg_win_r, decimalText)
    need('avg_loss_r', f.avg_loss_r, decimalText)
    need('trades_per_day', f.trades_per_day, isInt)
    if (f.uncertain) need('observed', f.observed, isInt)
    const win = pctToFraction(f.win_rate)
    if (win)
      source = {
        kind: 'model',
        win_rate: win,
        avg_win_r: dec(f.avg_win_r),
        avg_loss_r: dec(f.avg_loss_r),
        observed_trades: f.uncertain ? Number(f.observed) : null,
        trades_per_day: Number(f.trades_per_day),
      }
  }
  const mc = {
    ...(isInt(f.paths) && { paths: Number(f.paths) }),
    ...(isInt(f.horizon_days) && { horizon_days: Number(f.horizon_days) }),
    ...(isInt(f.max_attempts) && { max_attempts: Number(f.max_attempts) }),
    ...(isInt(f.seed) && { seed: Number(f.seed) }),
  }
  if (bad.length > 0 || missing.length > 0 || !source) return { req: null, bad, missing }
  return {
    req: {
      rules,
      risk_per_trade_pct: dec(f.risk_pct),
      source,
      ...(Object.keys(mc).length > 0 && { monte_carlo: mc }),
    },
    bad,
    missing,
  }
}
