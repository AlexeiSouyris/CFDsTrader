/**
 * La capa que se mueve de los gráficos (#355, #377): el punto que sigue al cierre, la guía
 * vertical y la medición. Lee el estado del cursor de `chart-measure` y es lo único que se vuelve
 * a dibujar al mover el mouse o el dedo. Solo dibujo: las cuentas son las de la medición.
 * La etiqueta de la medición con velas es una pestaña pegada al rectángulo (#394).
 * En los gráficos navegables (#401) también sigue al encuadre vivo (`useFrame`) y la medición
 * se recorta al panel de precio.
 */
import { useId, type ReactNode } from 'react'

import {
  measureTab,
  measureText,
  rangeTrend,
  segment,
  span,
  TREND_COLOR,
  useFrame,
  usePointer,
  type MeasurePlot,
  type PointerStore,
} from '@/components/ui/chart-measure'

/** La capa que se mueve: punto, guía y medición. Es lo único que se dibuja al mover el cursor. */
export function ChartFollower({ store, plot }: { store: PointerStore; plot: MeasurePlot }) {
  const s = usePointer(store, (st) => st)
  const gradId = useId()
  const clipId = useId()
  const frame = useFrame(plot)
  const { plotW, top, bottom, starts, closes, digits } = plot
  const { slot, yOf } = frame
  const line = plot.mode === 'Línea'
  const n = starts.length
  const xOf = (f: number) => (f - frame.start) * slot + slot / 2
  const trend = plot.trend ?? rangeTrend(closes, digits)

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
      const x0 = xOf(Math.min(ia, e.idx)) - slot / 2
      const x1 = xOf(Math.max(ia, e.idx)) + slot / 2
      const yTop = Math.min(ya, yb)
      const yBot = Math.max(ya, yb)
      // pestaña pegada al rectángulo (#394): sigue al rectángulo, no al cursor
      const tab = measureTab({
        x0,
        x1,
        yTop,
        yBot,
        top,
        bottom,
        plotW,
        parts: [
          t.price,
          t.pct,
          `${bars} ${bars === 1 ? 'vela' : 'velas'}`,
          span(Date.parse(starts[ib]) - Date.parse(starts[ia])),
        ],
      })
      const { x: tx, y: ty, w: tw, h: th } = tab
      const r = 5
      // borde abierto del lado que toca el rectángulo: la pestaña y el rectángulo quedan unidos
      const above = tab.side === 'above'
      const shape = above
        ? `M${tx},${ty + th}V${ty + r}Q${tx},${ty} ${tx + r},${ty}H${tx + tw - r}Q${tx + tw},${ty} ${tx + tw},${ty + r}V${ty + th}`
        : `M${tx},${ty}V${ty + th - r}Q${tx},${ty + th} ${tx + r},${ty + th}H${tx + tw - r}Q${tx + tw},${ty + th} ${tx + tw},${ty + th - r}V${ty}`
      // si la pestaña es más ancha que el rectángulo, se cierra el borde en lo que sobresale
      const ey = above ? ty + th : ty
      const edge = [
        [tx, Math.min(x0, tx + tw)],
        [Math.max(x1, tx), tx + tw],
      ]
        .filter(([u, v]) => v - u > 0.5)
        .map(([u, v]) => `M${u},${ey}H${v}`)
        .join('')
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
            y={yTop}
            width={Math.max(1, x1 - x0)}
            height={Math.max(1, yBot - yTop)}
            fill={color}
            fillOpacity={0.16}
            stroke={color}
            strokeOpacity={0.7}
          />
          <line x1={x0} x2={x1} y1={ya} y2={ya} stroke={color} strokeDasharray="3 3" />
          {points}
          <g data-measure-tab={tab.side} data-x={tx} data-y={ty} data-w={tw} data-h={th}>
            <path d={`${shape}Z`} fill="var(--background)" fillOpacity={0.82} />
            <path d={`${shape}Z`} fill={color} fillOpacity={0.16} />
            <path d={`${shape}${edge}`} fill="none" stroke={color} strokeOpacity={0.7} />
            {tab.lines.map((l, n) => {
              const [a, b, ...rest] = l.split(' · ')
              const colored = n === 0
              return (
                <text
                  key={n}
                  data-measure-line={n}
                  x={tx + tw / 2}
                  y={ty + 13 + n * 13}
                  textAnchor="middle"
                  fontSize={11}
                  fill="var(--foreground)"
                >
                  {colored ? (
                    <>
                      <tspan fontWeight={700} fill={color}>
                        {`${a} · ${b}`}
                      </tspan>
                      {rest.length > 0 && (
                        <tspan fillOpacity={0.8}>{` · ${rest.join(' · ')}`}</tspan>
                      )}
                    </>
                  ) : (
                    <tspan fillOpacity={0.8}>{l}</tspan>
                  )}
                </text>
              )
            })}
          </g>
        </g>
      )
    }
  }

  return (
    <>
      {overlay && (
        <>
          <clipPath id={clipId}>
            <rect x={0} y={top} width={plotW} height={Math.max(0, bottom - top)} />
          </clipPath>
          <g clipPath={`url(#${clipId})`}>{overlay}</g>
        </>
      )}
      {dot}
    </>
  )
}
