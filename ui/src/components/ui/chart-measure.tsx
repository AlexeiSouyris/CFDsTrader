/**
 * Herramienta de medición de los gráficos (#281): se aprieta el clic (o el dedo) en el panel de
 * precio y se arrastra. Se dibuja un rectángulo entre los dos puntos, verde si el precio final es
 * mayor y rojo si es menor, con la diferencia en precio, en %, en velas y en tiempo.
 *
 * **Es una medición visual que hace el usuario, no lógica de trading** (CLAUDE.md §1): no genera
 * señales, no toca el motor ni los contratos y no se guarda en ningún lado. El precio de cada punto
 * sale de la escala del eje Y del gráfico (la inversa de su `yP`), con `Number`, como ya se hace
 * para ubicar cosas en pantalla; las cuentas son solo para el texto de la etiqueta.
 *
 * Uso: al soltar, la medición queda visible hasta un clic o `Esc`. Un segundo arrastre la
 * reemplaza. Funciona con mouse y con el dedo (pointer events); por eso el SVG lleva
 * `touch-action: none` y la página se desplaza tocando fuera del gráfico.
 */
import { useEffect, useRef, useState, type PointerEvent, type RefObject } from 'react'

import { decimal } from '@/lib/format'

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
  /** precio a una altura y: la inversa de la escala del eje Y del gráfico */
  priceAt: (y: number) => number
  /** altura de un precio: la escala del eje Y del gráfico */
  yOf: (price: number) => number
  /** decimales con que se muestran los precios del instrumento */
  digits: number
}

/** Un punto en datos (vela y precio), así sobrevive a un cambio de ancho o a una vela nueva. */
type Point = { start: string; price: number }
type Measure = { from: Point; to: Point; dragging: boolean }

const CLICK_PX = 3

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

const TREND_COLOR = {
  up: 'var(--chart-up)',
  down: 'var(--chart-down)',
  flat: 'var(--muted-foreground)',
} as const

export function useChartMeasure(svgRef: RefObject<SVGSVGElement | null>, plot: MeasurePlot) {
  const [measure, setMeasure] = useState<Measure | null>(null)
  const downAt = useRef<{ x: number; y: number } | null>(null)
  const { width, height, plotW, top, bottom, slot, starts, priceAt, yOf, digits } = plot

  const open = measure !== null
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setMeasure(null)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open])

  /** Del cursor a un punto en datos; `null` si no hay dónde ubicarlo. */
  const toPoint = (e: PointerEvent<SVGSVGElement>, clamp: boolean): Point | null => {
    const r = svgRef.current?.getBoundingClientRect()
    if (!r || !r.width || !r.height || starts.length === 0) return null
    let x = ((e.clientX - r.left) / r.width) * width
    let y = ((e.clientY - r.top) / r.height) * height
    if (!clamp && (x < 0 || x > plotW || y < top || y > bottom)) return null
    x = Math.max(0, Math.min(plotW - 0.01, x))
    y = Math.max(top, Math.min(bottom, y))
    const i = Math.max(0, Math.min(starts.length - 1, Math.floor(x / slot)))
    return { start: starts[i], price: priceAt(y) }
  }

  const onPointerDown = (e: PointerEvent<SVGSVGElement>) => {
    if (e.button !== 0) return
    const p = toPoint(e, false)
    downAt.current = { x: e.clientX, y: e.clientY }
    if (!p) {
      // un clic fuera del panel de precio (eje, fechas) también cierra la medición
      setMeasure(null)
      return
    }
    try {
      e.currentTarget.setPointerCapture?.(e.pointerId)
    } catch {
      // sin captura el arrastre igual funciona dentro del gráfico
    }
    setMeasure({ from: p, to: p, dragging: true })
  }
  const onPointerMove = (e: PointerEvent<SVGSVGElement>) => {
    if (!measure?.dragging) return
    const p = toPoint(e, true)
    if (p) setMeasure({ ...measure, to: p })
  }
  const onPointerUp = (e: PointerEvent<SVGSVGElement>) => {
    const d = downAt.current
    downAt.current = null
    if (!measure?.dragging || !d) return
    const moved = Math.abs(e.clientX - d.x) > CLICK_PX || Math.abs(e.clientY - d.y) > CLICK_PX
    // sin arrastre es un clic: cierra la medición que hubiera
    setMeasure(moved ? { ...measure, dragging: false } : null)
  }
  const onPointerCancel = () => {
    downAt.current = null
    if (measure?.dragging) setMeasure(null)
  }

  let overlay = null
  const ia = measure ? starts.indexOf(measure.from.start) : -1
  const ib = measure ? starts.indexOf(measure.to.start) : -1
  // si una de las velas ya no está a la vista (cambió el rango), la medición no se dibuja
  if (measure && ia >= 0 && ib >= 0) {
    const t = measureText(measure.from.price, measure.to.price, digits)
    const color = TREND_COLOR[t.trend]
    const bars = Math.abs(ib - ia)
    const lines = [
      `${t.price} (${t.pct})`,
      `${bars} ${bars === 1 ? 'vela' : 'velas'} · ${span(Date.parse(starts[ib]) - Date.parse(starts[ia]))}`,
    ]
    const x0 = Math.min(ia, ib) * slot
    const x1 = (Math.max(ia, ib) + 1) * slot
    const ya = yOf(measure.from.price)
    const yb = yOf(measure.to.price)
    // etiqueta junto al cursor (el punto final), del lado donde entra
    const cx = ib * slot + slot / 2
    const w = Math.max(...lines.map((l) => l.length)) * 6.4 + 16
    const h = 36
    const lx = cx + 10 + w > plotW ? Math.max(0, cx - 10 - w) : cx + 10
    const ly = yb - h - 8 < top ? Math.min(bottom - h, yb + 8) : yb - h - 8
    overlay = (
      <g data-measure={t.trend} data-from={t.from} data-to={t.to} pointerEvents="none" aria-hidden>
        <rect
          data-measure-box
          x={x0}
          y={Math.min(ya, yb)}
          width={Math.max(1, x1 - x0)}
          height={Math.max(1, Math.abs(yb - ya))}
          fill={color}
          fillOpacity={0.16}
          stroke={color}
          strokeOpacity={0.7}
        />
        <line x1={x0} x2={x1} y1={ya} y2={ya} stroke={color} strokeDasharray="3 3" />
        <g transform={`translate(${lx},${ly})`}>
          <rect
            width={w}
            height={h}
            rx={5}
            fill="var(--background)"
            fillOpacity={0.92}
            stroke={color}
          />
          {lines.map((l, k) => (
            <text
              key={k}
              data-measure-line={k}
              x={8}
              y={15 + k * 14}
              fontSize={11}
              fontWeight={k === 0 ? 700 : 500}
              fill={k === 0 ? color : 'var(--foreground)'}
            >
              {l}
            </text>
          ))}
        </g>
      </g>
    )
  }

  return {
    handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel },
    overlay,
  }
}
