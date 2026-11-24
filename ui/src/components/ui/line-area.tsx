/**
 * Modo línea de los gráficos de precio (#377): la línea de los cierres con un área degradada
 * debajo que se desvanece hacia abajo, del color del signo del rango visible (verde si sube, rojo
 * si baja), una cuadrícula vertical tenue y un punto en el último precio.
 *
 * Solo dibuja los cierres que llegan; no calcula nada (CLAUDE.md §1). Está memoizado: el
 * gráfico no lo vuelve a dibujar al mover el cursor (eso lo hace la capa de `chart-measure`).
 */
import { memo, useId } from 'react'

import { TREND_COLOR, type Trend } from '@/components/ui/chart-measure'

export const LineArea = memo(function LineArea({
  points,
  top,
  bottom,
  gridX,
  trend,
}: {
  /** cada cierre ya ubicado en el SVG, en orden; los que no se pueden dibujar van como `null` */
  points: ([number, number] | null)[]
  top: number
  bottom: number
  /** x de las líneas verticales de la cuadrícula (las de las fechas) */
  gridX: number[]
  trend: Trend
}) {
  const id = useId()
  const color = TREND_COLOR[trend]
  const pts = points.filter((p): p is [number, number] => p !== null)
  if (pts.length === 0) return null
  const xy = pts.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`)
  const [lx, ly] = pts[pts.length - 1]
  return (
    <g data-line-area={trend} pointerEvents="none">
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={color} stopOpacity={0.3} />
          <stop offset="0.65" stopColor={color} stopOpacity={0.08} />
          <stop offset="1" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      {gridX.map((x) => (
        <line
          key={x}
          data-grid-x
          x1={x}
          x2={x}
          y1={top}
          y2={bottom}
          stroke="var(--chart-grid)"
          strokeDasharray="2 4"
        />
      ))}
      <path
        data-area
        d={`M${pts[0][0].toFixed(1)},${bottom}L${xy.join('L')}L${lx.toFixed(1)},${bottom}Z`}
        fill={`url(#${id})`}
      />
      <path
        data-close-line
        d={`M${xy.join('L')}`}
        fill="none"
        stroke={color}
        strokeWidth={1.8}
        strokeLinejoin="round"
        strokeLinecap="round"
      />
      <circle cx={lx} cy={ly} r={7} fill={color} fillOpacity={0.18} />
      <circle
        data-last-dot
        cx={lx}
        cy={ly}
        r={3.5}
        fill={color}
        stroke="var(--background)"
        strokeWidth={1.5}
      />
    </g>
  )
})
