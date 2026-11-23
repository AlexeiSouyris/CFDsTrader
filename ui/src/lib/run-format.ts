/**
 * Formato de pantalla de las corridas de backtest (Performance, #286 y #299). Solo presenta lo que
 * manda el motor: las fracciones pasan a % corriendo la coma en el texto (`percent`), sin cuentas.
 */
import { decimal, percent } from '@/lib/format'

const DAY_YEAR = new Intl.DateTimeFormat('es-ES', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  timeZone: 'UTC',
})
/** "01 ene 2024": el período de una corrida puede cruzar años. Acepta "2024-01-01" o ISO. */
export const dayYear = (iso: string) => DAY_YEAR.format(new Date(iso))

/** Una fracción del motor ("-0.0125") como % con signo ("−1,25 %"), sin pasar por float. */
export function signedPct(fraction: string, digits = 2) {
  const neg = fraction.startsWith('-')
  const p = percent(fraction.replace('-', ''), digits)
  const zero = /^[0,]+$/.test(p)
  return `${zero ? '' : neg ? '−' : '+'}${p} %`
}
export const pctOrDash = (f: string | null | undefined, digits = 1) =>
  f == null ? '—' : `${percent(f, digits)} %`
export const numOrDash = (f: string | null | undefined, digits = 2) =>
  f == null ? '—' : decimal(f, digits)
/** Verde o rojo por el signo del texto; nada si es cero. */
export const tone = (f: string) =>
  /^-?0(\.0*)?$/.test(f) ? undefined : f.startsWith('-') ? 'text-down' : 'text-up'
