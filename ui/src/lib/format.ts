/**
 * Formato de pantalla. Solo presenta números que ya vienen calculados: no hace cuentas de
 * trading (CLAUDE.md §1). Los montos llegan como texto decimal y se muestran tal cual.
 */

const DATE_TIME = new Intl.DateTimeFormat('es-ES', {
  day: '2-digit',
  month: 'short',
  hour: '2-digit',
  minute: '2-digit',
  timeZone: 'UTC',
})
const DATE = new Intl.DateTimeFormat('es-ES', { day: '2-digit', month: 'short', timeZone: 'UTC' })

/** "20 nov, 15:45 UTC": la hora interna es UTC (spec §2). */
export const dateTimeUtc = (iso: string) => `${DATE_TIME.format(new Date(iso))} UTC`
export const dateUtc = (iso: string) => DATE.format(new Date(iso))
const TIME = new Intl.DateTimeFormat('es-ES', {
  hour: '2-digit',
  minute: '2-digit',
  timeZone: 'UTC',
})

/**
 * Rótulo del eje de tiempo: la fecha ("20 nov") en la primera marca de cada día y la hora UTC
 * ("12:00") en las demás del mismo día. Con velas intradía, solo la fecha repetía "20 nov" (#415).
 * `prev` es la vela de la marca anterior (un paso fijo atrás), así el rótulo no salta al arrastrar.
 */
export function timeTickLabel(iso: string, prev?: string): string {
  const day = (t: string) => new Date(t).toISOString().slice(0, 10)
  return prev && day(prev) === day(iso) ? TIME.format(new Date(iso)) : dateUtc(iso)
}

/** Separa miles con punto y decimales con coma, sin pasar por float para el texto. */
export function decimal(text: string, digits?: number): string {
  const negative = text.startsWith('-')
  const [int, frac = ''] = text.replace('-', '').split('.')
  const grouped = int.replace(/\B(?=(\d{3})+(?!\d))/g, '.')
  const f = digits === undefined ? frac : frac.padEnd(digits, '0').slice(0, digits)
  return `${negative ? '−' : ''}${grouped}${f ? `,${f}` : ''}`
}

/** Con signo explícito: "+41,25" o "−18,40". */
export function signed(text: string, digits?: number): string {
  return text.startsWith('-') ? decimal(text, digits) : `+${decimal(text, digits)}`
}

export const isNegative = (text: string) => text.trim().startsWith('-')

/**
 * Una fracción de 0 a 1 (texto) como porcentaje: "0.6250" → "62,5". Corre la coma dos lugares en
 * el texto, sin pasar por float; no es un cálculo de trading.
 */
export function percent(fraction: string, digits = 1): string {
  const [int, frac = ''] = fraction.split('.')
  const f = frac.padEnd(2 + digits, '0')
  const whole = String(Number(int + f.slice(0, 2)))
  return decimal(digits ? `${whole}.${f.slice(2, 2 + digits)}` : whole)
}

/**
 * Una fracción con signo como porcentaje con signo: "-0.031" → "−3,1", "0.004" → "+0,4". Mueve la
 * coma en el texto, sin float; no es un cálculo de trading.
 */
export function signedPercent(fraction: string, digits = 1): string {
  const text = fraction.trim()
  const body = percent(text.replace(/^[-+]/, ''), digits)
  if (/^0(,0*)?$/.test(body)) return body
  return `${text.startsWith('-') ? '−' : '+'}${body}`
}
