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
const DATE_TIME_YEAR = new Intl.DateTimeFormat('es-ES', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  timeZone: 'UTC',
})
const DATE = new Intl.DateTimeFormat('es-ES', { day: '2-digit', month: 'short', timeZone: 'UTC' })
const DATE_YEAR = new Intl.DateTimeFormat('es-ES', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  timeZone: 'UTC',
})

/** Si la fecha no es del año en curso (UTC), se muestra el año: "21 nov 2024" no pasa por hoy (#577). */
const otherYear = (d: Date, now: number) => d.getUTCFullYear() !== new Date(now).getUTCFullYear()

/** "20 nov, 15:45 UTC" (con el año si no es el de hoy): la hora interna es UTC (spec §2). */
export const dateTimeUtc = (iso: string, now = Date.now()) => {
  const d = new Date(iso)
  return `${(otherYear(d, now) ? DATE_TIME_YEAR : DATE_TIME).format(d)} UTC`
}
/** "20 nov" (con el año si no es el de hoy). */
export const dateUtc = (iso: string, now = Date.now()) => {
  const d = new Date(iso)
  return (otherYear(d, now) ? DATE_YEAR : DATE).format(d)
}

/**
 * Desde cuántos días un dato es viejo y se avisa "datos de hace N días" (#577). Tres días cubren un
 * fin de semana normal (el cierre del viernes visto el lunes no es viejo); más que eso, el dato ya
 * no es de esta semana y se dice.
 */
export const STALE_AFTER_DAYS = 3
const DAY_MS = 86_400_000

/** Los días enteros desde `iso` si pasan `STALE_AFTER_DAYS`; si no (o si la fecha no sirve), `null`. */
export function staleDays(iso: string | null | undefined, now = Date.now()): number | null {
  const t = iso ? Date.parse(iso) : NaN
  if (Number.isNaN(t)) return null
  const days = Math.floor((now - t) / DAY_MS)
  return days >= STALE_AFTER_DAYS ? days : null
}

/**
 * La hora de una vela cerrada, siempre la de su **cierre** (`end`) y dicha así: el gráfico y el
 * panel mostraban una el inicio y otro el cierre (#580). "cerró el 25 nov, 04:00 UTC" (con el año si no es el de hoy, #577).
 */
export const closedAt = (endIso: string, now = Date.now()) => `cerró el ${dateTimeUtc(endIso, now)}`
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

/** Con signo explícito: "+41,25" o "−18,40". El cero va sin signo ("0", "0,00"). */
export function signed(text: string, digits?: number): string {
  const body = decimal(text.replace(/^[-+]/, ''), digits)
  if (/^0(,0*)?$/.test(body)) return body
  return text.startsWith('-') ? `−${body}` : `+${body}`
}

/**
 * Cuántos decimales tiene el instrumento, según los precios que llegan del motor (el texto
 * decimal de cada uno): el máximo de las muestras, hasta 5. Un cierre "26006" junto a
 * "26007.75" dice que el instrumento usa 2 decimales. Es formato, no un cálculo.
 */
export function priceDigits(texts: readonly string[]): number {
  let d = 0
  for (const t of texts) d = Math.max(d, t.split('.')[1]?.length ?? 0)
  return Math.min(d, 5)
}

/**
 * El formateador de precios de la app: siempre con los decimales del instrumento, así "26.006"
 * no se lee como 26 con decimales al lado de "26.007,75". Corta el texto sin pasar por float.
 */
export const price = (text: string, digits: number) => decimal(text, digits)

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
