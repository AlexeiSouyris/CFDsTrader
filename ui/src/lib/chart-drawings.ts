/**
 * Dibujos del usuario sobre los gráficos (#543): línea horizontal, línea de tendencia y
 * rectángulo. Son **solo visuales**: no generan señales ni órdenes y no se usan en ningún cálculo
 * de trading (CLAUDE.md §1). Por eso el precio de un punto es un `number`: es una posición en la
 * pantalla, no dinero.
 *
 * Cada punto se ancla a una hora UTC y a un precio, no a píxeles: así el dibujo sigue a las velas
 * al hacer zoom o desplazarse, y se ve en el mismo lugar con otro timeframe del mismo instrumento.
 * Se guardan en el navegador (`localStorage`) por instrumento; si no se puede leer o escribir, se
 * sigue sin guardar.
 */
import { useSyncExternalStore } from 'react'

import type { ChartMode } from '@/components/ui/chart-measure'
import { decimal } from '@/lib/format'

export type DrawKind = 'hline' | 'trend' | 'rect'
/** La herramienta de dibujo elegida: `none` = navegar (y seleccionar dibujos). */
export type DrawTool = 'none' | DrawKind

/** Las herramientas de un gráfico (barra de dibujo, #540 y #552): medición, dibujo y tipo. */
export type ChartTools = { ruler: boolean; draw: DrawTool; mode: ChartMode }
export const NO_TOOLS: ChartTools = { ruler: false, draw: 'none', mode: 'Velas' }

/** Un punto del dibujo: hora UTC (ISO 8601 con `Z`) y precio. */
export type Anchor = { t: string; p: number }
/** Un dibujo. La línea horizontal usa solo el precio de `a` (`b` es igual a `a`). */
export type Drawing = { id: string; kind: DrawKind; a: Anchor; b: Anchor }

export const DRAW_KINDS: readonly DrawKind[] = ['hline', 'trend', 'rect']
export const DRAW_LABEL: Record<DrawKind, string> = {
  hline: 'Línea horizontal',
  trend: 'Línea de tendencia',
  rect: 'Rectángulo',
}

/** El título de un dibujo, con sus precios (con los decimales del instrumento). */
export function drawingTitle(d: Pick<Drawing, 'kind' | 'a' | 'b'>, digits: number) {
  const text = (p: number) => decimal(p.toFixed(digits))
  const name = DRAW_LABEL[d.kind]
  if (d.kind === 'hline') return `${name}: ${text(d.a.p)}`
  return `${name}: de ${text(d.a.p)} a ${text(d.b.p)}`
}

const PREFIX = 'cfdstrader.drawings.'
/** Tope por instrumento: el dibujo es para marcar zonas, no para guardar miles. */
export const MAX_DRAWINGS = 200

// ---------------------------------------------------------------------------------------------
// Hora ↔ posición en el eje de índices de las velas (la vela `i` ocupa de `i` a `i + 1`).

/**
 * La hora (ms UTC) de una posición del eje de índices. Dentro de una vela se reparte entre su
 * inicio y su cierre; antes de la primera o después de la última se sigue con la duración de esa
 * vela (para poder dibujar en el lugar vacío de la derecha).
 */
export function timeAt(starts: readonly number[], ends: readonly number[], f: number): number {
  const n = starts.length
  if (n === 0 || !Number.isFinite(f)) return NaN
  const i = Math.max(0, Math.min(n - 1, Math.floor(f)))
  const dur = ends[i] - starts[i] || 1
  return starts[i] + (f - i) * dur
}

/**
 * La posición en el eje de índices de una hora (ms UTC): la inversa de `timeAt`. Una hora en un
 * hueco entre velas (noche, fin de semana) cae al inicio de la vela siguiente.
 */
export function posAt(starts: readonly number[], ends: readonly number[], t: number): number {
  const n = starts.length
  if (n === 0 || !Number.isFinite(t)) return NaN
  if (t < starts[0]) return (t - starts[0]) / (ends[0] - starts[0] || 1)
  // la última vela que empezó a esa hora o antes
  let lo = 0
  let hi = n - 1
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1
    if (starts[mid] <= t) lo = mid
    else hi = mid - 1
  }
  const dur = ends[lo] - starts[lo] || 1
  if (lo < n - 1 && t >= ends[lo]) return lo + 1
  return lo + (t - starts[lo]) / dur
}

// ---------------------------------------------------------------------------------------------
// Guardado por instrumento.

const isAnchor = (v: unknown): v is Anchor => {
  const a = v as Anchor | null
  return (
    typeof a === 'object' &&
    a !== null &&
    typeof a.t === 'string' &&
    Number.isFinite(Date.parse(a.t)) &&
    typeof a.p === 'number' &&
    Number.isFinite(a.p)
  )
}

/** Lee lo guardado y se queda solo con los dibujos bien formados. */
export function parseDrawings(raw: unknown): Drawing[] {
  if (!Array.isArray(raw)) return []
  return raw
    .filter(
      (d): d is Drawing =>
        typeof d === 'object' &&
        d !== null &&
        typeof (d as Drawing).id === 'string' &&
        DRAW_KINDS.includes((d as Drawing).kind) &&
        isAnchor((d as Drawing).a) &&
        isAnchor((d as Drawing).b),
    )
    .slice(-MAX_DRAWINGS)
}

function read(instrument: string): Drawing[] {
  try {
    return parseDrawings(JSON.parse(localStorage.getItem(PREFIX + instrument) ?? 'null'))
  } catch {
    return []
  }
}

function write(instrument: string, list: Drawing[]) {
  try {
    if (list.length) localStorage.setItem(PREFIX + instrument, JSON.stringify(list))
    else localStorage.removeItem(PREFIX + instrument)
  } catch {
    // modo privado o almacenamiento bloqueado: se ve igual, solo no se recuerda
  }
}

/**
 * Los dibujos de cada instrumento, en memoria y en el navegador. Los gráficos del mismo
 * instrumento (por ejemplo, 15 m y 1 h en el Multigráfico) comparten la lista.
 */
class DrawingStore {
  private cache = new Map<string, Drawing[]>()
  private listeners = new Set<() => void>()

  subscribe = (fn: () => void) => {
    this.listeners.add(fn)
    return () => {
      this.listeners.delete(fn)
    }
  }

  get(instrument: string): Drawing[] {
    let list = this.cache.get(instrument)
    if (!list) {
      list = read(instrument)
      this.cache.set(instrument, list)
    }
    return list
  }

  private set(instrument: string, list: Drawing[]) {
    this.cache.set(instrument, list)
    write(instrument, list)
    for (const fn of this.listeners) fn()
  }

  add(instrument: string, d: Drawing) {
    this.set(instrument, [...this.get(instrument), d].slice(-MAX_DRAWINGS))
  }

  remove(instrument: string, id: string) {
    const list = this.get(instrument)
    if (list.some((d) => d.id === id))
      this.set(
        instrument,
        list.filter((d) => d.id !== id),
      )
  }

  clear(instrument: string) {
    if (this.get(instrument).length) this.set(instrument, [])
  }

  /** Olvida lo que tiene en memoria (para los tests). */
  reset() {
    this.cache.clear()
  }
}

export const drawingStore = new DrawingStore()

/** Los dibujos de un instrumento; se vuelve a dibujar cuando cambian. */
export function useDrawings(instrument: string): Drawing[] {
  return useSyncExternalStore(
    drawingStore.subscribe,
    () => drawingStore.get(instrument),
    () => drawingStore.get(instrument),
  )
}

let seq = 0
/** Un id propio para cada dibujo (no hace falta que sea secreto, solo único). */
export const newDrawingId = () => `d${Date.now().toString(36)}${(seq++).toString(36)}`
