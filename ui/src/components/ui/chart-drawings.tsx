/**
 * Capa de dibujos del usuario (#543, ver `lib/chart-drawings.ts`): línea horizontal, línea de
 * tendencia y rectángulo. Solo dibujo: no son señales ni órdenes, y no se calcula nada de trading.
 *
 * - `DrawingShapes` dibuja los guardados dentro de la capa de precio del gráfico (la que se mueve
 *   con el encuadre), así siguen a las velas al desplazarse o hacer zoom. Con la herramienta de
 *   navegar, un clic sobre un dibujo lo selecciona.
 * - `DrawingInput` es la superficie que toma el puntero mientras hay una herramienta de dibujo
 *   elegida: arrastrar dibuja (la línea horizontal, con un clic alcanza).
 */
import { useRef, useState, type PointerEvent, type RefObject } from 'react'

import {
  drawingTitle,
  newDrawingId,
  type Anchor,
  type DrawKind,
  type Drawing,
} from '@/lib/chart-drawings'

const THIN = 'non-scaling-stroke'
const COLOR = 'var(--foreground)'
/** Un arrastre más corto que esto (px) no dibuja una tendencia ni un rectángulo. */
const MIN_PX = 4

/** De un punto del dibujo a la pantalla, con la escala que corresponda. */
type ToXY = (a: Anchor) => { x: number; y: number }

/** La figura de un dibujo, ya en coordenadas de pantalla. */
function Shape({
  d,
  xy,
  span,
  selected,
  draft,
}: {
  d: Pick<Drawing, 'kind' | 'a' | 'b'>
  xy: ToXY
  /** de dónde a dónde va la línea horizontal (de lado a lado, con margen para el arrastre) */
  span: [number, number]
  selected?: boolean
  draft?: boolean
}) {
  const p = xy(d.a)
  const q = xy(d.b)
  const common = {
    stroke: COLOR,
    strokeOpacity: draft ? 0.6 : 0.85,
    strokeWidth: selected ? 2 : 1.3,
    vectorEffect: THIN,
  } as const
  if (d.kind === 'hline') return <line x1={span[0]} x2={span[1]} y1={p.y} y2={p.y} {...common} />
  if (d.kind === 'trend') return <line x1={p.x} x2={q.x} y1={p.y} y2={q.y} {...common} />
  return (
    <rect
      x={Math.min(p.x, q.x)}
      y={Math.min(p.y, q.y)}
      width={Math.abs(q.x - p.x)}
      height={Math.abs(q.y - p.y)}
      fill={COLOR}
      fillOpacity={0.07}
      {...common}
    />
  )
}

/** Zona invisible y más ancha para tomar el dibujo con el puntero. */
function Hit({ d, xy, span }: { d: Drawing; xy: ToXY; span: [number, number] }) {
  const p = xy(d.a)
  const q = xy(d.b)
  const hit = {
    stroke: 'transparent',
    strokeWidth: 12,
    fill: 'none',
    vectorEffect: THIN,
    pointerEvents: 'stroke',
  } as const
  if (d.kind === 'hline') return <line x1={span[0]} x2={span[1]} y1={p.y} y2={p.y} {...hit} />
  if (d.kind === 'trend') return <line x1={p.x} x2={q.x} y1={p.y} y2={q.y} {...hit} />
  return (
    <rect
      x={Math.min(p.x, q.x)}
      y={Math.min(p.y, q.y)}
      width={Math.abs(q.x - p.x)}
      height={Math.abs(q.y - p.y)}
      {...hit}
    />
  )
}

/** Los dibujos guardados, dentro de la capa de precio del gráfico. */
export function DrawingShapes({
  drawings,
  xy,
  span,
  digits,
  selected,
  selectable,
  onSelect,
}: {
  drawings: readonly Drawing[]
  xy: ToXY
  span: [number, number]
  digits: number
  selected: string | null
  /** con la herramienta de navegar: un clic sobre el dibujo lo selecciona */
  selectable: boolean
  onSelect: (id: string) => void
}) {
  return (
    <g data-drawings={drawings.length}>
      {drawings.map((d) => {
        const on = d.id === selected
        const pts = d.kind === 'hline' ? [] : [d.a, d.b]
        return (
          <g
            key={d.id}
            data-drawing={d.kind}
            data-selected={on || undefined}
            pointerEvents={selectable ? undefined : 'none'}
            className={selectable ? 'cursor-pointer' : undefined}
            onPointerDown={
              selectable
                ? (e) => {
                    if (e.button !== 0) return
                    // el clic es del dibujo: el gráfico no empieza a desplazarse
                    e.stopPropagation()
                    onSelect(d.id)
                  }
                : undefined
            }
          >
            <title>{drawingTitle(d, digits)}</title>
            <Shape d={d} xy={xy} span={span} selected={on} />
            {selectable && <Hit d={d} xy={xy} span={span} />}
            {on &&
              pts.map((a, i) => {
                const p = xy(a)
                return (
                  <circle
                    key={i}
                    data-handle
                    cx={p.x}
                    cy={p.y}
                    r={3.5}
                    fill="var(--background)"
                    stroke={COLOR}
                    vectorEffect={THIN}
                    pointerEvents="none"
                  />
                )
              })}
          </g>
        )
      })}
    </g>
  )
}

/**
 * La superficie de dibujo, encima del panel de precio, mientras hay una herramienta elegida.
 * `toAnchor` y `xy` usan la escala vigente del gráfico.
 */
export function DrawingInput({
  tool,
  svgRef,
  width,
  height,
  plotW,
  top,
  bottom,
  toAnchor,
  xy,
  onCreate,
}: {
  tool: DrawKind
  svgRef: RefObject<SVGSVGElement | null>
  width: number
  height: number
  plotW: number
  top: number
  bottom: number
  toAnchor: (x: number, y: number) => Anchor | null
  xy: ToXY
  onCreate: (d: Drawing) => void
}) {
  const [draft, setDraft] = useState<{ a: Anchor; b: Anchor } | null>(null)
  const down = useRef<{ id: number; x: number; y: number } | null>(null)

  const toSvg = (e: PointerEvent) => {
    const r = svgRef.current?.getBoundingClientRect()
    if (!r || !r.width || !r.height) return null
    const x = ((e.clientX - r.left) / r.width) * width
    const y = ((e.clientY - r.top) / r.height) * height
    return { x: Math.max(0, Math.min(plotW, x)), y: Math.max(top, Math.min(bottom, y)) }
  }
  const anchorOf = (e: PointerEvent) => {
    const p = toSvg(e)
    return p ? toAnchor(p.x, p.y) : null
  }

  const onPointerDown = (e: PointerEvent<SVGRectElement>) => {
    if (e.button !== 0) return
    // el gesto es del dibujo: el gráfico no se desplaza ni mide
    e.stopPropagation()
    const a = anchorOf(e)
    if (!a) return
    try {
      e.currentTarget.setPointerCapture?.(e.pointerId)
    } catch {
      // sin captura el dibujo igual funciona dentro del gráfico
    }
    down.current = { id: e.pointerId, x: e.clientX, y: e.clientY }
    setDraft({ a, b: a })
  }
  const onPointerMove = (e: PointerEvent<SVGRectElement>) => {
    e.stopPropagation()
    if (!draft || down.current?.id !== e.pointerId) return
    const b = anchorOf(e)
    if (!b) return
    // la línea horizontal sigue al cursor en altura; las otras estiran su segundo punto
    setDraft(tool === 'hline' ? { a: b, b } : { a: draft.a, b })
  }
  const onPointerUp = (e: PointerEvent<SVGRectElement>) => {
    e.stopPropagation()
    const d = down.current
    down.current = null
    if (!draft || !d || d.id !== e.pointerId) return setDraft(null)
    const b = anchorOf(e) ?? draft.b
    const moved = Math.hypot(e.clientX - d.x, e.clientY - d.y) >= MIN_PX
    setDraft(null)
    if (tool === 'hline') return onCreate({ id: newDrawingId(), kind: 'hline', a: b, b })
    if (moved) onCreate({ id: newDrawingId(), kind: tool, a: draft.a, b })
  }
  const onPointerCancel = () => {
    down.current = null
    setDraft(null)
  }

  return (
    <g data-drawing-input={tool}>
      {draft && (
        <g data-drawing-draft pointerEvents="none">
          <Shape d={{ kind: tool, ...draft }} xy={xy} span={[0, plotW]} draft />
        </g>
      )}
      <rect
        aria-hidden
        x={0}
        y={top}
        width={plotW}
        height={bottom - top}
        fill="transparent"
        className="cursor-crosshair"
        style={{ touchAction: 'none' }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
      />
    </g>
  )
}
