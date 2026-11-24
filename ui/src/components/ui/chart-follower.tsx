/**
 * La capa que se mueve de los gráficos (#355, #377): el punto que sigue al cierre, la guía
 * vertical y la medición. Lee el estado del cursor de `chart-measure` y es lo único que se vuelve
 * a dibujar al mover el mouse o el dedo. Solo dibujo: las cuentas son las de la medición.
 */
import { useId, type ReactNode } from 'react'

import {
  measureText,
  rangeTrend,
  segment,
  span,
  TREND_COLOR,
  usePointer,
  type MeasurePlot,
  type PointerStore,
} from '@/components/ui/chart-measure'

/** La capa que se mueve: punto, guía y medición. Es lo único que se dibuja al mover el cursor. */
export function ChartFollower({ store, plot }: { store: PointerStore; plot: MeasurePlot }) {
  const s = usePointer(store, (st) => st)
  const gradId = useId()
  const { plotW, top, bottom, slot, starts, closes, yOf, digits } = plot
  const line = plot.mode === 'Línea'
  const n = starts.length
  const xOf = (f: number) => f * slot + slot / 2
  const trend = rangeTrend(closes, digits)

  // el punto que sigue al gráfico: en la vela bajo el cursor, a la altura de su cierre
  const hi = s.hover !== null && s.hover >= 0 && s.hover < n ? s.hover : null
  let dot: ReactNode = null
  if (hi !== null && s.dot && s.dotGoal && Number.isFinite(s.dotGoal.price)) {
    const x = xOf(s.dot.idx)
    const y = yOf(s.dot.price)
    // en modo línea, del color del rango; mientras se mide, del color de la medición
    const m = s.measure
    const shown = m && starts.includes(m.from.start) && starts.includes(m.to.start)
    const color = !line
      ? 'var(--foreground)'
      : TREND_COLOR[shown ? measureText(m.from.price, m.to.price, digits).trend : trend]
    dot = (
      <g
        data-hover-dot={starts[hi]}
        data-x={xOf(s.dotGoal.idx)}
        data-y={yOf(s.dotGoal.price)}
        pointerEvents="none"
        aria-hidden
      >
        {plot.guide ? (
          plot.guide({ x, y, i: hi })
        ) : (
          // línea vertical tenue, salvo que el gráfico dibuje su propia guía
          <line
            x1={x}
            x2={x}
            y1={top}
            y2={bottom}
            stroke="var(--foreground)"
            strokeOpacity={line ? 0.28 : 0.18}
            strokeDasharray={line ? '2 3' : undefined}
          />
        )}
        {line && <circle cx={x} cy={y} r={9} fill={color} fillOpacity={0.18} />}
        <circle
          data-dot
          cx={x}
          cy={y}
          r={line ? 4.5 : 4}
          fill={color}
          stroke="var(--background)"
          strokeWidth={1.5}
        />
      </g>
    )
  }

  let overlay: ReactNode = null
  const m = s.measure
  const ia = m ? starts.indexOf(m.from.start) : -1
  const ib = m ? starts.indexOf(m.to.start) : -1
  // si una de las velas ya no está a la vista (cambió el rango), la medición no se dibuja
  if (m && ia >= 0 && ib >= 0) {
    const t = measureText(m.from.price, m.to.price, digits)
    const color = TREND_COLOR[t.trend]
    const e = s.end ?? { idx: ib, price: m.to.price }
    const ya = yOf(m.from.price)
    const yb = yOf(e.price)
    const ends = [
      { k: 'from', idx: ia, y: ya, goal: ya },
      { k: 'to', idx: e.idx, y: yb, goal: yOf(m.to.price) },
    ]
    const points = ends.map(({ k, idx, y, goal }) => (
      <circle
        key={k}
        data-measure-point={k}
        data-y={goal}
        cx={xOf(idx)}
        cy={y}
        r={3.5}
        fill={color}
        stroke="var(--background)"
        strokeWidth={1.5}
      />
    ))
    if (line) {
      // modo línea: se atenúa lo de afuera y el tramo se pinta con el color del signo
      const xa = xOf(ia)
      const xb = xOf(e.idx)
      const x0 = Math.max(0, Math.min(xa, xb))
      const x1 = Math.min(plotW, Math.max(xa, xb))
      const pts = segment(closes, ia, e.idx, xOf, yOf)
      overlay = (
        <g
          data-measure={t.trend}
          data-mode="line"
          data-from={t.from}
          data-to={t.to}
          pointerEvents="none"
          aria-hidden
        >
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stopColor={color} stopOpacity={0.34} />
              <stop offset="1" stopColor={color} stopOpacity={0} />
            </linearGradient>
          </defs>
          <rect
            x={0}
            y={top}
            width={x0}
            height={bottom - top}
            fill="var(--background)"
            fillOpacity={0.55}
          />
          <rect
            x={x1}
            y={top}
            width={Math.max(0, plotW - x1)}
            height={bottom - top}
            fill="var(--background)"
            fillOpacity={0.55}
          />
          {pts.length > 1 && (
            <>
              <path
                d={`M${pts[0].split(',')[0]},${bottom}L${pts.join('L')}L${pts[pts.length - 1].split(',')[0]},${bottom}Z`}
                fill={`url(#${gradId})`}
              />
              <path
                data-measure-segment
                d={`M${pts.join('L')}`}
                fill="none"
                stroke={color}
                strokeWidth={2.4}
                strokeLinejoin="round"
                strokeLinecap="round"
              />
            </>
          )}
          {[xa, xb].map((x, k) => (
            <line
              key={k}
              x1={x}
              x2={x}
              y1={top}
              y2={bottom}
              stroke={color}
              strokeOpacity={0.45}
              strokeDasharray="2 3"
            />
          ))}
          {points}
        </g>
      )
    } else {
      const bars = Math.abs(ib - ia)
      const lines = [
        `${t.price} (${t.pct})`,
        `${bars} ${bars === 1 ? 'vela' : 'velas'} · ${span(Date.parse(starts[ib]) - Date.parse(starts[ia]))}`,
      ]
      const x0 = Math.min(ia, e.idx) * slot
      const x1 = (Math.max(ia, e.idx) + 1) * slot
      // etiqueta junto al cursor (el punto final), del lado donde entra
      const cx = xOf(e.idx)
      const w = Math.max(...lines.map((l) => l.length)) * 6.4 + 16
      const h = 36
      const lx = cx + 10 + w > plotW ? Math.max(0, cx - 10 - w) : cx + 10
      const ly = yb - h - 8 < top ? Math.min(bottom - h, yb + 8) : yb - h - 8
      overlay = (
        <g
          data-measure={t.trend}
          data-mode="candles"
          data-from={t.from}
          data-to={t.to}
          pointerEvents="none"
          aria-hidden
        >
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
          {points}
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
  }

  return (
    <>
      {overlay}
      {dot}
    </>
  )
}
