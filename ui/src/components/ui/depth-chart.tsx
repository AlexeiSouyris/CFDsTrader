/**
 * Profundidad de mercado: compras (bid) a la izquierda del medio, ventas (ask) a la derecha.
 * Inspirado en "Depth Chart" (21st.dev), reescrito (#164) sin el motor de gráficos que
 * faltaba en el original. Los totales acumulados llegan en los datos: la UI no los suma.
 */
import { useState, type PointerEvent } from 'react'

import { decimal } from '@/lib/format'
import { useElementWidth } from '@/lib/use-element-width'
import type { BookLevel, OrderBook } from '@/mocks'

const PAD = { top: 22, right: 54, bottom: 22, left: 8 }

export function DepthChart({
  instrument,
  book,
  height = 260,
}: {
  instrument: string
  book: OrderBook
  height?: number
}) {
  const [ref, width] = useElementWidth<HTMLDivElement>(560)
  const [probe, setProbe] = useState<{ x: number; level: BookLevel; bid: boolean } | null>(null)
  const mid = Number(book.mid)
  const lastBid = book.bids[book.bids.length - 1]
  const lastAsk = book.asks[book.asks.length - 1]
  const half = Math.max(mid - Number(lastBid.price), Number(lastAsk.price) - mid) || 1
  const maxTotal = Math.max(Number(lastBid.total), Number(lastAsk.total), 1)
  const x0 = PAD.left
  const x1 = width - PAD.right
  const y0 = PAD.top
  const y1 = height - PAD.bottom
  const xOf = (p: number) => x0 + ((p - (mid - half)) / (2 * half)) * (x1 - x0)
  const yOf = (t: number) => y1 - (t / maxTotal) * (y1 - y0)

  const step = (levels: BookLevel[]) => {
    let d = `M${xOf(mid)},${y1}`
    let prev = y1
    for (const l of levels) {
      const x = xOf(Number(l.price))
      d += `L${x},${prev}L${x},${yOf(Number(l.total))}`
      prev = yOf(Number(l.total))
    }
    return `${d}L${xOf(Number(levels[levels.length - 1].price))},${y1}Z`
  }

  const onMove = (e: PointerEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    const px = ((e.clientX - r.left) / r.width) * width
    const price = mid - half + ((px - x0) / (x1 - x0)) * 2 * half
    const bid = price < mid
    const side = bid ? book.bids : book.asks
    // el último nivel alcanzado desde el medio hacia el puntero
    const reached = side.filter((l) => (bid ? price <= Number(l.price) : price >= Number(l.price)))
    const level = reached[reached.length - 1]
    setProbe(level ? { x: Math.max(x0, Math.min(x1, px)), level, bid } : null)
  }

  return (
    <div className="w-full">
      <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase">
            {instrument} · profundidad
          </p>
          <p className="mt-1 text-[22px] leading-none font-bold">{decimal(book.mid)}</p>
        </div>
        <dl className="flex gap-4 text-right text-[11px]">
          <div>
            <dt className="text-muted-foreground">Spread</dt>
            <dd className="font-semibold">{decimal(book.spread)}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Compras</dt>
            <dd className="font-semibold text-up">{decimal(lastBid.total)}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Ventas</dt>
            <dd className="font-semibold text-down">{decimal(lastAsk.total)}</dd>
          </div>
        </dl>
      </div>
      <div ref={ref}>
        <svg
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label={`Profundidad de ${instrument}: medio ${decimal(book.mid)}, spread ${decimal(book.spread)}.`}
          className="block w-full touch-pan-y select-none"
          onPointerMove={onMove}
          onPointerLeave={() => setProbe(null)}
        >
          {[0.25, 0.5, 0.75, 1].map((f) => (
            <g key={f}>
              <line
                x1={x0}
                x2={x1}
                y1={yOf(maxTotal * f)}
                y2={yOf(maxTotal * f)}
                stroke="var(--chart-grid)"
              />
              <text
                x={width - 4}
                y={yOf(maxTotal * f) + 3}
                textAnchor="end"
                fontSize={10}
                fill="var(--muted-foreground)"
              >
                {(maxTotal * f).toFixed(0)}
              </text>
            </g>
          ))}
          <path
            d={step(book.bids)}
            fill="var(--chart-up)"
            fillOpacity={0.18}
            stroke="var(--chart-up)"
            strokeWidth={1.5}
          />
          <path
            d={step(book.asks)}
            fill="var(--chart-down)"
            fillOpacity={0.18}
            stroke="var(--chart-down)"
            strokeWidth={1.5}
          />
          <line
            x1={xOf(mid)}
            x2={xOf(mid)}
            y1={y0 - 6}
            y2={y1}
            stroke="var(--foreground)"
            opacity={0.45}
            strokeDasharray="3 3"
          />
          <text
            x={xOf(mid)}
            y={y0 - 9}
            textAnchor="middle"
            fontSize={10}
            fontWeight={700}
            fill="var(--foreground)"
          >
            MEDIO
          </text>
          {[mid - half, mid - half / 2, mid + half / 2, mid + half].map((p) => (
            <text
              key={p}
              x={Math.max(x0 + 30, Math.min(x1 - 30, xOf(p)))}
              y={height - 5}
              textAnchor="middle"
              fontSize={10}
              fill="var(--muted-foreground)"
            >
              {decimal(p.toFixed(1))}
            </text>
          ))}
          {probe && (
            <g pointerEvents="none">
              <line
                x1={probe.x}
                x2={probe.x}
                y1={y0}
                y2={y1}
                stroke="var(--foreground)"
                opacity={0.35}
                strokeDasharray="3 3"
              />
              <circle
                cx={probe.x}
                cy={yOf(Number(probe.level.total))}
                r={4}
                fill="var(--surface)"
                stroke={probe.bid ? 'var(--chart-up)' : 'var(--chart-down)'}
                strokeWidth={2}
              />
              <text
                x={Math.min(x1 - 60, probe.x + 8)}
                y={y0 + 12}
                fontSize={11}
                fontWeight={700}
                fill="var(--foreground)"
              >
                {decimal(probe.level.price)} · {decimal(probe.level.total)}
              </text>
            </g>
          )}
        </svg>
      </div>
    </div>
  )
}
