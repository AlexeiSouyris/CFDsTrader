/**
 * Acorta un decimal del motor a `digits` decimales para mostrarlo (mitad hacia afuera), sin pasar
 * por float: el índice llega con más de 20 decimales y desbordaba la tarjeta (#357).
 * Vive acá y no en `lib/format` porque ese archivo es compartido; se puede mover después. Si ya es
 * corto, o no es un decimal simple, vuelve tal cual. Solo es formato: el valor guardado no cambia.
 */
export function roundedText(text: string, digits = 2): string {
  const m = /^([-+]?)(\d+)(?:\.(\d+))?$/.exec(text.trim())
  if (!m || (m[3] ?? '').length <= digits) return text
  const [, sign, int, frac] = m
  const up = frac[digits] >= '5' ? 1n : 0n
  const digitsText = (BigInt(int + frac.slice(0, digits)) + up).toString().padStart(digits + 1, '0')
  const whole = digitsText.slice(0, digitsText.length - digits)
  const part = digits ? `.${digitsText.slice(-digits)}` : ''
  const zero = /^[0.]+$/.test(whole + part)
  return `${sign === '-' && !zero ? '-' : ''}${whole}${part}`
}
