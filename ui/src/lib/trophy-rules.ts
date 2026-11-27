/**
 * Los trofeos de proceso previstos (#581, #616): premian la disciplina y las reglas cumplidas,
 * **nunca la ganancia**. Es el catálogo de reglas, no datos: el estado de cada trofeo lo va a
 * calcular el motor con las operaciones del runner de paper, y la UI solo lo va a mostrar. Hoy el
 * motor no los calcula (no hay operaciones en simulación todavía).
 */
export type TrophyRule = {
  id: 'stop' | 'diario' | 'limite' | 'propuestas' | 'calendario'
  title: string
  rule: string
  period: 'día' | 'semana' | 'mes'
}

export const TROPHY_RULES: readonly TrophyRule[] = [
  {
    id: 'stop',
    title: 'Stop respetado',
    rule: 'Ninguna operación de la semana movió ni quitó su stop-loss.',
    period: 'semana',
  },
  {
    id: 'diario',
    title: 'Diario completo',
    rule: 'Cada operación cerrada tiene su nota en el diario antes del cierre del día.',
    period: 'semana',
  },
  {
    id: 'limite',
    title: 'Dentro del límite',
    rule: 'Ningún día llegó a la pérdida máxima diaria configurada.',
    period: 'mes',
  },
  {
    id: 'propuestas',
    title: 'Propuestas a tiempo',
    rule: 'Todas las propuestas se aprobaron o rechazaron antes de vencer.',
    period: 'día',
  },
  {
    id: 'calendario',
    title: 'Calendario revisado',
    rule: 'Sin operaciones abiertas en los 15 minutos antes de un evento de impacto alto.',
    period: 'semana',
  },
]

/** La regla con ese id (el catálogo es fijo: un id que no está es un error del código). */
export function trophyRule(id: TrophyRule['id']): TrophyRule {
  const rule = TROPHY_RULES.find((r) => r.id === id)
  if (!rule) throw new Error(`trofeo desconocido: ${id}`)
  return rule
}
