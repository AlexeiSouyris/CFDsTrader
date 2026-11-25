/**
 * Cursor sincronizado entre gráficos (#544): el gráfico que tiene el cursor avisa la hora UTC de
 * la vela de encima, y los demás marcan con una línea vertical la vela que contiene esa hora en
 * su timeframe (si no tienen ninguna, nada). Pensado para mirar varios gráficos a la vez; no
 * sincroniza el zoom ni el encuadre, y no calcula nada de trading.
 *
 * Vive fuera de React: el cursor cambia cuadro a cuadro y solo se vuelve a dibujar la línea de
 * cada gráfico, no los gráficos enteros.
 */

/** Quién movió el cursor y a qué hora (ms UTC). */
export type CursorAt = { from: string; t: number }

export class CursorSync {
  private v: CursorAt | null = null
  private subs = new Set<() => void>()

  get = () => this.v
  subscribe = (fn: () => void) => {
    this.subs.add(fn)
    return () => {
      this.subs.delete(fn)
    }
  }

  /** El gráfico `from` puso el cursor a la hora `t` (`null` = lo sacó). */
  set(from: string, t: number | null) {
    const cur = this.v
    if (t === null || !Number.isFinite(t)) {
      // solo borra el que lo puso: el que lo saca no pisa al que entra
      if (!cur || cur.from !== from) return
      this.v = null
    } else {
      if (cur && cur.from === from && cur.t === t) return
      this.v = { from, t }
    }
    this.subs.forEach((fn) => fn())
  }
}

/**
 * La vela que contiene una hora (ms UTC): su índice, o `null` si no hay ninguna (antes de la
 * primera, después de la última o en un hueco entre velas).
 */
export function barAt(
  starts: readonly number[],
  ends: readonly number[],
  t: number,
): number | null {
  const n = starts.length
  if (n === 0 || !Number.isFinite(t) || t < starts[0]) return null
  let lo = 0
  let hi = n - 1
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    if (starts[mid] <= t) lo = mid
    else hi = mid - 1
  }
  return t < ends[lo] ? lo : null
}
