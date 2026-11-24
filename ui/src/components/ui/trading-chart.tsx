/**
 * Gráfico de precio con paneles de volumen, RSI y MACD.
 *
 * Une dos componentes de referencia que repetían el panel de precio (#164): "Candle Chart"
 * (velas, volumen, cruz de lectura, rangos) e "Indicator Chart" (RSI y MACD debajo).
 * Diferencia clave con el original: **no calcula indicadores**. RSI, MACD, señal e histograma
 * llegan en los datos, como los envía el motor (CLAUDE.md §1). Los textos se muestran tal cual
 * vienen; `Number()` se usa solo para ubicar cada cosa en pantalla.
 *
 * Lleva la herramienta de medición en el panel de precio (#281, `chart-measure.tsx`): una
 * medición visual del usuario sobre la escala del eje Y, no lógica de trading. Velas o línea
 * con área (#377); la cruz de lectura se desliza y la cabecera lee la vela bajo el cursor sin
 * volver a dibujar el gráfico.
 */
import { useMemo, useRef, useState, type KeyboardEvent } from 'react'

import {
  CHART_MODES,
  rangeTrend,
  usePointer,
  useChartMeasure,
  type ChartMode,
  type PointerStore,
} from '@/components/ui/chart-measure'
import { ChartFollower } from '@/components/ui/chart-follower'
import { ChartReadout } from '@/components/ui/chart-readout'
import { LineArea } from '@/components/ui/line-area'
import { dateUtc, decimal } from '@/lib/format'
import { useElementWidth } from '@/lib/use-element-width'
import { Segmented } from '@/components/ui/segmented'
import { cn } from '@/lib/utils'
import type { PriceBar } from '@/mocks'

const RANGES = [
  { label: '24 h', bars: 24 },
  { label: '3 días', bars: 72 },
  { label: '1 semana', bars: 120 },
  { label: 'Todo', bars: Infinity },
] as const

const AXIS_W = 62
const GAP = 16
const SHARE = { price: 0.52, volume: 0.12, rsi: 0.16, macd: 0.2 }

export function TradingChart({
  instrument,
  bars,
  height = 520,
  className,
}: {
  instrument: string
  bars: PriceBar[]
  height?: number
  className?: string
}) {
  const [range, setRange] = useState<(typeof RANGES)[number]['label']>('1 semana')
  const [mode, setMode] = useState<ChartMode>('Velas')
  const svgRef = useRef<SVGSVGElement>(null)
  const [boxRef, width] = useElementWidth<HTMLDivElement>(720, 300)

  const n = RANGES.find((r) => r.label === range)!.bars
  const view = useMemo(() => bars.slice(Math.max(0, bars.length - n)), [bars, n])
  const count = view.length
  const num = (s: string | null) => (s === null ? null : Number(s))

  const plotW = width - AXIS_W
  const slot = plotW / Math.max(count, 1)
  const body = Math.max(1.5, Math.min(slot * 0.62, 12))
  const x = (i: number) => i * slot + slot / 2

  const usable = height - GAP * 3 - 22
  const top = { price: 4, volume: 0, rsi: 0, macd: 0 }
  const h = {
    price: usable * SHARE.price,
    volume: usable * SHARE.volume,
    rsi: usable * SHARE.rsi,
    macd: usable * SHARE.macd,
  }
  top.volume = top.price + h.price + GAP
  top.rsi = top.volume + h.volume + GAP
  top.macd = top.rsi + h.rsi + GAP

  const lows = view.map((b) => Number(b.low))
  const highs = view.map((b) => Number(b.high))
  const lo = Math.min(...lows)
  const hi = Math.max(...highs)
  const pad = (hi - lo) * 0.08 || 1
  const yP = useMemo(
    () => (v: number) => top.price + (1 - (v - (lo - pad)) / (hi - lo + 2 * pad)) * h.price,
    [top.price, h.price, lo, hi, pad],
  )
  const maxVol = Math.max(...view.map((b) => Number(b.volume)), 1)
  const yR = (v: number) => top.rsi + (1 - v / 100) * h.rsi
  const macdAbs = Math.max(
    1e-9,
    ...view.flatMap((b) => [b.macd, b.signal, b.histogram].map((s) => Math.abs(num(s) ?? 0))),
  )
  const yM = (v: number) => top.macd + h.macd / 2 - (v / (macdAbs * 1.1)) * (h.macd / 2)

  const path = (key: 'close' | 'rsi14' | 'macd' | 'signal', y: (v: number) => number) => {
    let d = ''
    view.forEach((b, i) => {
      const v = num(b[key])
      if (v === null) return
      d += `${d ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`
    })
    return d
  }

  const ticks = [0, 0.25, 0.5, 0.75, 1].map((f) => lo - pad + f * (hi - lo + 2 * pad))
  const first = view[0]

  const last = view[count - 1]
  const starts = useMemo(() => view.map((b) => b.start), [view])
  const closeTexts = useMemo(() => view.map((b) => b.close), [view])
  const closes = useMemo(() => view.map((b) => Number(b.close)), [view])
  const digits = (last?.close.split('.')[1] ?? '').length
  const measure = useChartMeasure(svgRef, {
    width,
    height,
    plotW,
    top: top.price,
    bottom: top.price + h.price,
    slot,
    starts,
    closes,
    yOf: yP,
    digits,
    mode,
    // la lectura sigue al cursor en todos los paneles (volumen, RSI, MACD)
    anyY: true,
    // cruz de lectura: vertical en todos los paneles, horizontal y precio en el eje
    guide: ({ x: gx, y: gy, i }) => {
      const b = view[i]
      const up = Number(b.close) >= Number(b.open)
      return (
        <g data-crosshair>
          <line
            x1={gx}
            x2={gx}
            y1={0}
            y2={height - 18}
            stroke="var(--foreground)"
            opacity={0.3}
            strokeDasharray="3 3"
          />
          <line
            x1={0}
            x2={plotW}
            y1={gy}
            y2={gy}
            stroke="var(--foreground)"
            opacity={0.2}
            strokeDasharray="3 3"
          />
          <rect
            x={plotW + 2}
            y={gy - 9}
            width={AXIS_W - 4}
            height={18}
            rx={4}
            fill={up ? 'var(--chart-up)' : 'var(--chart-down)'}
          />
          <text
            x={plotW + AXIS_W / 2}
            y={gy + 3.5}
            textAnchor="middle"
            fontSize={10}
            fontWeight={700}
            fill="var(--primary-foreground)"
          >
            {decimal(b.close)}
          </text>
        </g>
      )
    },
  })

  const onKey = (e: KeyboardEvent<SVGSVGElement>) => {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
    if (step) {
      e.preventDefault()
      measure.step(step)
    } else if (e.key === 'Escape') measure.clear()
  }

  // Las velas (o la línea) se arman una vez por rango, ancho y modo: mover el cursor no las toca.
  const candles = useMemo(
    () =>
      mode === 'Línea'
        ? null
        : view.map((b, i) => {
            const color =
              Number(b.close) >= Number(b.open) ? 'var(--chart-up)' : 'var(--chart-down)'
            const yo = yP(Number(b.open))
            const yc = yP(Number(b.close))
            const cx = i * slot + slot / 2
            return (
              <g key={b.start} data-candle={b.start}>
                <line
                  x1={cx}
                  x2={cx}
                  y1={yP(Number(b.high))}
                  y2={yP(Number(b.low))}
                  stroke={color}
                />
                <rect
                  x={cx - body / 2}
                  y={Math.min(yo, yc)}
                  width={body}
                  height={Math.max(1, Math.abs(yc - yo))}
                  fill={color}
                />
              </g>
            )
          }),
    [mode, view, yP, slot, body],
  )
  const linePoints = useMemo(
    () =>
      closes.map((c, i): [number, number] | null =>
        Number.isFinite(c) ? [i * slot + slot / 2, yP(c)] : null,
      ),
    [closes, slot, yP],
  )

  const timeLabels = Array.from({ length: 5 }, (_, k) =>
    Math.min(count - 1, Math.round((k * (count - 1)) / 4)),
  )

  return (
    <div className={cn('w-full', className)}>
      <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase">
            {instrument} · 1 h
          </p>
          <ChartReadout
            store={measure.store}
            starts={starts}
            closes={closeTexts}
            rangeLabel={range}
          />
          <BarStats store={measure.store} view={view} />
        </div>
        <div className="flex flex-wrap gap-2">
          <Segmented
            label="Tipo de gráfico"
            options={CHART_MODES}
            value={mode}
            onChange={(v) => setMode(v as ChartMode)}
          />
          <Segmented
            label="Rango"
            options={RANGES.map((r) => r.label)}
            value={range}
            onChange={(v) => {
              setRange(v as typeof range)
              measure.clear()
            }}
          />
        </div>
      </div>

      <div ref={boxRef} className="relative w-full">
        <svg
          ref={svgRef}
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          tabIndex={0}
          aria-label={`${instrument}, ${count} velas de 1 hora desde ${first ? dateUtc(first.start) : ''}. Flechas izquierda y derecha para leer cada vela.`}
          className="block w-full touch-none select-none focus-visible:outline-2 focus-visible:outline-ring"
          {...measure.handlers}
          onKeyDown={onKey}
        >
          {/* precio */}
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
          {mode === 'Línea' ? (
            <LineArea
              points={linePoints}
              top={top.price}
              bottom={top.price + h.price}
              gridX={timeLabels.map((i) => x(i))}
              trend={rangeTrend(closes, digits)}
            />
          ) : (
            candles
          )}

          {/* volumen */}
          {view.map((b, i) => {
            const vh = (Number(b.volume) / maxVol) * h.volume
            return (
              <rect
                key={b.start}
                x={x(i) - body / 2}
                y={top.volume + h.volume - vh}
                width={body}
                height={vh}
                fill={Number(b.close) >= Number(b.open) ? 'var(--chart-up)' : 'var(--chart-down)'}
                opacity={0.45}
              />
            )
          })}
          <PaneLabel y={top.volume} text="Volumen" />

          {/* RSI */}
          <rect
            x={0}
            y={yR(70)}
            width={plotW}
            height={yR(30) - yR(70)}
            fill="var(--foreground)"
            opacity={0.04}
          />
          {[30, 70].map((lvl) => (
            <g key={lvl}>
              <line
                x1={0}
                x2={plotW}
                y1={yR(lvl)}
                y2={yR(lvl)}
                stroke={lvl === 70 ? 'var(--chart-down)' : 'var(--chart-up)'}
                strokeDasharray="3 3"
                opacity={0.6}
              />
              <text
                x={width - 4}
                y={yR(lvl) + 3}
                textAnchor="end"
                fontSize={10}
                fill="var(--muted-foreground)"
              >
                {lvl}
              </text>
            </g>
          ))}
          <path d={path('rsi14', yR)} fill="none" stroke="var(--chart-accent)" strokeWidth={1.5} />
          <PaneLabel y={top.rsi} text="RSI 14" />

          {/* MACD */}
          <line x1={0} x2={plotW} y1={yM(0)} y2={yM(0)} stroke="var(--chart-grid)" />
          {view.map((b, i) => {
            const v = num(b.histogram)
            if (v === null) return null
            return (
              <rect
                key={b.start}
                x={x(i) - body / 2}
                y={Math.min(yM(v), yM(0))}
                width={body}
                height={Math.max(0.75, Math.abs(yM(v) - yM(0)))}
                fill={v >= 0 ? 'var(--chart-up)' : 'var(--chart-down)'}
                opacity={0.5}
              />
            )
          })}
          <path d={path('macd', yM)} fill="none" stroke="var(--chart-accent)" strokeWidth={1.4} />
          <path
            d={path('signal', yM)}
            fill="none"
            stroke="var(--chart-signal)"
            strokeWidth={1.4}
            strokeDasharray="4 3"
          />
          <PaneLabel y={top.macd} text="MACD 12 · 26 · 9" />

          {/* fechas */}
          {timeLabels.map((i) =>
            view[i] ? (
              <text
                key={i}
                x={Math.max(24, Math.min(plotW - 24, x(i)))}
                y={height - 4}
                textAnchor="middle"
                fontSize={10}
                fill="var(--muted-foreground)"
              >
                {dateUtc(view[i].start)}
              </text>
            ) : null,
          )}

          <ChartFollower store={measure.store} plot={measure.plot} />
        </svg>
      </div>
    </div>
  )
}

/** Apertura, máximo, mínimo e indicadores de la vela bajo el cursor (o de la última). */
function BarStats({ store, view }: { store: PointerStore; view: PriceBar[] }) {
  const hover = usePointer(store, (s) => s.hover)
  const active = view[hover ?? view.length - 1]
  if (!active) return null
  return (
    <dl className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
      <Stat label="Ap" value={decimal(active.open)} />
      <Stat label="Máx" value={decimal(active.high)} />
      <Stat label="Mín" value={decimal(active.low)} />
      <Stat label="RSI" value={active.rsi14 ? decimal(active.rsi14) : '—'} />
      <Stat label="MACD" value={active.macd ? decimal(active.macd) : '—'} />
      <Stat label="Señal" value={active.signal ? decimal(active.signal) : '—'} />
    </dl>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-1">
      <dt>{label}</dt>
      <dd className="font-semibold text-foreground">{value}</dd>
    </div>
  )
}

function PaneLabel({ y, text }: { y: number; text: string }) {
  return (
    <text
      x={4}
      y={y + 10}
      fontSize={9.5}
      fontWeight={700}
      fill="var(--muted-foreground)"
      letterSpacing="0.06em"
    >
      {text.toUpperCase()}
    </text>
  )
}
