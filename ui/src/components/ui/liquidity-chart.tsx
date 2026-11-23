/**
 * Gráfico de liquidez en vivo (#231, spec §5.4.1): velas de 15m con los niveles de liquidez, los
 * FVG y las velas de desplazamiento que manda el motor.
 *
 * Sigue el estilo de `trading-chart.tsx` (SVG propio, sin librería de gráficos). **No calcula
 * nada:** ni niveles, ni estados, ni zonas. Solo los ubica en pantalla; `Number()` se usa para la
 * posición y los textos se muestran tal cual llegan.
 */
import { useMemo, useState } from 'react'

import { Segmented } from '@/components/ui/segmented'
import { dateTimeUtc, dateUtc, decimal } from '@/lib/format'
import { fvgKey, levelKey, levelName, levelShort, STATE_LABEL } from '@/lib/liquidity-labels'
import { useElementWidth } from '@/lib/use-element-width'
import { cn } from '@/lib/utils'
import type { FvgEvent, Imbalance, LevelState, LiquidityBar, LiquidityLevel } from '@/mocks'

const RANGES = [
  { label: '1 día', bars: 96 },
  { label: '2 días', bars: 192 },
  { label: 'Todo', bars: Infinity },
] as const
const FILTERS = ['Todos', 'Activos'] as const

const AXIS_W = 70
const SLOT_MS = 15 * 60_000

const STATE_STYLE: Record<LevelState, { stroke: string; dash?: string; opacity: number }> = {
  intact: { stroke: 'var(--chart-accent)', opacity: 0.95 },
  swept: { stroke: 'var(--chart-signal)', dash: '5 3', opacity: 0.95 },
  broken: { stroke: 'var(--muted-foreground)', dash: '1.5 3', opacity: 0.7 },
}

export function LiquidityChart({
  instrument,
  bars,
  levels,
  fvgs,
  imbalance,
  height = 440,
  className,
}: {
  instrument: string
  bars: LiquidityBar[]
  levels: LiquidityLevel[]
  fvgs: FvgEvent[]
  /** `imbalance.v1` por vela: la marca de desplazamiento la trae el motor */
  imbalance: Imbalance[]
  height?: number
  className?: string
}) {
  const [range, setRange] = useState<(typeof RANGES)[number]['label']>('2 días')
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>('Todos')
  const [boxRef, width] = useElementWidth<HTMLDivElement>(720, 300)

  const n = RANGES.find((r) => r.label === range)!.bars
  const view = useMemo(() => bars.slice(Math.max(0, bars.length - n)), [bars, n])
  const count = view.length
  const plotW = width - AXIS_W
  const slot = plotW / Math.max(count, 1)
  const body = Math.max(1.5, Math.min(slot * 0.62, 10))
  const top = 6
  const plotH = height - top - 22

  const lo = Math.min(...view.map((b) => Number(b.low)))
  const hi = Math.max(...view.map((b) => Number(b.high)))
  const pad = (hi - lo) * 0.06 || 1
  const yP = (v: number) => top + (1 - (v - (lo - pad)) / (hi - lo + 2 * pad)) * plotH
  const inScale = (v: number) => v >= lo - pad && v <= hi + pad

  // Ubica una hora (UTC) en el eje x: borde derecho de las velas que empezaron antes.
  const firstMs = count ? Date.parse(view[0].start) : 0
  const xAt = (iso: string | null) => {
    if (iso === null) return plotW
    const k = Math.ceil((Date.parse(iso) - firstMs) / SLOT_MS)
    return Math.max(0, Math.min(plotW, k * slot))
  }
  const lastClose = count ? new Date(Date.parse(view[count - 1].start) + SLOT_MS) : null

  // Activo = intacto y sin vencer (`expired_at` de liquidity_level.v1). Los vencidos se dibujan
  // atenuados hasta la vela en que vencieron.
  const expired = (l: LiquidityLevel) => l.state === 'intact' && l.expired_at != null
  const shownLevels = levels.filter(
    (l) =>
      (filter === 'Todos' || (l.state === 'intact' && !expired(l))) &&
      lastClose !== null &&
      Date.parse(l.exists_from) <= lastClose.getTime() &&
      xAt(l.state_changed_at ?? l.expired_at ?? null) > 0,
  )
  // Rótulos de los niveles intactos en el eje: los de fuera de escala van al borde con una flecha,
  // y se separan para que no se pisen (solo geometría de dibujo).
  const labels = shownLevels
    .filter((l) => l.state === 'intact' && !expired(l))
    .map((l) => {
      const v = Number(l.price)
      const arrow = v > hi + pad ? ' ↑' : v < lo - pad ? ' ↓' : ''
      return { l, arrow, y: Math.max(top + 6, Math.min(top + plotH - 2, yP(v))) }
    })
    .sort((a, b) => a.y - b.y)
  labels.forEach((t, k) => {
    if (k > 0) t.y = Math.max(t.y, labels[k - 1].y + 10)
  })
  const fvgEnd = (f: FvgEvent) => f.fill?.at ?? f.expiry?.at ?? null
  const shownFvgs = fvgs.filter((f) => xAt(fvgEnd(f)) > 0)
  const ticks = [0, 0.25, 0.5, 0.75, 1].map((f) => lo - pad + f * (hi - lo + 2 * pad))
  const timeLabels = Array.from({ length: 4 }, (_, k) =>
    Math.min(count - 1, Math.round((k * (count - 1)) / 3)),
  )
  const last = view[count - 1]

  return (
    <div className={cn('w-full', className)}>
      <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase">
            {instrument} · 15 m · liquidez
          </p>
          {last && (
            <p className="mt-1 text-[26px] leading-none font-bold tracking-[-0.03em]">
              {decimal(last.close)}
            </p>
          )}
          {lastClose && (
            <p className="mt-1 text-[11px] text-muted-foreground">
              Última vela cerrada: {dateTimeUtc(lastClose.toISOString())}
            </p>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          <Segmented
            label="Niveles"
            options={FILTERS}
            value={filter}
            onChange={(v) => setFilter(v as typeof filter)}
          />
          <Segmented
            label="Rango"
            options={RANGES.map((r) => r.label)}
            value={range}
            onChange={(v) => setRange(v as typeof range)}
          />
        </div>
      </div>

      <div ref={boxRef} className="relative w-full">
        <svg
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label={`${instrument}, ${count} velas de 15 minutos con ${shownLevels.length} niveles de liquidez y ${shownFvgs.length} FVG.`}
          className="block w-full select-none"
        >
          {ticks.map((t) => (
            <g key={t}>
              <line x1={0} x2={plotW} y1={yP(t)} y2={yP(t)} stroke="var(--chart-grid)" />
              <text
                x={width - 4}
                y={yP(t) + 3}
                textAnchor="end"
                fontSize={10}
                fill="var(--muted-foreground)"
              >
                {decimal(t.toFixed(1))}
              </text>
            </g>
          ))}

          {/* FVG: zona desde la vela de creación hasta la que la llena (o hasta ahora) */}
          {shownFvgs.map((f) => {
            const x0 = xAt(f.created_at)
            const x1 = xAt(fvgEnd(f))
            const yTop = yP(Number(f.top))
            const yBot = yP(Number(f.bottom))
            const color = f.direction === 'bullish' ? 'var(--chart-up)' : 'var(--chart-down)'
            return (
              <rect
                key={fvgKey(f)}
                data-fvg={f.status}
                x={x0}
                y={yTop}
                width={Math.max(1, x1 - x0)}
                height={Math.max(1, yBot - yTop)}
                fill={color}
                opacity={f.status === 'open' ? 0.22 : 0.08}
                stroke={color}
                strokeOpacity={f.status === 'open' ? 0.5 : 0.25}
                strokeDasharray={f.status === 'open' ? undefined : '2 2'}
              />
            )
          })}

          {/* velas */}
          {view.map((b, i) => {
            const color =
              Number(b.close) >= Number(b.open) ? 'var(--chart-up)' : 'var(--chart-down)'
            const yo = yP(Number(b.open))
            const yc = yP(Number(b.close))
            const x = i * slot + slot / 2
            return (
              <g key={b.start}>
                <line x1={x} x2={x} y1={yP(Number(b.high))} y2={yP(Number(b.low))} stroke={color} />
                <rect
                  x={x - body / 2}
                  y={Math.min(yo, yc)}
                  width={body}
                  height={Math.max(1, Math.abs(yc - yo))}
                  fill={color}
                />
              </g>
            )
          })}

          {/* velas de desplazamiento (marca del motor) */}
          {imbalance.map((d) => {
            if (d.displacement === null) return null
            const x = xAt(d.at) - slot / 2
            if (x < 0) return null
            const y = d.displacement === 'bullish' ? height - 26 : top + 4
            return (
              <text
                key={d.at}
                x={x}
                y={y}
                textAnchor="middle"
                fontSize={9}
                fill={d.displacement === 'bullish' ? 'var(--chart-up)' : 'var(--chart-down)'}
              >
                {d.displacement === 'bullish' ? '▲' : '▼'}
              </text>
            )
          })}

          {/* niveles: desde que existen hasta la vela que cambió su estado */}
          {shownLevels.map((l) => {
            const v = Number(l.price)
            if (!inScale(v)) return null
            const s = STATE_STYLE[l.state]
            const x0 = xAt(l.exists_from)
            const x1 = xAt(l.state_changed_at ?? l.expired_at ?? null)
            const y = yP(v)
            const old = expired(l)
            return (
              <g
                key={levelKey(l)}
                data-level-state={l.state}
                data-expired={old || undefined}
                opacity={old ? 0.3 : s.opacity}
              >
                <line
                  x1={x0}
                  x2={x1}
                  y1={y}
                  y2={y}
                  stroke={s.stroke}
                  strokeWidth={1.3}
                  strokeDasharray={s.dash}
                />
                {l.state !== 'intact' && (
                  <circle cx={x1} cy={y} r={2.6} fill={s.stroke}>
                    <title>{`${levelName(l)}: ${STATE_LABEL[l.state].toLowerCase()}`}</title>
                  </circle>
                )}
              </g>
            )
          })}

          {/* rótulos de los niveles intactos (los de fuera de escala, con flecha) */}
          {labels.map(({ l, arrow, y }) => (
            <g key={levelKey(l)}>
              <rect
                x={plotW + 1}
                y={y - 6}
                width={AXIS_W - 2}
                height={11}
                rx={3}
                fill="var(--background)"
                opacity={0.9}
              />
              <text
                x={plotW + 4}
                y={y + 3}
                fontSize={9}
                fontWeight={700}
                fill="var(--chart-accent)"
              >
                <title>{`${levelName(l)}: ${decimal(l.price)}`}</title>
                {`${levelShort(l)}${arrow}`}
              </text>
            </g>
          ))}

          {timeLabels.map((i) =>
            view[i] ? (
              <text
                key={i}
                x={Math.max(24, Math.min(plotW - 24, i * slot + slot / 2))}
                y={height - 4}
                textAnchor="middle"
                fontSize={10}
                fill="var(--muted-foreground)"
              >
                {dateUtc(view[i].start)}
              </text>
            ) : null,
          )}
        </svg>
      </div>

      <ul className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-muted-foreground">
        {(Object.keys(STATE_STYLE) as LevelState[]).map((st) => (
          <li key={st} className="flex items-center gap-1.5">
            <svg width={22} height={6} aria-hidden>
              <line
                x1={0}
                x2={22}
                y1={3}
                y2={3}
                stroke={STATE_STYLE[st].stroke}
                strokeWidth={1.6}
                strokeDasharray={STATE_STYLE[st].dash}
              />
            </svg>
            {STATE_LABEL[st]}
          </li>
        ))}
        <li className="flex items-center gap-1.5">
          <svg width={22} height={6} aria-hidden opacity={0.3}>
            <line x1={0} x2={22} y1={3} y2={3} stroke="var(--chart-accent)" strokeWidth={1.6} />
          </svg>
          Vencido
        </li>
        <li className="flex items-center gap-1.5">
          <span className="inline-block h-2.5 w-4 rounded-[2px] bg-up/30" aria-hidden />
          FVG abierto
        </li>
        <li className="flex items-center gap-1.5">
          <span
            className="inline-block h-2.5 w-4 rounded-[2px] border border-dashed border-muted-foreground/50"
            aria-hidden
          />
          FVG lleno o vencido
        </li>
        <li>▲ ▼ vela de desplazamiento</li>
      </ul>
    </div>
  )
}
