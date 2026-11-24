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
 * medición visual del usuario sobre la escala del eje Y, no lógica de trading.
 */
import { useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react'

import { useChartMeasure } from '@/components/ui/chart-measure'
import { dateTimeUtc, dateUtc, decimal } from '@/lib/format'
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

type Kind = 'Velas' | 'Línea'

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
  const [kind, setKind] = useState<Kind>('Velas')
  const [hover, setHover] = useState<number | null>(null)
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
  const yP = (v: number) => top.price + (1 - (v - (lo - pad)) / (hi - lo + 2 * pad)) * h.price
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
  const active = view[hover ?? count - 1]
  const first = view[0]
  const up = active ? Number(active.close) >= Number(active.open) : true

  const last = view[count - 1]
  const measure = useChartMeasure(svgRef, {
    width,
    height,
    plotW,
    top: top.price,
    bottom: top.price + h.price,
    slot,
    starts: view.map((b) => b.start),
    closes: view.map((b) => Number(b.close)),
    yOf: yP,
    digits: (last?.close.split('.')[1] ?? '').length,
    // el punto sigue a la cruz de lectura, que ya conoce la vela bajo el cursor
    hover,
  })

  const onPointer = (e: PointerEvent<SVGSVGElement>) => {
    const r = svgRef.current?.getBoundingClientRect()
    if (!r) return
    const px = ((e.clientX - r.left) / r.width) * width
    setHover(px > plotW ? null : Math.max(0, Math.min(count - 1, Math.floor(px / slot))))
  }
  const onKey = (e: KeyboardEvent<SVGSVGElement>) => {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
    if (step) {
      e.preventDefault()
      setHover((i) => Math.max(0, Math.min(count - 1, (i ?? count - 1) + step)))
    } else if (e.key === 'Escape') setHover(null)
  }

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
          {active && (
            <>
              <p className="mt-1 text-[26px] leading-none font-bold tracking-[-0.03em]">
                {decimal(active.close)}
              </p>
              <dl className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                <Stat label="Ap" value={decimal(active.open)} />
                <Stat label="Máx" value={decimal(active.high)} />
                <Stat label="Mín" value={decimal(active.low)} />
                <Stat label="RSI" value={active.rsi14 ? decimal(active.rsi14) : '—'} />
                <Stat label="MACD" value={active.macd ? decimal(active.macd) : '—'} />
                <Stat label="Señal" value={active.signal ? decimal(active.signal) : '—'} />
                <dd className="text-foreground/70">{dateTimeUtc(active.start)}</dd>
              </dl>
            </>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          <Segmented
            label="Tipo de gráfico"
            options={['Velas', 'Línea']}
            value={kind}
            onChange={(v) => setKind(v as Kind)}
          />
          <Segmented
            label="Rango"
            options={RANGES.map((r) => r.label)}
            value={range}
            onChange={(v) => {
              setRange(v as typeof range)
              setHover(null)
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
          onPointerMove={(e) => {
            onPointer(e)
            measure.handlers.onPointerMove(e)
          }}
          onPointerDown={(e) => {
            onPointer(e)
            measure.handlers.onPointerDown(e)
          }}
          onPointerUp={measure.handlers.onPointerUp}
          onPointerCancel={measure.handlers.onPointerCancel}
          onPointerLeave={() => setHover(null)}
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
          {kind === 'Línea' ? (
            <path d={path('close', yP)} fill="none" stroke="var(--chart-line)" strokeWidth={1.6} />
          ) : (
            view.map((b, i) => {
              const color =
                Number(b.close) >= Number(b.open) ? 'var(--chart-up)' : 'var(--chart-down)'
              const yo = yP(Number(b.open))
              const yc = yP(Number(b.close))
              return (
                <g
                  key={b.start}
                  data-candle={b.start}
                  opacity={hover !== null && hover !== i ? 0.5 : 1}
                >
                  <line
                    x1={x(i)}
                    x2={x(i)}
                    y1={yP(Number(b.high))}
                    y2={yP(Number(b.low))}
                    stroke={color}
                  />
                  <rect
                    x={x(i) - body / 2}
                    y={Math.min(yo, yc)}
                    width={body}
                    height={Math.max(1, Math.abs(yc - yo))}
                    fill={color}
                  />
                </g>
              )
            })
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

          {/* cruz de lectura */}
          {hover !== null && active && (
            <g pointerEvents="none">
              <line
                x1={x(hover)}
                x2={x(hover)}
                y1={0}
                y2={height - 18}
                stroke="var(--foreground)"
                opacity={0.3}
                strokeDasharray="3 3"
              />
              <line
                x1={0}
                x2={plotW}
                y1={yP(Number(active.close))}
                y2={yP(Number(active.close))}
                stroke="var(--foreground)"
                opacity={0.2}
                strokeDasharray="3 3"
              />
              <rect
                x={plotW + 2}
                y={yP(Number(active.close)) - 9}
                width={AXIS_W - 4}
                height={18}
                rx={4}
                fill={up ? 'var(--chart-up)' : 'var(--chart-down)'}
              />
              <text
                x={plotW + AXIS_W / 2}
                y={yP(Number(active.close)) + 3.5}
                textAnchor="middle"
                fontSize={10}
                fontWeight={700}
                fill="var(--primary-foreground)"
              >
                {decimal(active.close)}
              </text>
            </g>
          )}

          {measure.overlay}
        </svg>
      </div>
    </div>
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
