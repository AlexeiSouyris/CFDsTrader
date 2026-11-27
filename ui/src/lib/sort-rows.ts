/**
 * Ordenar tablas (#581, etapa 6): presentación, no cálculo. Las filas se reordenan tal como
 * llegan del motor; no se crea ni se transforma ningún valor (CLAUDE.md §1). Los decimales del
 * motor llegan como texto y se comparan exactos, dígito por dígito, sin pasar por `float`.
 */
import { useState } from 'react'

/** Lo que se compara de una fila: texto, decimal en texto, número entero o nada (va al final). */
export type SortValue = string | number | null | undefined
export type SortDir = 'asc' | 'desc'
export type Sort<K extends string> = { key: K; dir: SortDir } | null

const DECIMAL = /^[+-]?\d+(\.\d+)?$/

/** Compara dos decimales en texto sin redondear: `-0.5` < `0` < `0.05` < `12`. */
export function compareDecimal(a: string, b: string): number {
  const parse = (s: string) => {
    const neg = s.startsWith('-')
    const [int = '', frac = ''] = s.replace(/^[+-]/, '').split('.')
    const i = int.replace(/^0+/, '')
    const f = frac.replace(/0+$/, '')
    return { neg: neg && (i !== '' || f !== ''), i, f }
  }
  const x = parse(a)
  const y = parse(b)
  if (x.neg !== y.neg) return x.neg ? -1 : 1
  const sign = x.neg ? -1 : 1
  if (x.i.length !== y.i.length) return sign * (x.i.length - y.i.length)
  if (x.i !== y.i) return sign * (x.i < y.i ? -1 : 1)
  const n = Math.max(x.f.length, y.f.length)
  const fx = x.f.padEnd(n, '0')
  const fy = y.f.padEnd(n, '0')
  return fx === fy ? 0 : sign * (fx < fy ? -1 : 1)
}

/** Orden ascendente de dos valores; los vacíos (`null`) no se comparan acá. */
function compare(a: string | number, b: string | number): number {
  if (typeof a === 'number' && typeof b === 'number') return a - b
  const sa = String(a)
  const sb = String(b)
  if (DECIMAL.test(sa) && DECIMAL.test(sb)) return compareDecimal(sa, sb)
  return sa.localeCompare(sb, 'es', { numeric: true, sensitivity: 'base' })
}

/**
 * Las filas en el orden pedido (sin `sort`, tal como llegaron). Estable: las filas iguales quedan
 * en su orden original. Los valores vacíos van siempre al final, en las dos direcciones.
 */
export function sortRows<T>(rows: readonly T[], value: (row: T) => SortValue, dir: SortDir): T[] {
  const sign = dir === 'asc' ? 1 : -1
  return rows
    .map((row, i) => ({ row, i, v: value(row) }))
    .sort((a, b) => {
      const an = a.v === null || a.v === undefined || a.v === ''
      const bn = b.v === null || b.v === undefined || b.v === ''
      if (an || bn) return an === bn ? a.i - b.i : an ? 1 : -1
      return sign * compare(a.v!, b.v!) || a.i - b.i
    })
    .map((x) => x.row)
}

/**
 * El orden de una tabla: tocar una columna la ordena (los números, de mayor a menor primero; el
 * texto, de la A a la Z), tocarla de nuevo invierte y la tercera vez vuelve al orden de llegada.
 */
export function useSort<K extends string>(numeric: readonly K[] = []) {
  const [sort, setSort] = useState<Sort<K>>(null)
  const toggle = (key: K) =>
    setSort((cur) => {
      const first: SortDir = numeric.includes(key) ? 'desc' : 'asc'
      if (cur?.key !== key) return { key, dir: first }
      if (cur.dir === first) return { key, dir: first === 'asc' ? 'desc' : 'asc' }
      return null
    })
  return [sort, toggle] as const
}
