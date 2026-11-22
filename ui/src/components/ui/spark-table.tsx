/**
 * Comparación de índices: una fila por instrumento con su mini curva, el valor y el cambio
 * del período. Pasar el puntero (o las flechas) recorre la misma fecha en todas las filas.
 *
 * Inspirado en "Spark Table" (21st.dev), reescrito en SVG (#164): el original dependía de
 * cinco archivos auxiliares que no venían. El cambio del período llega en los datos.
 */
import { useState, type KeyboardEvent, type PointerEvent } from 'react'

import { dateUtc, decimal, isNegative, signed } from '@/lib/format'
import { cn } from '@/lib/utils'
import type { SparkRow } from '@/mocks'

const W = 240
const H = 40

export function SparkTable({
  rows,
  periodLabel = 'Últimos 90 días',
}: {
  rows: SparkRow[]
  periodLabel?: string
}) {
  const [index, setIndex] = useState<number | null>(null)
  const [selected, setSelected] = useState(rows[0]?.id ?? '')
  const len = Math.max(0, ...rows.map((r) => r.points.length))
  const at = (r: SparkRow) => r.points[Math.min(r.points.length - 1, index ?? r.points.length - 1)]

  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    setIndex(
      Math.max(0, Math.min(len - 1, Math.round(((e.clientX - r.left) / r.width) * (len - 1)))),
    )
  }
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
    if (step) {
      e.preventDefault()
      setIndex((i) => Math.max(0, Math.min(len - 1, (i ?? len - 1) + step * (e.shiftKey ? 7 : 1))))
    } else if (e.key === 'Escape' || e.key === 'End') setIndex(null)
  }

  return (
    <div
      className="w-full"
      role="group"
      tabIndex={0}
      aria-label="Comparación de índices. Flechas para recorrer las fechas, Escape para volver al último dato."
      onKeyDown={onKey}
    >
      <div className="grid grid-cols-[minmax(0,1.3fr)_minmax(48px,1fr)_auto_auto] sm:grid-cols-[minmax(92px,1.1fr)_minmax(80px,2.4fr)_minmax(70px,1fr)_64px] items-center gap-2 sm:gap-3 border-b border-border pb-2 text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
        <span>Índice</span>
        <span className="text-center normal-case tracking-normal">
          {index === null ? periodLabel : dateUtc(rows[0].points[index].t)}
        </span>
        <span className="text-right">Valor</span>
        <span className="text-right">Cambio</span>
      </div>
      {rows.map((r) => {
        const vals = r.points.map((p) => Number(p.v))
        const lo = Math.min(...vals)
        const hi = Math.max(...vals)
        const x = (i: number) => (i / Math.max(1, vals.length - 1)) * W
        const y = (v: number) => 4 + (1 - (v - lo) / (hi - lo || 1)) * (H - 8)
        const d = vals
          .map((v, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`)
          .join('')
        const on = r.id === selected
        const k = Math.min(vals.length - 1, index ?? vals.length - 1)
        const neg = isNegative(r.changePct)
        return (
          <div
            key={r.id}
            className="grid grid-cols-[minmax(0,1.3fr)_minmax(48px,1fr)_auto_auto] sm:grid-cols-[minmax(92px,1.1fr)_minmax(80px,2.4fr)_minmax(70px,1fr)_64px] items-center gap-2 sm:gap-3 border-b border-border py-2 last:border-b-0"
          >
            <button
              type="button"
              aria-pressed={on}
              onClick={() => setSelected(r.id)}
              className="min-w-0 text-left"
            >
              <span className="flex items-center gap-1.5 truncate text-[13px] font-bold">
                <i
                  className={cn(
                    'size-1.5 shrink-0 rounded-full bg-[var(--chart-accent)]',
                    !on && 'opacity-0',
                  )}
                />
                {r.label}
              </span>
              <span className="block truncate pl-3 text-[10.5px] text-muted-foreground">
                {r.sublabel}
              </span>
            </button>
            <div
              className="touch-pan-y"
              onPointerMove={onMove}
              onPointerLeave={() => setIndex(null)}
            >
              <svg
                viewBox={`0 0 ${W} ${H}`}
                preserveAspectRatio="none"
                className="block h-10 w-full"
                aria-hidden="true"
              >
                <path
                  d={d}
                  fill="none"
                  stroke={on ? 'var(--chart-accent)' : 'var(--chart-line)'}
                  strokeOpacity={on ? 1 : 0.6}
                  strokeWidth={on ? 1.8 : 1.2}
                  vectorEffect="non-scaling-stroke"
                />
                {index !== null && (
                  <line
                    x1={x(k)}
                    x2={x(k)}
                    y1={0}
                    y2={H}
                    stroke="var(--foreground)"
                    strokeOpacity={0.3}
                    vectorEffect="non-scaling-stroke"
                  />
                )}
                <circle
                  cx={x(k)}
                  cy={y(vals[k])}
                  r={2.6}
                  fill={on ? 'var(--chart-accent)' : 'var(--foreground)'}
                />
              </svg>
            </div>
            <span className="text-right text-[13px] font-semibold">{decimal(at(r).v)}</span>
            <span className="flex justify-end">
              <span
                className={cn(
                  'rounded-md px-1.5 py-0.5 text-[11px] font-bold',
                  neg ? 'bg-down/10 text-down' : 'bg-up/10 text-up',
                )}
              >
                {signed(r.changePct)} %
              </span>
            </span>
          </div>
        )
      })}
    </div>
  )
}
