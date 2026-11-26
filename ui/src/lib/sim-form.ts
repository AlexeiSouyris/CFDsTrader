/**
 * Formularios de los simuladores (#806): del texto que escribe la persona al pedido del contrato.
 * Solo forma y formato (decimales del contrato, porcentaje → fracción moviendo la coma): los rangos
 * y todo cálculo los juzga el motor (CLAUDE.md §1).
 */
import type { VarianceRequest } from '@/lib/sim-contracts'

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
