/**
 * Gráfico del replay (#772): dibuja **solo** las velas que mandó el motor hasta el cursor, sin la
 * fecha real (el eje dice «día N · HH:MM» en UTC). Encima, las marcas y las entradas tal como las
 * devolvió el motor (#773). Tocar el gráfico propone un precio y una vela para marcar: es dibujo,
 * no un cálculo. No pide velas ni calcula nada de trading: `Number()` se usa solo para ubicar cada
 * cosa en pantalla.
 */
import type { MouseEvent } from 'react'

import { niceTicks } from '@/lib/chart-viewport'
import { decimal } from '@/lib/format'
import { barLabel } from '@/lib/replay-api'
import type { ReplayBar } from '@/lib/replay-contracts'
import { useElementWidth } from '@/lib/use-element-width'

const AXIS_W = 64
const AXIS_H = 18
/** velas a la vista: las últimas (el motor manda todas hasta el cursor) */
const SPAN = 120

/** Una línea horizontal (nivel, stop u objetivo) con su rótulo. */
export type ChartLine = { price: string; label: string; tone: 'mark' | 'order' }
/** Un punto en una vela (pivote o entrada) con su rótulo. */
export type ChartPoint = { index: number; price: string; label: string }

export function ReplayChart({
  bars,
  digits,
  height = 340,
  lines = [],
  points = [],
  onPick,
}: {
  bars: ReplayBar[]
  /** decimales del instrumento (de los precios que llegan) */
  digits: number
  height?: number
  lines?: ChartLine[]
  points?: ChartPoint[]
  /** tocar el gráfico: el precio bajo el puntero y la vela de esa columna */
  onPick?: (p: { price: string; bar: ReplayBar }) => void
}) {
  const [boxRef, width] = useElementWidth<HTMLDivElement>(720, 260)
  const shown = bars.slice(-SPAN)
  const last = bars.at(-1)
  const plotW = width - AXIS_W
  const plotH = height - AXIS_H
  const lows = shown.map((b) => Number(b.low))
  const highs = shown.map((b) => Number(b.high))
  const lo0 = Math.min(...lows)
  const hi0 = Math.max(...highs)
  const pad = (hi0 - lo0) * 0.06 || 1
  const [lo, hi] = [lo0 - pad, hi0 + pad]
  const slot = plotW / Math.max(shown.length, 1)
  const x = (i: number) => (i + 0.5) * slot
  const y = (p: number) => ((hi - p) / (hi - lo)) * plotH
  // lo que cae fuera del rango a la vista se dibuja en el borde
  const yIn = (p: string) => Math.min(plotH - 1, Math.max(1, y(Number(p))))
  const first = shown[0]?.index ?? 0
  const pick = (e: MouseEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    const [px, py] = [e.clientX - r.left, e.clientY - r.top]
    const bar = shown[Math.min(shown.length - 1, Math.max(0, Math.floor(px / slot)))]
    if (!onPick || !bar || px > plotW || py > plotH) return
    onPick({ price: (hi - (py / plotH) * (hi - lo)).toFixed(digits), bar })
  }
  const { ticks } = niceTicks(lo, hi, 6)
  // una hora en el eje cada ~80 px; con «dN» cuando cambia el día desde el rótulo anterior
  const every = Math.max(1, Math.ceil(shown.length / Math.max(2, Math.floor(plotW / 80))))
  return (
    <figure className="grid min-w-0 content-start gap-1.5 self-start">
      <figcaption aria-live="polite" className="font-mono text-[12px]">
        {last
          ? `${barLabel(last)} UTC · A ${decimal(last.open, digits)} · M ${decimal(last.high, digits)} · m ${decimal(last.low, digits)} · C ${decimal(last.close, digits)}`
          : 'Sin velas todavía'}
      </figcaption>
      <div ref={boxRef} className="min-w-0 overflow-hidden border border-border bg-card">
        <svg
          width={width}
          height={height}
          role="img"
          aria-label={`Velas del replay: ${shown.length} a la vista de ${bars.length} recibidas del motor${last ? `, la última del ${barLabel(last)} UTC` : ''}.`}
          className={onPick ? 'block cursor-crosshair' : 'block'}
          onClick={pick}
        >
          {ticks.map((t) => (
            <g key={t}>
              <line x1={0} x2={plotW} y1={y(t)} y2={y(t)} stroke="var(--chart-grid)" />
              <text
                x={plotW + 6}
                y={y(t)}
                dy="0.32em"
                className="fill-muted-foreground font-mono text-[10px]"
              >
                {decimal(String(t), digits)}
              </text>
            </g>
          ))}
          {shown.map((b, i) => {
            const [o, c] = [Number(b.open), Number(b.close)]
            const color = c >= o ? 'var(--chart-up)' : 'var(--chart-down)'
            const top = y(Math.max(o, c))
            const newDay = i > 0 && shown[i - 1].day !== b.day
            return (
              <g key={b.index}>
                {newDay && (
                  <line x1={i * slot} x2={i * slot} y1={0} y2={plotH} stroke="var(--rule)" />
                )}
                <line
                  x1={x(i)}
                  x2={x(i)}
                  y1={y(Number(b.high))}
                  y2={y(Number(b.low))}
                  stroke={color}
                />
                <rect
                  x={x(i) - Math.max(1, slot * 0.35)}
                  y={top}
                  width={Math.max(2, slot * 0.7)}
                  height={Math.max(1, y(Math.min(o, c)) - top)}
                  fill={color}
                />
                {i % every === 0 && (
                  <text
                    x={i === 0 ? 2 : x(i)}
                    y={plotH + 13}
                    textAnchor={i === 0 ? 'start' : 'middle'}
                    className="fill-muted-foreground font-mono text-[10px]"
                  >
                    {i === 0 || shown[i - every].day !== b.day ? `d${b.day} ${b.time}` : b.time}
                  </text>
                )}
              </g>
            )
          })}
          {lines.map((l, k) => (
            <g key={`l${k}`} className={l.tone === 'mark' ? 'text-info' : 'text-foreground'}>
              <line
                x1={0}
                x2={plotW}
                y1={yIn(l.price)}
                y2={yIn(l.price)}
                stroke="currentColor"
                strokeDasharray={l.tone === 'mark' ? undefined : '4 3'}
              />
              <text
                x={4}
                y={yIn(l.price) < 14 ? yIn(l.price) + 11 : yIn(l.price) - 3}
                fill="currentColor"
                className="font-mono text-[10px]"
              >
                {l.label} {decimal(l.price, digits)}
              </text>
            </g>
          ))}
          {points
            .filter((p) => p.index >= first)
            .map((p, k) => (
              <g key={`p${k}`} className="text-info">
                <circle cx={x(p.index - first)} cy={yIn(p.price)} r={4} fill="currentColor" />
                <text
                  x={x(p.index - first)}
                  y={yIn(p.price) - 7}
                  textAnchor="middle"
                  fill="currentColor"
                  className="font-mono text-[10px]"
                >
                  {p.label}
                </text>
              </g>
            ))}
        </svg>
      </div>
    </figure>
  )
}
