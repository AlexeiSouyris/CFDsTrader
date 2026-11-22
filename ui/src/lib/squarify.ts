/** Treemap "squarified" para el mapa de mercado: geometría de dibujo, no lógica de trading. */
import type { HeatmapCompany } from '@/mocks'

export type Tile = { x: number; y: number; w: number; h: number; co: HeatmapCompany }

function worst(row: number[], side: number, area: number) {
  const s2 = area * area
  const l2 = side * side
  return Math.max((l2 * Math.max(...row)) / s2, s2 / (l2 * Math.min(...row)))
}

/** Treemap "squarified": filas a lo largo del lado corto, con cuadros lo más cuadrados posible. */
export function squarify(cos: HeatmapCompany[], w: number, h: number): Tile[] {
  const total = cos.reduce((s, c) => s + Number(c.cap), 0) || 1
  const items = cos.map((co) => ({ area: (Number(co.cap) / total) * w * h, co }))
  const out: Tile[] = []
  let rx = 0
  let ry = 0
  let rw = w
  let rh = h
  let i = 0
  while (i < items.length) {
    const side = Math.min(rw, rh)
    const row = [items[i]]
    let area = items[i].area
    let k = i + 1
    while (k < items.length) {
      const next = [...row.map((r) => r.area), items[k].area]
      if (
        worst(next, side, area + items[k].area) >
        worst(
          row.map((r) => r.area),
          side,
          area,
        )
      )
        break
      row.push(items[k])
      area += items[k].area
      k++
    }
    if (rw >= rh) {
      const colW = area / rh
      let cy = ry
      for (const r of row) {
        out.push({ x: rx, y: cy, w: colW, h: r.area / colW, co: r.co })
        cy += r.area / colW
      }
      rx += colW
      rw -= colW
    } else {
      const rowH = area / rw
      let cx = rx
      for (const r of row) {
        out.push({ x: cx, y: ry, w: r.area / rowH, h: rowH, co: r.co })
        cx += r.area / rowH
      }
      ry += rowH
      rh -= rowH
    }
    i = k
  }
  return out
}
