/**
 * Navegación de los gráficos (#401): arrastrar desplaza en el tiempo (con inercia), la rueda
 * acerca o aleja anclada en el cursor, el gesto de pellizcar (trackpad o dos dedos) hace zoom y el
 * desplazamiento horizontal del trackpad desplaza. Sobre el eje de precio, arrastrar en vertical
 * estira o comprime la escala (modo manual) y el doble clic la vuelve automática. Con el foco en
 * el gráfico: ← → desplazan, + − acercan y alejan, Fin va a la última vela, Inicio a la primera.
 * La medición (#281) pasa a Shift + arrastrar o al botón de regla.
 *
 * Con el dedo (#424): el gráfico deja el desplazamiento vertical a la página (`touch-action:
 * pan-y`), así no atrapa el scroll. Un dedo en horizontal mueve el gráfico, dos dedos hacen zoom,
 * mantener apretado y arrastrar mide, y sobre el eje de precio el arrastre vertical lo estira.
 * Para esos tres gestos se cancela el `touchmove` (la página no se mueve mientras tanto).
 *
 * Rendimiento: el encuadre vive en `ViewportStore` (fuera de React). Las velas se dibujan una vez
 * para un tramo (lo visible más una pantalla a cada lado) y, mientras se arrastra o se hace zoom,
 * se mueven con una transformación del SVG (`matrix`) puesta a mano, sin volver a dibujarlas.
 * Solo se vuelven a dibujar al salir de ese tramo, al cambiar mucho el zoom o cuando el gesto
 * termina. Los ejes (unos pocos textos) y la capa del cursor sí siguen cada cuadro.
 *
 * Es solo presentación (CLAUDE.md §1): ningún cálculo de trading, solo dónde se dibuja cada cosa.
 */
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type RefObject,
} from 'react'

import type { FrameSource, Frame, useChartMeasure } from '@/components/ui/chart-measure'
import {
  affine,
  atLatest,
  mapping,
  nearStart,
  needsRedraw,
  panBy,
  priceOf,
  rangeView,
  shiftPrice,
  stretchPrice,
  ViewportStore,
  zoomAt,
  type Mapping,
  type PriceRange,
  type View,
  type ViewportState,
} from '@/lib/chart-viewport'

/** Un gesto que se mueve menos que esto (px) es un clic. */
const CLICK_PX = 3
/** Después de este tiempo quieto (ms) se vuelven a dibujar las velas con su escala exacta. */
export const SETTLE_MS = 150
/** Sensibilidad de la rueda (zoom) y del pellizco del trackpad (`ctrl` + rueda). */
const WHEEL_ZOOM = 0.002
const PINCH_ZOOM = 0.01
/** cuánto se ve la pista de zoom con la rueda (#580) */
const WHEEL_HINT_MS = 1800
/** Sensibilidad del arrastre vertical sobre el eje de precio. */
const AXIS_STRETCH = 0.006
/** Mantener apretado el dedo este tiempo (ms) sin moverlo empieza a medir. */
export const LONG_PRESS_MS = 450
/** Cuánto (px de pantalla) puede temblar el dedo durante esa espera. */
const LONG_PRESS_SLOP = 8

export type HistoryState = 'idle' | 'loading' | 'done' | 'error'

/**
 * El encuadre de un gráfico: el almacén vivo, el encuadre con que se dibujaron las velas
 * (`base`), las capas que se transforman y la escala vigente para los ejes.
 */
export function useViewport(opts: {
  starts: readonly string[]
  lows: readonly number[]
  highs: readonly number[]
  /** velas visibles al abrir (`Infinity` = todas) */
  initial: number
  plotW: number
  /** borde superior y alto del panel de precio */
  top: number
  h: number
  /** se llama al acercarse al borde izquierdo por un gesto del usuario */
  onNearStart?: () => void
}) {
  const { starts, lows, highs, plotW, top, h } = opts
  const n = starts.length
  const [store] = useState(() => new ViewportStore(rangeView(opts.initial, n), n))
  const [link] = useState(() => new ViewportLink(store))
  const [base, setBase] = useState<ViewportState>(() => store.get())
  const baseMap = useMemo(
    () => mapping(base, priceOf(base, lows, highs), plotW, top, h),
    [base, lows, highs, plotW, top, h],
  )

  useLayoutEffect(() => {
    link.sync({ lows, highs, plotW, top, h }, base, baseMap, opts.onNearStart)
  })

  // Velas nuevas: si se sumaron viejas al principio, el encuadre se corre para seguir mostrando
  // lo mismo; si llegó una vela nueva y se estaba viendo la última, la sigue.
  useLayoutEffect(() => link.data(starts), [link, starts])

  useEffect(() => link.listen(setBase), [link])

  const frame: FrameSource = useMemo(
    () => ({ subscribe: store.subscribe, get: () => link.geo().f }),
    [store, link],
  )

  return {
    store,
    link,
    n,
    base,
    baseMap,
    frame,
    /** refs (función) de las capas que se transforman: precio (x e y) y paneles (solo x) */
    priceLayer: link.setPriceLayer,
    xLayer: link.setXLayer,
    /** Pone el encuadre de un rango (las últimas `bars` velas), sin animación. */
    setRange: (bars: number) => store.view(rangeView(bars, store.count()), false),
    /** Va a la última vela con el mismo zoom, deslizándose. */
    goLatest: () => {
      const s = store.get()
      const c = store.count()
      store.glideTo({ start: c - (s.end - s.start), end: c })
    },
  }
}

type Data = {
  lows: readonly number[]
  highs: readonly number[]
  plotW: number
  top: number
  h: number
}

/**
 * Une el almacén con el dibujo, fuera de React: la escala vigente (una por estado), la
 * transformación de las capas y cuándo hay que volver a dibujar las velas.
 */
export class ViewportLink {
  private d: Data = { lows: [], highs: [], plotW: 1, top: 0, h: 1 }
  private cache: { s: ViewportState; d: Data; m: Mapping; f: Frame } | null = null
  private base: ViewportState
  private baseMap: Mapping | null = null
  private price: SVGGElement | null = null
  private x: SVGGElement | null = null
  private force = false
  private onNear: (() => void) | undefined
  private prev: { first: string | undefined; n: number }
  private commit: ((s: ViewportState) => void) | null = null

  private store: ViewportStore

  constructor(store: ViewportStore) {
    this.store = store
    this.base = store.get()
    this.prev = { first: undefined, n: store.count() }
  }

  setPriceLayer = (el: SVGGElement | null) => {
    this.price = el
  }
  setXLayer = (el: SVGGElement | null) => {
    this.x = el
  }

  /** Lo último que se dibujó (después de cada dibujo del gráfico). */
  sync(d: Data, base: ViewportState, baseMap: Mapping, onNear?: () => void) {
    this.d = d
    this.base = base
    this.baseMap = baseMap
    this.onNear = onNear
    this.apply()
  }

  /** La escala vigente y el encuadre para la capa del cursor (mismo objeto en un mismo estado). */
  geo = () => {
    const s = this.store.get()
    const d = this.d
    const c = this.cache
    if (c && c.s === s && c.d === d) return c
    const m = mapping(s, priceOf(s, d.lows, d.highs), d.plotW, d.top, d.h)
    const next = { s, d, m, f: { start: m.start, slot: m.slot, yOf: m.y } }
    this.cache = next
    return next
  }

  /** Pone a mano la transformación que lleva lo dibujado al encuadre vivo. */
  apply() {
    if (!this.baseMap) return
    const { sx, tx, sy, ty } = affine(this.baseMap, this.geo().m)
    const id = Math.abs(sx - 1) < 1e-9 && Math.abs(tx) < 1e-6
    const idY = id && Math.abs(sy - 1) < 1e-9 && Math.abs(ty) < 1e-6
    const set = (el: SVGGElement | null, t: string | null) => {
      if (!el) return
      if (t === null) el.removeAttribute('transform')
      else el.setAttribute('transform', t)
    }
    set(this.price, idY ? null : `matrix(${sx} 0 0 ${sy} ${tx} ${ty})`)
    set(this.x, id ? null : `matrix(${sx} 0 0 1 ${tx} 0)`)
  }

  /** Cambiaron las velas: corre el encuadre si se sumaron viejas o si sigue a la última. */
  data(starts: readonly string[]) {
    const p = this.prev
    const n = starts.length
    if (p.first === starts[0] && p.n === n) return
    this.prev = { first: starts[0], n }
    if (p.first === undefined && p.n === n) return
    const k = p.first === undefined ? 0 : Math.max(0, starts.indexOf(p.first))
    const appended = Math.max(0, n - k - p.n)
    const follow = p.n > 0 && atLatest(this.store.get(), p.n)
    this.force = true
    this.store.setCount(n, k + (follow ? appended : 0))
  }

  /** Escucha el almacén; `commit` vuelve a dibujar las velas con el encuadre que se le pasa. */
  listen(commit: (s: ViewportState) => void) {
    this.commit = commit
    let timer = 0
    const unsub = this.store.subscribe(() => {
      const s = this.store.get()
      this.apply()
      if (s.user && nearStart(s)) this.onNear?.()
      window.clearTimeout(timer)
      if (this.force || !s.user || needsRedraw(this.base, s, this.store.count())) {
        this.force = false
        commit(s)
        return
      }
      // al quedar quieto, se vuelve a dibujar con la escala exacta (trazos y textos nítidos)
      timer = window.setTimeout(() => {
        if (this.store.get() !== this.base) commit(this.store.get())
      }, SETTLE_MS)
    })
    return () => {
      window.clearTimeout(timer)
      this.commit = null
      unsub()
    }
  }

  /** Vuelve a dibujar ya (gestos de a un paso, como las teclas), sin esperar a que quede quieto. */
  flush() {
    const s = this.store.get()
    if (s !== this.base) this.commit?.(s)
  }
}

export type Viewport = ReturnType<typeof useViewport>

/** La escala vigente; el componente que la usa se vuelve a dibujar en cada cuadro del gesto. */
export function useMapping(vp: Viewport): Mapping {
  return useSyncExternalStore(
    vp.store.subscribe,
    () => vp.link.geo().m,
    () => vp.link.geo().m,
  )
}

/** El rango de los botones que coincide con el encuadre dibujado, o ninguno. */
export function matchRange(
  base: View,
  n: number,
  ranges: readonly { label: string; bars: number }[],
): string {
  const hit = ranges.find((r) => {
    const v = rangeView(r.bars, n)
    return Math.abs(v.start - base.start) < 0.5 && Math.abs(v.end - base.end) < 0.5
  })
  return hit?.label ?? ''
}

type Measure = ReturnType<typeof useChartMeasure>

type Drag = {
  mode: 'pan' | 'axis' | 'measure'
  id: number
  cx: number
  cy: number
  view: View
  price: PriceRange
  manual: PriceRange | null
  moved: boolean
  samples: { t: number; x: number }[]
}

/**
 * Los gestos del gráfico. Devuelve los manejadores del SVG (puntero, doble clic, teclas); la
 * rueda se escucha aparte porque necesita `preventDefault` (no deja desplazar la página mientras
 * el cursor está sobre el gráfico).
 */
export function useChartNav(
  svgRef: RefObject<SVGSVGElement | null>,
  opts: {
    vp: Viewport
    width: number
    height: number
    plotW: number
    /** panel de precio: ahí el eje de la derecha se estira */
    top: number
    bottom: number
    measure: Measure
    /** botón de regla: el arrastre mide en vez de desplazar */
    ruler: boolean
  },
) {
  const { vp, measure } = opts
  const o = useRef(opts)
  useLayoutEffect(() => {
    o.current = opts
  })
  const drag = useRef<Drag | null>(null)
  const pointers = useRef(new Map<number, { x: number; y: number }>())
  const pinch = useRef<{ d0: number; view: View } | null>(null)
  /** dedo apoyado esperando el toque largo que empieza a medir */
  const press = useRef<{ timer: number } | null>(null)
  const endPress = () => {
    if (press.current) window.clearTimeout(press.current.timer)
    press.current = null
  }

  /** De la pantalla al SVG: posición y cuántas unidades del SVG mide un px. */
  const toSvg = (clientX: number, clientY: number) => {
    const { width, height } = o.current
    const r = svgRef.current?.getBoundingClientRect()
    if (!r || !r.width || !r.height) return null
    return {
      x: ((clientX - r.left) / r.width) * width,
      y: ((clientY - r.top) / r.height) * height,
      k: width / r.width,
    }
  }

  /**
   * La rueda vertical es de la página (#580): hace zoom solo con Ctrl o ⌘, con el pellizco del
   * trackpad (que llega con Ctrl) o después de un clic en el gráfico, hasta que el puntero sale.
   * Si no, deja scrollear y avisa cómo hacer zoom.
   */
  const engaged = useRef(false)
  const [wheelHint, setWheelHint] = useState(false)
  const hintTimer = useRef(0)
  const showHint = () => {
    window.clearTimeout(hintTimer.current)
    setWheelHint(true)
    hintTimer.current = window.setTimeout(() => setWheelHint(false), WHEEL_HINT_MS)
  }
  useEffect(() => () => window.clearTimeout(hintTimer.current), [])

  // La rueda (y el trackpad): zoom anclado al cursor, desplazamiento horizontal y pellizco.
  useEffect(() => {
    const svg = svgRef.current
    if (!svg) return
    const onWheel = (e: WheelEvent) => {
      const p = toSvg(e.clientX, e.clientY)
      if (!p) return
      const { plotW, top, bottom } = o.current
      const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1
      const dx = e.deltaX * unit
      const dy = e.deltaY * unit
      const horizontal = Math.abs(dx) > Math.abs(dy) || (e.shiftKey && dx === 0)
      if (!horizontal && !e.ctrlKey && !e.metaKey && !engaged.current) return showHint()
      e.preventDefault()
      const store = vp.store
      const s = store.get()
      const span = s.end - s.start
      const anchor = Math.max(0, Math.min(1, p.x / plotW))
      if (e.ctrlKey || e.metaKey) return store.zoom(Math.exp(dy * PINCH_ZOOM), anchor)
      if (e.shiftKey && dx === 0) return store.pan((dy * p.k * span) / plotW)
      if (Math.abs(dx) > Math.abs(dy)) return store.pan((dx * p.k * span) / plotW)
      if (p.x > plotW && p.y >= top && p.y <= bottom) {
        // sobre el eje de precio, la rueda estira o comprime la escala
        const price = priceOf(s, vp.link.geo().d.lows, vp.link.geo().d.highs)
        return store.set({ manual: stretchPrice(price, Math.exp(dy * WHEEL_ZOOM)), user: true })
      }
      store.zoom(Math.exp(dy * WHEEL_ZOOM), anchor)
    }
    svg.addEventListener('wheel', onWheel, { passive: false })
    return () => svg.removeEventListener('wheel', onWheel)
    // toSvg y opts se leen de `o`; el SVG y el almacén no cambian
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [svgRef, vp.store])

  // El dedo: el vertical es de la página (pan-y), salvo al medir, estirar el eje o pellizcar.
  useEffect(() => {
    const svg = svgRef.current
    if (!svg) return
    const onTouchMove = (e: TouchEvent) => {
      const mode = drag.current?.mode
      if (e.touches.length > 1 || pinch.current || mode === 'measure' || mode === 'axis')
        if (e.cancelable) e.preventDefault()
    }
    svg.addEventListener('touchmove', onTouchMove, { passive: false })
    return () => {
      svg.removeEventListener('touchmove', onTouchMove)
      endPress()
    }
  }, [svgRef])

  const capture = (e: PointerEvent<SVGSVGElement>) => {
    try {
      e.currentTarget.setPointerCapture?.(e.pointerId)
    } catch {
      // sin captura el gesto igual funciona dentro del gráfico
    }
  }

  const onPointerDown = (e: PointerEvent<SVGSVGElement>) => {
    const { plotW, height, top, bottom, ruler } = o.current
    engaged.current = true
    if (wheelHint) setWheelHint(false)
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY })
    // dos dedos: pellizco (zoom)
    if (pointers.current.size === 2 && e.pointerType === 'touch') {
      const [a, b] = [...pointers.current.values()]
      endPress()
      if (drag.current?.mode === 'measure') measure.handlers.onPointerCancel()
      drag.current = null
      pinch.current = { d0: Math.hypot(a.x - b.x, a.y - b.y) || 1, view: vp.store.get() }
      measure.clear()
      capture(e)
      return
    }
    if (e.button !== 0) return measure.handlers.onPointerDown(e)
    const p = toSvg(e.clientX, e.clientY)
    if (!p) return
    const s = vp.store.get()
    const base = {
      id: e.pointerId,
      cx: e.clientX,
      cy: e.clientY,
      view: { start: s.start, end: s.end },
      price: priceOf(s, vp.link.geo().d.lows, vp.link.geo().d.highs),
      manual: s.manual,
      moved: false,
      samples: [],
    }
    if (e.shiftKey || ruler) {
      drag.current = { ...base, mode: 'measure' }
      return measure.handlers.onPointerDown(e)
    }
    vp.store.stop()
    if (p.x > plotW && p.y >= top && p.y <= bottom) {
      drag.current = { ...base, mode: 'axis' }
    } else if (p.x >= 0 && p.x <= plotW && p.y >= 0 && p.y <= height) {
      drag.current = { ...base, mode: 'pan' }
      measure.clear()
      if (e.pointerType === 'touch') startPress(e)
    } else return
    capture(e)
  }

  /** Toque largo: si el dedo queda quieto, el arrastre pasa a medir desde donde se apoyó. */
  const startPress = (e: PointerEvent<SVGSVGElement>) => {
    endPress()
    const svg = e.currentTarget
    const down = { clientX: e.clientX, clientY: e.clientY, pointerId: e.pointerId, button: 0 }
    const timer = window.setTimeout(() => {
      const d = drag.current
      const waiting = press.current !== null
      press.current = null
      if (!waiting || !d || d.id !== down.pointerId || d.mode !== 'pan') return
      // lo poco que se movió mientras esperaba se deshace: medir no desplaza
      vp.store.set({ ...d.view, manual: d.manual, user: true })
      drag.current = { ...d, mode: 'measure' }
      measure.handlers.onPointerDown({
        ...down,
        currentTarget: svg,
      } as unknown as PointerEvent<SVGSVGElement>)
    }, LONG_PRESS_MS)
    press.current = { timer }
  }

  const onPointerMove = (e: PointerEvent<SVGSVGElement>) => {
    if (pointers.current.has(e.pointerId))
      pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY })
    const pz = pinch.current
    if (pz && pointers.current.size >= 2) {
      const [a, b] = [...pointers.current.values()]
      const d = Math.hypot(a.x - b.x, a.y - b.y) || 1
      const mid = toSvg((a.x + b.x) / 2, (a.y + b.y) / 2)
      const anchor = mid ? Math.max(0, Math.min(1, mid.x / o.current.plotW)) : 0.5
      vp.store.view(zoomAt(pz.view, pz.d0 / d, anchor, vp.store.count()))
      return
    }
    const d = drag.current
    if (!d || d.id !== e.pointerId || d.mode === 'measure') return measure.handlers.onPointerMove(e)
    if (press.current && Math.hypot(e.clientX - d.cx, e.clientY - d.cy) > LONG_PRESS_SLOP)
      endPress()
    const p = toSvg(e.clientX, e.clientY)
    if (!p) return
    const dx = (e.clientX - d.cx) * p.k
    const dy = (e.clientY - d.cy) * p.k
    if (Math.abs(dx) > CLICK_PX || Math.abs(dy) > CLICK_PX) d.moved = true
    const { plotW, top, bottom } = o.current
    if (d.mode === 'axis') {
      // hacia abajo comprime (entra más precio), hacia arriba estira
      vp.store.set({ manual: stretchPrice(d.price, Math.exp(dy * AXIS_STRETCH)), user: true })
      return
    }
    const span = d.view.end - d.view.start
    const now = performance.now()
    d.samples.push({ t: now, x: e.clientX * p.k })
    while (d.samples.length > 2 && now - d.samples[0].t > 100) d.samples.shift()
    const v = panBy(d.view, (-dx * span) / plotW, vp.store.count())
    // con la escala manual, el arrastre vertical también mueve el precio
    const manual = d.manual
      ? shiftPrice(d.manual, (dy * (d.manual.hi - d.manual.lo)) / (bottom - top || 1))
      : null
    vp.store.set({ ...v, manual, user: true })
  }

  const onPointerUp = (e: PointerEvent<SVGSVGElement>) => {
    pointers.current.delete(e.pointerId)
    endPress()
    if (pinch.current) {
      if (pointers.current.size < 2) pinch.current = null
      return
    }
    const d = drag.current
    if (!d || d.id !== e.pointerId || d.mode === 'measure') {
      if (d?.id === e.pointerId) drag.current = null
      return measure.handlers.onPointerUp(e)
    }
    drag.current = null
    if (d.mode !== 'pan') return
    // un clic sin arrastre borra la medición, como antes
    if (!d.moved) return measure.dismiss()
    const sm = d.samples
    const now = performance.now()
    if (sm.length < 2 || now - sm[sm.length - 1].t > 60) return
    const dt = sm[sm.length - 1].t - sm[0].t
    if (dt <= 0) return
    const span = d.view.end - d.view.start
    const pxPerMs = (sm[sm.length - 1].x - sm[0].x) / dt
    vp.store.fling((-pxPerMs * span) / o.current.plotW)
  }

  const onPointerCancel = (e: PointerEvent<SVGSVGElement>) => {
    pointers.current.delete(e.pointerId)
    endPress()
    pinch.current = null
    drag.current = null
    measure.handlers.onPointerCancel()
  }

  const onPointerLeave = () => {
    if (!drag.current) measure.handlers.onPointerLeave()
    // al salir del gráfico, la rueda vuelve a ser de la página
    if (!drag.current) engaged.current = false
  }

  const onDoubleClick = (e: MouseEvent<SVGSVGElement>) => {
    const p = toSvg(e.clientX, e.clientY)
    if (p && p.x > o.current.plotW) vp.store.set({ manual: null, user: true })
  }

  const onKeyDown = (e: KeyboardEvent<SVGSVGElement>) => {
    const store = vp.store
    const s = store.get()
    const span = s.end - s.start
    const n = store.count()
    const arrow = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
    let handled = true
    if (arrow && e.shiftKey) measure.step(arrow)
    else if (arrow) store.pan(arrow * Math.max(1, span * 0.15))
    else if (e.key === '+' || e.key === '=') store.zoom(0.8, atLatest(s, n) ? 1 : 0.5)
    else if (e.key === '-' || e.key === '_') store.zoom(1.25, atLatest(s, n) ? 1 : 0.5)
    else if (e.key === 'End') store.view({ start: n - span, end: n })
    else if (e.key === 'Home') store.view({ start: 0, end: span })
    else if (e.key === 'Escape') measure.clear()
    else handled = false
    if (handled) {
      e.preventDefault()
      vp.link.flush()
    }
  }

  return {
    handlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onPointerCancel,
      onPointerLeave,
      onDoubleClick,
    },
    onKeyDown,
    /** la rueda pasó sin zoom: mostrar la pista (`ChartNavBar` `wheelHint`) */
    wheelHint,
  }
}
