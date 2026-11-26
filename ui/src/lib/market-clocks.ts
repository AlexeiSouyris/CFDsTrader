/**
 * Relojes de la barra de estado (#708): la hora local de cada plaza con su zona IANA. Solo
 * presentación: no dice si un mercado está abierto (los horarios de sesión son del motor).
 */

/** Las plazas de la barra, de oeste a este después de UTC. */
export const CLOCKS = [
  { label: 'UTC', zone: 'UTC', name: 'Hora UTC' },
  { label: 'NY', zone: 'America/New_York', name: 'Hora de Nueva York' },
  { label: 'LON', zone: 'Europe/London', name: 'Hora de Londres' },
  { label: 'TYO', zone: 'Asia/Tokyo', name: 'Hora de Tokio' },
] as const

const formats = new Map<string, Intl.DateTimeFormat>()
/** "16:00" (o "16:00:05" con segundos) en la zona `zone`, siempre en 24 h. */
export function clockText(at: Date, zone: string, seconds = false): string {
  const key = `${zone}|${seconds}`
  let f = formats.get(key)
  if (!f) {
    f = new Intl.DateTimeFormat('es-ES', {
      hour: '2-digit',
      minute: '2-digit',
      second: seconds ? '2-digit' : undefined,
      hourCycle: 'h23',
      timeZone: zone,
    })
    formats.set(key, f)
  }
  return f.format(at)
}
