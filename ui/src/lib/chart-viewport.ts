/**
 * Encuadre de los gráficos navegables (#401): qué velas se ven (un tramo continuo de índices,
 * fraccionario) y qué precios abarca el eje Y (automático o manual).
 *
 * Todo esto es **geometría de pantalla**, no lógica de trading (CLAUDE.md §1): no cambia ningún
 * dato, solo dónde se dibuja. Las funciones son puras y el estado vivo está en `ViewportStore`,
 * fuera de React: arrastrar, hacer zoom o la inercia lo cambian cuadro a cuadro sin volver a
 * dibujar las velas (las mueve una transformación del SVG; ver `chart-nav.tsx`).
 *
 * Convención: la vela `i` ocupa de `i` a `i + 1` en el eje de índices y su centro está en
 * `i + 0.5`. El encuadre `{ start, end }` es la parte visible de ese eje: `end − start` velas.
 */

/** Tramo visible del eje de índices. */
export type View = { start: number; end: number }
/** Precios que abarca el eje Y, de abajo (`lo`) a arriba (`hi`). */
export type PriceRange = { lo: number; hi: number }

export type ViewportState = View & {
  /** escala de precio manual (arrastre sobre el eje); `null` = se ajusta sola a lo visible */
  manual: PriceRange | null
  /** el último cambio lo hizo el usuario (arrastre, rueda, teclas), no un botón de rango */
  user: boolean
}

/** Mínimo de velas a la vista (zoom máximo). */
export const MIN_SPAN = 8
/** Lugar vacío que se permite a la derecha de la última vela, en fracción de lo visible. */
export const RIGHT_ROOM = 0.25
/** Cerca del borde izquierdo (en fracción de lo visible) se piden velas más viejas. */
export const LOAD_EDGE = 0.25
/** Margen de precio sobre el máximo y bajo el mínimo visibles en modo automático. */
export const PRICE_PAD = 0.08
/** Constante de tiempo de la inercia del arrastre, en ms (frenado exponencial). */
export const FLING_TAU_MS = 280
/** Constante de tiempo del deslizamiento al ir a un encuadre (botón, teclas), en ms. */
export const GLIDE_TAU_MS = 70

const span = (v: View) => v.end - v.start

/** Lleva un encuadre a los límites: entre `MIN_SPAN` y todas las velas, sin pasar el inicio. */
export function clampView(v: View, n: number): View {
  if (n <= 0) return { start: 0, end: 1 }
  const lo = Math.min(MIN_SPAN, n)
  const hi = Math.max(MIN_SPAN, n)
  const s = Math.max(lo, Math.min(hi, Number.isFinite(span(v)) ? span(v) : hi))
  const maxStart = Math.max(0, n + s * RIGHT_ROOM - s)
  const start = Math.max(0, Math.min(maxStart, Number.isFinite(v.start) ? v.start : n - s))
  return { start, end: start + s }
}

/**
 * Zoom anclado: el punto que está en la fracción `anchor` del ancho (0 = borde izquierdo,
 * 1 = derecho) queda en el mismo lugar. `factor` > 1 aleja (más velas), < 1 acerca.
 */
export function zoomAt(v: View, factor: number, anchor: number, n: number): View {
  const a = Math.max(0, Math.min(1, anchor))
  const lo = Math.min(MIN_SPAN, Math.max(n, 1))
  const hi = Math.max(MIN_SPAN, n)
  const s = Math.max(lo, Math.min(hi, span(v) * factor))
  const at = v.start + a * span(v)
  return clampView({ start: at - a * s, end: at - a * s + s }, n)
}

/** Desplaza `bars` velas (negativo = hacia el pasado). */
export function panBy(v: View, bars: number, n: number): View {
  return clampView({ start: v.start + bars, end: v.end + bars }, n)
}

/** Las últimas `bars` velas (`Infinity` = todas), con la última pegada al borde derecho. */
export function rangeView(bars: number, n: number): View {
  const s = Math.min(bars, n)
  return clampView({ start: n - s, end: n }, n)
}

/** Mismo zoom, con la última vela pegada al borde derecho. */
export function latestView(v: View, n: number): View {
  return clampView({ start: n - span(v), end: n }, n)
}

/** ¿La última vela está a la vista, en el borde derecho o casi? */
export const atLatest = (v: View, n: number) => v.end >= n - 0.5

/** Índices de las velas que se ven aunque sea en parte: `[a, b)`. */
export function visibleRange(v: View, n: number): [number, number] {
  return [Math.max(0, Math.floor(v.start)), Math.min(n, Math.ceil(v.end))]
}

/**
 * Las velas que se dibujan: lo visible más `margin` pantallas a cada lado, así el arrastre corto
 * no necesita dibujar nada nuevo.
 */
export function renderWindow(v: View, n: number, margin = 1): [number, number] {
  const s = span(v) * margin
  return [Math.max(0, Math.floor(v.start - s)), Math.min(n, Math.ceil(v.end + s))]
}

/** ¿Hay que volver a dibujar? Si lo visible salió de lo dibujado o el zoom cambió mucho. */
export function needsRedraw(base: View, cur: View, n: number): boolean {
  const [a, b] = renderWindow(base, n)
  const [va, vb] = visibleRange(cur, n)
  const ratio = span(cur) / span(base)
  return va < a || vb > b || ratio < 0.6 || ratio > 1.6
}

/** ¿Está cerca del borde izquierdo (hay que pedir velas más viejas)? */
export const nearStart = (v: View) => v.start <= span(v) * LOAD_EDGE

/** Mínimo y máximo de lo visible, con margen (`PRICE_PAD`); `null` si no hay precios. */
export function autoPrice(
  lows: readonly number[],
  highs: readonly number[],
  v: View,
): PriceRange | null {
  const [a, b] = visibleRange(v, lows.length)
  let lo = Infinity
  let hi = -Infinity
  for (let i = a; i < b; i++) {
    if (Number.isFinite(lows[i]) && lows[i] < lo) lo = lows[i]
    if (Number.isFinite(highs[i]) && highs[i] > hi) hi = highs[i]
  }
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return null
  const pad = (hi - lo) * PRICE_PAD || Math.abs(hi) * 0.001 || 1
  return { lo: lo - pad, hi: hi + pad }
}

/** Estira (`factor` > 1) o comprime la escala de precio alrededor de su centro. */
export function stretchPrice(r: PriceRange, factor: number): PriceRange {
  const c = (r.lo + r.hi) / 2
  const half = Math.max(((r.hi - r.lo) / 2) * factor, Math.abs(c) * 1e-6 || 1e-9)
  return { lo: c - half, hi: c + half }
}

/** Mueve la escala de precio `d` (positivo = sube). */
export const shiftPrice = (r: PriceRange, d: number): PriceRange => ({ lo: r.lo + d, hi: r.hi + d })

/** Escala de precio vigente: la manual o, si no hay, la automática de lo visible. */
export function priceOf(
  s: ViewportState,
  lows: readonly number[],
  highs: readonly number[],
): PriceRange {
  return s.manual ?? autoPrice(lows, highs, s) ?? { lo: 0, hi: 1 }
}

/** Cómo se pasa de datos (índice, precio) a coordenadas del SVG en un panel. */
export type Mapping = View & {
  span: number
  /** ancho de una vela */
  slot: number
  plotW: number
  lo: number
  hi: number
  /** borde superior y alto del panel de precio */
  top: number
  h: number
  /** x del centro de la vela (o índice fraccionario) `f` */
  x: (f: number) => number
  /** y de un precio */
  y: (p: number) => number
}

export function mapping(
  v: View,
  price: PriceRange,
  plotW: number,
  top: number,
  h: number,
): Mapping {
  const s = span(v)
  const slot = plotW / s
  const range = price.hi - price.lo || 1
  return {
    start: v.start,
    end: v.start + s,
    span: s,
    slot,
    plotW,
    lo: price.lo,
    hi: price.hi,
    top,
    h,
    x: (f) => (f + 0.5 - v.start) * slot,
    y: (p) => top + (1 - (p - price.lo) / range) * h,
  }
}

/**
 * La transformación que lleva lo dibujado con `base` a como se ve con `cur`:
 * `x' = sx·x + tx` y `y' = sy·y + ty`. Con ella las velas se mueven sin volver a dibujarse.
 */
export function affine(base: Mapping, cur: Mapping) {
  const sx = cur.slot / base.slot
  const tx = (base.start - cur.start) * cur.slot
  const sy = (base.hi - base.lo) / (cur.hi - cur.lo || 1)
  const bottom = base.top + base.h
  const ty = bottom * (1 - sy) - ((base.lo - cur.lo) * cur.h) / (cur.hi - cur.lo || 1)
  return { sx, tx, sy, ty }
}

const NICE = [1, 2, 2.5, 5, 10]

/** Precios redondos para el eje (1, 2, 2,5 o 5 × 10ⁿ) y los decimales con que se muestran. */
export function niceTicks(lo: number, hi: number, count = 5): { ticks: number[]; digits: number } {
  const range = hi - lo
  if (!(range > 0) || !Number.isFinite(range)) return { ticks: [], digits: 0 }
  const raw = range / count
  let pow = 10 ** Math.floor(Math.log10(raw))
  let m = NICE.find((k) => k * pow >= raw * (1 - 1e-9)) ?? 10
  if (m === 10) [pow, m] = [pow * 10, 1]
  const step = m * pow
  const digits = Math.max(0, -Math.round(Math.log10(pow)) + (m === 2.5 ? 1 : 0))
  const ticks: number[] = []
  for (let t = Math.ceil(lo / step) * step; t <= hi + step * 1e-9; t += step)
    ticks.push(Number(t.toFixed(Math.min(20, digits + 2))))
  return { ticks, digits }
}

const TIME_STEPS = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768]

/**
 * Velas que llevan fecha en el eje: múltiplos de un paso fijo, así cada rótulo queda pegado a
 * su vela mientras se arrastra (no salta). Unas `count` a la vista.
 */
export function timeTicks(
  v: View,
  n: number,
  count = 5,
  /** tramo de índices donde buscarlas (por defecto, lo visible) */
  within?: [number, number],
): number[] {
  const want = span(v) / count
  const step = TIME_STEPS.find((s) => s >= want) ?? Math.ceil(want)
  const [a, b] = within ?? visibleRange(v, n)
  const out: number[] = []
  for (let i = Math.ceil(a / step) * step; i < b; i += step) out.push(i)
  return out
}

/**
 * El encuadre vivo: lo cambian el arrastre, la rueda, las teclas y los botones, y lo leen la
 * transformación de las velas y las capas chicas (ejes, cursor), nunca el gráfico entero.
 * La inercia y los deslizamientos corren acá con `requestAnimationFrame` y paran solos.
 */
export class ViewportStore {
  private s: ViewportState
  private n: number
  private subs = new Set<() => void>()
  private frame = 0
  private last = 0
  private anim: { kind: 'fling'; v: number } | { kind: 'glide'; goal: View; at: number } | null =
    null

  constructor(view: View, n: number) {
    this.n = n
    this.s = { ...clampView(view, n), manual: null, user: false }
  }

  get = () => this.s
  count = () => this.n
  subscribe = (f: () => void) => {
    this.subs.add(f)
    return () => {
      this.subs.delete(f)
      if (this.subs.size === 0) this.stop()
    }
  }

  private emit(next: ViewportState) {
    if (
      next.start === this.s.start &&
      next.end === this.s.end &&
      next.manual === this.s.manual &&
      next.user === this.s.user
    )
      return
    this.s = next
    this.subs.forEach((f) => f())
  }

  /** Cambia el encuadre (lo lleva a los límites). Corta la inercia o el deslizamiento. */
  set(patch: Partial<ViewportState>, opts: { keepAnim?: boolean } = {}) {
    if (!opts.keepAnim) this.stop()
    const next = { ...this.s, ...patch }
    const v = clampView(next, this.n)
    this.emit({ ...next, ...v })
  }

  view = (v: View, user = true) => this.set({ ...v, user })
  pan = (bars: number) => this.view(panBy(this.s, bars, this.n))
  zoom = (factor: number, anchor: number) => this.view(zoomAt(this.s, factor, anchor, this.n))

  /** Hay más (o menos) velas. `shift` corre el encuadre: al sumar velas viejas al principio. */
  setCount(n: number, shift = 0) {
    this.n = n
    if (this.anim?.kind === 'glide')
      this.anim.goal = { start: this.anim.goal.start + shift, end: this.anim.goal.end + shift }
    const v = clampView({ start: this.s.start + shift, end: this.s.end + shift }, n)
    this.emit({ ...this.s, ...v })
  }

  private reduced() {
    return (
      typeof requestAnimationFrame !== 'function' ||
      (typeof window !== 'undefined' &&
        window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true)
    )
  }

  /** Inercia después de soltar el arrastre: `v` en velas por ms; frena sola. */
  fling(v: number) {
    this.stop()
    if (this.reduced() || !Number.isFinite(v) || Math.abs(v) < 0.002) return
    this.anim = { kind: 'fling', v }
    this.start()
  }

  /** Va a un encuadre deslizándose (o directo con `prefers-reduced-motion`). */
  glideTo(goal: View, user = true) {
    const g = clampView(goal, this.n)
    if (this.reduced()) return this.view(g, user)
    this.stop()
    this.anim = { kind: 'glide', goal: g, at: performance.now() }
    this.s = { ...this.s, user }
    this.start()
  }

  stop() {
    if (this.frame) cancelAnimationFrame(this.frame)
    this.frame = 0
    this.anim = null
  }

  private start() {
    this.last = performance.now()
    this.frame = requestAnimationFrame(this.tick)
  }

  private tick = (now: number) => {
    this.frame = 0
    const a = this.anim
    if (!a) return
    const dt = Math.max(0, Math.min(64, now - this.last))
    this.last = now
    if (a.kind === 'fling') {
      const before = this.s.start
      this.set({ ...panBy(this.s, a.v * dt, this.n), user: true }, { keepAnim: true })
      a.v *= Math.exp(-dt / FLING_TAU_MS)
      // para al frenar o al chocar con un borde
      if (Math.abs(a.v) * 16 < 0.01 || this.s.start === before) return void (this.anim = null)
    } else {
      const done = now - a.at >= GLIDE_TAU_MS * 6
      const k = done ? 1 : 1 - Math.exp(-dt / GLIDE_TAU_MS)
      const s = this.s
      this.set(
        {
          start: s.start + (a.goal.start - s.start) * k,
          end: s.end + (a.goal.end - s.end) * k,
        },
        { keepAnim: true },
      )
      if (done) return void (this.anim = null)
    }
    this.frame = requestAnimationFrame(this.tick)
  }
}
