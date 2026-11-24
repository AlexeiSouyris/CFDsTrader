/**
 * Herramienta de medición de los gráficos (#281): se aprieta el clic (o el dedo) en el panel de
 * precio y se arrastra. Se dibuja un rectángulo entre los dos puntos, verde si el precio final es
 * mayor y rojo si es menor, con la diferencia en precio, en %, en velas y en tiempo.
 *
 * Punto que sigue al gráfico (#355): al pasar el cursor (o el dedo) por el panel de precio, un
 * círculo chico se ubica en la vela de esa x, a la altura de su **cierre** (arriba del cuerpo en
 * una vela alcista, abajo en una bajista; sobre la línea en el gráfico de línea), sin importar la
 * altura del cursor. Los dos puntos de la medición también se pegan al cierre de su vela.
 *
 * Movimiento fluido (#377): el punto, la línea vertical y el extremo de la medición no saltan de
 * vela en vela: se deslizan hacia su destino con `requestAnimationFrame` (un acercamiento
 * exponencial, sin rebote). Con `prefers-reduced-motion` van directo. El estado del cursor vive
 * en un almacén chico fuera de React (`PointerStore`): mover el mouse **no vuelve a dibujar el
 * gráfico**, solo la capa del punto y la cabecera que lo leen. En modo línea el punto recorre la
 * línea de forma continua (entre cierre y cierre); con velas se pega al cierre.
 *
 * **Es una medición visual que hace el usuario, no lógica de trading** (CLAUDE.md §1): no genera
 * señales, no toca el motor ni los contratos y no se guarda en ningún lado. El precio de cada punto
 * es el cierre de la vela tal como llega, pasado a `Number` como ya se hace para ubicar cosas en
 * pantalla; las cuentas son solo para el texto de la etiqueta.
 *
 * Uso: al soltar, la medición queda visible hasta un clic o `Esc`. Un segundo arrastre la
 * reemplaza. Funciona con mouse y con el dedo (pointer events); por eso el SVG lleva
 * `touch-action: none` y la página se desplaza tocando fuera del gráfico.
 */
import {
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type PointerEvent,
  type ReactNode,
  type RefObject,
} from 'react'

import { decimal } from '@/lib/format'

/** Cómo se dibuja el precio: velas o línea de cierre con área (#377). */
export const CHART_MODES = ['Velas', 'Línea'] as const
export type ChartMode = (typeof CHART_MODES)[number]

/** Cómo está dibujado el panel de precio, en coordenadas del SVG. */
export type MeasurePlot = {
  /** ancho y alto del SVG (su `viewBox`), para pasar del cursor a coordenadas del dibujo */
  width: number
  height: number
  /** borde derecho del área de velas (las velas arrancan en x = 0) */
  plotW: number
  /** borde superior e inferior del panel de precio */
  top: number
  bottom: number
  /** ancho de cada vela */
  slot: number
  /** inicio (ISO, UTC) de cada vela visible, en orden */
  starts: string[]
  /** cierre de cada vela visible (mismo orden que `starts`): ahí se pegan el punto y la medición */
  closes: number[]
  /** altura de un precio: la escala del eje Y del gráfico */
  yOf: (price: number) => number
  /** decimales con que se muestran los precios del instrumento */
  digits: number
  /** velas o línea (por defecto, velas) */
  mode?: ChartMode
  /** el punto también sigue al cursor fuera del panel de precio (gráficos con más paneles) */
  anyY?: boolean
  /**
   * Guía propia del gráfico (la cruz de lectura de `trading-chart`), en vez de la línea vertical
   * tenue. Recibe la posición animada del punto y la vela bajo el cursor.
   */
  guide?: (p: { x: number; y: number; i: number }) => ReactNode
}

/** Un punto en datos (vela y precio), así sobrevive a un cambio de ancho o a una vela nueva. */
type Point = { start: string; price: number }
type Measure = { from: Point; to: Point; dragging: boolean }
/** Posición en datos: índice de vela (puede ser fraccionario en modo línea) y precio. */
type Spot = { idx: number; price: number }

export type PointerState = {
  /** vela más cercana al cursor (la que se lee en la cabecera); `null` fuera del gráfico */
  hover: number | null
  /** dónde va el punto y dónde está dibujado ahora (se acerca a `dotGoal` cuadro a cuadro) */
  dotGoal: Spot | null
  dot: Spot | null
  measure: Measure | null
  /** extremo final de la medición: destino y posición dibujada */
  endGoal: Spot | null
  end: Spot | null
}

const CLICK_PX = 3
/** constante de tiempo del acercamiento, en ms: a los ~5 τ el punto ya llegó */
export const GLIDE_TAU_MS = 55
const GLIDE_DONE_MS = GLIDE_TAU_MS * 5

export const prefersReducedMotion = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true

const EMPTY: PointerState = {
  hover: null,
  dotGoal: null,
  dot: null,
  measure: null,
  endGoal: null,
  end: null,
}

/** Un paso del acercamiento exponencial; `k` es la fracción del camino que se recorre. */
function approach(cur: Spot | null, goal: Spot | null, k: number): Spot | null {
  if (!cur || !goal) return goal
  return {
    idx: cur.idx + (goal.idx - cur.idx) * k,
    price: cur.price + (goal.price - cur.price) * k,
  }
}

/**
 * Estado del cursor fuera de React: los manejadores del puntero lo escriben y solo los
 * componentes chicos que lo leen (`useSyncExternalStore`) se vuelven a dibujar. La animación corre
 * acá, con `requestAnimationFrame`, y se detiene sola al llegar.
 */
export class PointerStore {
  private s: PointerState = EMPTY
  private subs = new Set<() => void>()
  private frame = 0
  private last = 0
  private goalAt = 0

  get = () => this.s
  subscribe = (f: () => void) => {
    this.subs.add(f)
    return () => {
      this.subs.delete(f)
      if (this.subs.size === 0) this.stop()
    }
  }

  private emit(next: PointerState) {
    this.s = next
    this.subs.forEach((f) => f())
  }

  /** Cambia el estado. El punto y el extremo de la medición se deslizan, salvo que aparezcan. */
  set(patch: Partial<Pick<PointerState, 'hover' | 'dotGoal' | 'measure' | 'endGoal'>>) {
    const smooth = !prefersReducedMotion() && typeof requestAnimationFrame === 'function'
    const next: PointerState = { ...this.s, ...patch }
    // al aparecer (o sin animación) va directo a su lugar; si no, arranca desde donde está
    next.dot = smooth && next.dotGoal && this.s.dot ? this.s.dot : next.dotGoal
    next.end = smooth && next.endGoal && this.s.end ? this.s.end : next.endGoal
    const moving =
      (next.dot !== null && next.dot !== next.dotGoal) ||
      (next.end !== null && next.end !== next.endGoal)
    this.emit(next)
    if (moving) {
      this.goalAt = performance.now()
      this.start()
    }
  }

  private start() {
    if (this.frame) return
    this.last = performance.now()
    this.frame = requestAnimationFrame(this.tick)
  }
  private stop() {
    if (this.frame) cancelAnimationFrame(this.frame)
    this.frame = 0
  }
  private tick = (now: number) => {
    this.frame = 0
    const dt = Math.max(0, Math.min(64, now - this.last))
    this.last = now
    const done = now - this.goalAt >= GLIDE_DONE_MS
    const k = done ? 1 : 1 - Math.exp(-dt / GLIDE_TAU_MS)
    const s = this.s
    const dot = done ? s.dotGoal : approach(s.dot, s.dotGoal, k)
    const end = done ? s.endGoal : approach(s.end, s.endGoal, k)
    this.emit({ ...s, dot, end })
    if (!done) this.frame = requestAnimationFrame(this.tick)
  }
}

/** Lee una parte del estado del cursor; el componente se dibuja solo cuando esa parte cambia. */
export function usePointer<T>(store: PointerStore, select: (s: PointerState) => T): T {
  return useSyncExternalStore(
    store.subscribe,
    () => select(store.get()),
    () => select(EMPTY),
  )
}

/** distancia mínima entre rótulos del eje (y entre un rótulo y un precio del eje), en px */
export const LABEL_GAP = 12

/**
 * Ubica los rótulos del eje sin que se pisen y sin salir del panel `[minY, maxY]`: los ordena,
 * los empuja hacia abajo para separarlos y después, desde abajo, los sube si se pasaron del borde.
 * Si no entran todos, los que sobran quedan pegados arriba (no salen nunca del panel).
 * Es solo geometría de dibujo.
 */
export function placeLabels<T extends { y: number }>(items: T[], minY: number, maxY: number): T[] {
  const out = items
    .map((t) => ({ ...t, y: Math.max(minY, Math.min(maxY, t.y)) }))
    .sort((a, b) => a.y - b.y)
  for (let k = 1; k < out.length; k++) out[k].y = Math.max(out[k].y, out[k - 1].y + LABEL_GAP)
  for (let k = out.length - 1; k >= 0; k--) {
    const limit = k === out.length - 1 ? maxY : out[k + 1].y - LABEL_GAP
    out[k].y = Math.max(minY, Math.min(out[k].y, limit))
  }
  return out
}

/** Número con signo y `d` decimales para la etiqueta: "+12,50", "−0,08" o "0,00". */
export function signedFixed(n: number, d: number): string {
  const t = n.toFixed(d)
  if (Number(t) === 0) return decimal(t.replace('-', ''))
  return t.startsWith('-') ? decimal(t) : `+${decimal(t)}`
}

/** Un lapso en ms como texto: "2 d 3 h", "1 h 15 min", "45 min" o "0 min". */
export function span(ms: number): string {
  const total = Math.round(Math.abs(ms) / 60_000)
  const d = Math.floor(total / 1440)
  const h = Math.floor((total % 1440) / 60)
  const m = total % 60
  const parts = [d && `${d} d`, h && `${h} h`, m && `${m} min`].filter(Boolean)
  return parts.length ? parts.join(' ') : '0 min'
}

/**
 * Lo que muestra la etiqueta. Los precios se redondean a los decimales del instrumento antes de
 * comparar, así el color y los textos dicen lo mismo.
 */
export function measureText(from: number, to: number, digits: number) {
  const a = Number(from.toFixed(digits))
  const b = Number(to.toFixed(digits))
  const trend: 'up' | 'down' | 'flat' = b > a ? 'up' : b < a ? 'down' : 'flat'
  const pct = a === 0 ? '—' : `${signedFixed(((b - a) / a) * 100, 2)} %`
  return {
    trend,
    from: a.toFixed(digits),
    to: b.toFixed(digits),
    price: signedFixed(b - a, digits),
    pct,
  }
}

export type Trend = ReturnType<typeof measureText>['trend']

/**
 * El cambio para la cabecera (#377): "+312,28 ▲ 4,49 %" o "−482,25 ▼ 6,42 %". Solo texto de
 * pantalla, con las mismas cuentas que la medición.
 */
export function changeText(from: number, to: number, digits: number) {
  const t = measureText(from, to, digits)
  const arrow = t.trend === 'up' ? '▲' : t.trend === 'down' ? '▼' : '·'
  return { ...t, text: `${t.price} ${arrow} ${t.pct.replace(/^[+−]/, '')}` }
}

/** Color de cada signo, con los tokens del tema (claro y oscuro). */
export const TREND_COLOR = {
  up: 'var(--chart-up)',
  down: 'var(--chart-down)',
  flat: 'var(--muted-foreground)',
} as const

/** Signo del rango visible: último cierre contra el primero (para el color del modo línea). */
export function rangeTrend(closes: number[], digits: number): Trend {
  const a = closes.find(Number.isFinite)
  const b = closes.findLast(Number.isFinite)
  return a === undefined || b === undefined ? 'flat' : measureText(a, b, digits).trend
}

/** Precio sobre la línea en un índice fraccionario (entre dos cierres). */
export function closeAt(closes: number[], f: number): number {
  const i0 = Math.max(0, Math.min(closes.length - 1, Math.floor(f)))
  const i1 = Math.min(closes.length - 1, i0 + 1)
  const a = closes[i0]
  const b = closes[i1]
  if (!Number.isFinite(a)) return b
  if (!Number.isFinite(b)) return a
  return a + (b - a) * Math.max(0, Math.min(1, f - i0))
}

/** Puntos de la línea de cierre entre dos índices (fraccionarios), para resaltar un tramo. */
export function segment(
  closes: number[],
  a: number,
  b: number,
  x: (f: number) => number,
  yOf: (p: number) => number,
) {
  const lo = Math.min(a, b)
  const hi = Math.max(a, b)
  const idx = [lo]
  for (let i = Math.ceil(lo); i <= Math.floor(hi); i++) if (i > lo && i < hi) idx.push(i)
  idx.push(hi)
  return idx
    .map((f) => [x(f), yOf(closeAt(closes, f))])
    .filter(([, y]) => Number.isFinite(y))
    .map(([px, py]) => `${px.toFixed(1)},${py.toFixed(1)}`)
}

export function useChartMeasure(svgRef: RefObject<SVGSVGElement | null>, plot: MeasurePlot) {
  const [store] = useState(() => new PointerStore())
  const downAt = useRef<{ x: number; y: number } | null>(null)
  const { width, height, plotW, top, bottom, slot, starts, closes } = plot
  const line = plot.mode === 'Línea'
  const n = starts.length

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && store.get().measure) store.set({ measure: null, endGoal: null })
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [store])

  /** Del cursor a una posición continua en velas; `null` fuera del panel (si `clamp` es falso). */
  const toPos = (e: PointerEvent<SVGSVGElement>, clamp: boolean): number | null => {
    const r = svgRef.current?.getBoundingClientRect()
    if (!r || !r.width || !r.height || n === 0) return null
    const x = ((e.clientX - r.left) / r.width) * width
    const y = ((e.clientY - r.top) / r.height) * height
    const outY = plot.anyY ? y < 0 || y > height : y < top || y > bottom
    if (!clamp && (x < 0 || x > plotW || outY)) return null
    const cx = Math.max(0, Math.min(plotW - 0.01, x))
    return Math.max(0, Math.min(n - 1, cx / slot - 0.5))
  }
  /** Adónde va el punto: sobre la línea (continuo) o al cierre de la vela. */
  const spotOf = (pos: number): Spot => {
    const i = Math.round(pos)
    return line ? { idx: pos, price: closeAt(closes, pos) } : { idx: i, price: closes[i] }
  }
  const hoverAt = (pos: number | null) => ({
    hover: pos === null ? null : Math.round(pos),
    dotGoal: pos === null ? null : spotOf(pos),
  })
  const pointOf = (pos: number | null): Point | null => {
    if (pos === null) return null
    const i = Math.round(pos)
    return Number.isFinite(closes[i]) ? { start: starts[i], price: closes[i] } : null
  }
  const endOf = (p: Point): Spot => ({ idx: starts.indexOf(p.start), price: p.price })

  const onPointerDown = (e: PointerEvent<SVGSVGElement>) => {
    const pos = toPos(e, false)
    if (e.button !== 0) {
      store.set(hoverAt(pos))
      return
    }
    const p = pointOf(pos)
    downAt.current = { x: e.clientX, y: e.clientY }
    if (!p) {
      // un clic fuera del panel de precio (eje, fechas) también cierra la medición
      store.set({ ...hoverAt(pos), measure: null, endGoal: null })
      return
    }
    try {
      e.currentTarget.setPointerCapture?.(e.pointerId)
    } catch {
      // sin captura el arrastre igual funciona dentro del gráfico
    }
    store.set({ ...hoverAt(pos), measure: { from: p, to: p, dragging: true }, endGoal: endOf(p) })
  }
  const onPointerMove = (e: PointerEvent<SVGSVGElement>) => {
    const measure = store.get().measure
    // mientras se arrastra el punto sigue al cursor aunque salga del panel
    const pos = toPos(e, measure?.dragging === true)
    if (!measure?.dragging) {
      store.set(hoverAt(pos))
      return
    }
    const p = pointOf(toPos(e, true))
    if (!p) return store.set(hoverAt(pos))
    store.set({ ...hoverAt(pos), measure: { ...measure, to: p }, endGoal: endOf(p) })
  }
  const onPointerUp = (e: PointerEvent<SVGSVGElement>) => {
    const d = downAt.current
    downAt.current = null
    const measure = store.get().measure
    if (!measure?.dragging || !d) return
    const moved = Math.abs(e.clientX - d.x) > CLICK_PX || Math.abs(e.clientY - d.y) > CLICK_PX
    // sin arrastre es un clic: cierra la medición que hubiera
    store.set(
      moved ? { measure: { ...measure, dragging: false } } : { measure: null, endGoal: null },
    )
  }
  const onPointerCancel = () => {
    downAt.current = null
    const dragging = store.get().measure?.dragging
    store.set({ ...hoverAt(null), ...(dragging ? { measure: null, endGoal: null } : {}) })
  }
  const onPointerLeave = () => {
    if (!store.get().measure?.dragging) store.set(hoverAt(null))
  }

  /** Mueve la lectura con el teclado (flechas): `delta` velas desde la actual o la última. */
  const step = (delta: number) => {
    if (n === 0) return
    const i = Math.max(0, Math.min(n - 1, (store.get().hover ?? n - 1) + delta))
    store.set(hoverAt(i))
  }
  const clear = () => store.set(hoverAt(null))

  return {
    store,
    step,
    clear,
    handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel, onPointerLeave },
    /** lo que necesita `ChartFollower` para dibujar la capa que se mueve */
    plot,
  }
}
