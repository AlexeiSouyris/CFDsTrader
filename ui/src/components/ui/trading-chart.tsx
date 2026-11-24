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
 *
 * Navegable (#401, `chart-nav.tsx`): arrastrar desplaza en el tiempo, la rueda hace zoom anclado
 * en el cursor y el eje de precio se ajusta solo o se estira a mano. Medir es Shift + arrastrar
 * o el botón de regla.
 */
import { useId, useMemo, useRef, useState } from 'react'

import {
  CHART_MODES,
  rangeTrend,
  usePointer,
  useChartMeasure,
  type ChartMode,
  type PointerStore,
} from '@/components/ui/chart-measure'
import { ChartFollower } from '@/components/ui/chart-follower'
import { ChartNavBar, ViewportLive } from '@/components/ui/chart-nav-bar'
import { matchRange, useChartNav, useViewport } from '@/components/ui/chart-nav'
import { ChartReadout } from '@/components/ui/chart-readout'
import { LineArea } from '@/components/ui/line-area'
import { niceTicks, renderWindow, timeTicks, visibleRange } from '@/lib/chart-viewport'
import { dateUtc, decimal, timeTickLabel } from '@/lib/format'
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
/** trazos que no se engrosan ni se afinan cuando la capa se estira durante el zoom (#401) */
const THIN = 'non-scaling-stroke'

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
  const [mode, setMode] = useState<ChartMode>('Velas')
  const [ruler, setRuler] = useState(false)
  const clipPrice = useId()
  const clipSub = useId()
  const svgRef = useRef<SVGSVGElement>(null)
  const [boxRef, width] = useElementWidth<HTMLDivElement>(720, 300)
  const num = (s: string | null) => (s === null ? null : Number(s))

  const plotW = width - AXIS_W
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

  const starts = useMemo(() => bars.map((b) => b.start), [bars])
  const closeTexts = useMemo(() => bars.map((b) => b.close), [bars])
  const closes = useMemo(() => bars.map((b) => Number(b.close)), [bars])
  const lows = useMemo(() => bars.map((b) => Number(b.low)), [bars])
  const highs = useMemo(() => bars.map((b) => Number(b.high)), [bars])
  const n = bars.length

  // Encuadre (#401): las velas se dibujan con `base` y se mueven con una transformación.
  const vp = useViewport({
    starts,
    lows,
    highs,
    initial: 120,
    plotW,
    top: top.price,
    h: h.price,
  })
  const { base, baseMap, priceLayer, xLayer } = vp
  const [a, b] = renderWindow(base, n)
  const [va, vb] = visibleRange(base, n)
  const count = vb - va
  const slot = baseMap.slot
  const body = Math.max(1.5, Math.min(slot * 0.62, 12))
  const x = baseMap.x
  const yP = baseMap.y
  const visible = useMemo(() => bars.slice(va, vb), [bars, va, vb])
  const range = matchRange(base, n, RANGES)

  // los paneles de abajo se escalan con lo visible al dibujar (no cambian cuadro a cuadro)
  const maxVol = Math.max(...visible.map((v) => Number(v.volume)), 1)
  const yR = (v: number) => top.rsi + (1 - v / 100) * h.rsi
  const macdAbs = Math.max(
    1e-9,
    ...visible.flatMap((v) => [v.macd, v.signal, v.histogram].map((t) => Math.abs(num(t) ?? 0))),
  )
  const yM = (v: number) => top.macd + h.macd / 2 - (v / (macdAbs * 1.1)) * (h.macd / 2)

  const path = (key: 'close' | 'rsi14' | 'macd' | 'signal', y: (v: number) => number) => {
    let d = ''
    for (let i = a; i < b; i++) {
      const v = num(bars[i][key])
      if (v === null) continue
      d += `${d ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`
    }
    return d
  }

  const first = bars[va]
  const last = bars[n - 1]
  const digits = (last?.close.split('.')[1] ?? '').length
  const trend = rangeTrend(
    visible.map((v) => Number(v.close)),
    digits,
  )
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
    frame: vp.frame,
    trend,
    // la lectura sigue al cursor en todos los paneles (volumen, RSI, MACD)
    anyY: true,
    // cruz de lectura: vertical en todos los paneles, horizontal y precio en el eje
    guide: ({ x: gx, y: gy, i }) => {
      const bar = bars[i]
      const up = Number(bar.close) >= Number(bar.open)
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
            {decimal(bar.close)}
          </text>
        </g>
      )
    },
  })
  const nav = useChartNav(svgRef, {
    vp,
    width,
    height,
    plotW,
    top: top.price,
    bottom: top.price + h.price,
    measure,
    ruler,
  })

  // Las velas (o la línea) se arman una vez por tramo dibujado, ancho y modo: ni el cursor ni el
  // arrastre las tocan (el arrastre las mueve con la transformación de la capa).
  const candles = useMemo(() => {
    if (mode === 'Línea') return null
    const out = []
    for (let i = a; i < b; i++) {
      const bar = bars[i]
      const color = Number(bar.close) >= Number(bar.open) ? 'var(--chart-up)' : 'var(--chart-down)'
      const yo = yP(Number(bar.open))
      const yc = yP(Number(bar.close))
      const cx = x(i)
      out.push(
        <g key={bar.start} data-candle={bar.start}>
          <line
            x1={cx}
            x2={cx}
            y1={yP(Number(bar.high))}
            y2={yP(Number(bar.low))}
            stroke={color}
            vectorEffect={THIN}
          />
          <rect
            x={cx - body / 2}
            y={Math.min(yo, yc)}
            width={body}
            height={Math.max(1, Math.abs(yc - yo))}
            fill={color}
          />
        </g>,
      )
    }
    return out
  }, [mode, bars, a, b, x, yP, body])
  const linePoints = useMemo(() => {
    const out: ([number, number] | null)[] = []
    for (let i = a; i < b; i++) out.push(Number.isFinite(closes[i]) ? [x(i), yP(closes[i])] : null)
    return out
  }, [closes, a, b, x, yP])
  const gridX = useMemo(() => timeTicks(base, n, 5, [a, b]).map((i) => x(i)), [base, n, a, b, x])

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
            rangeLabel={range || `${count} velas`}
            from={va}
            to={vb - 1}
          />
          <BarStats store={measure.store} bars={bars} last={vb - 1} />
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
              vp.setRange(RANGES.find((r) => r.label === v)!.bars)
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
          aria-label={`${instrument}, ${count} velas de 1 hora desde ${first ? dateUtc(first.start) : ''}. Arrastrá para moverte en el tiempo y usá la rueda para acercar o alejar; con el foco, flechas para desplazar, + y − para el zoom y Shift + flechas para leer cada vela.`}
          className={cn(
            'block w-full touch-none select-none focus-visible:outline-2 focus-visible:outline-ring',
            ruler ? 'cursor-crosshair' : 'cursor-grab active:cursor-grabbing',
          )}
          {...nav.handlers}
          onKeyDown={nav.onKeyDown}
        >
          <defs>
            <clipPath id={clipPrice}>
              <rect x={0} y={top.price} width={plotW} height={h.price} />
            </clipPath>
            <clipPath id={clipSub}>
              <rect x={0} y={top.volume} width={plotW} height={top.macd + h.macd - top.volume} />
            </clipPath>
          </defs>

          {/* ejes: siguen al encuadre en cada cuadro (son pocos textos) */}
          <ViewportLive vp={vp}>
            {(m) => {
              const { ticks, digits: td } = niceTicks(m.lo, m.hi)
              return (
                <>
                  {ticks.map((t) => (
                    <g key={t}>
                      <line x1={0} x2={plotW} y1={m.y(t)} y2={m.y(t)} stroke="var(--chart-grid)" />
                      <text
                        data-axis-tick
                        x={width - 4}
                        y={m.y(t) + 3}
                        textAnchor="end"
                        fontSize={10}
                        fill="var(--muted-foreground)"
                      >
                        {decimal(t.toFixed(td))}
                      </text>
                    </g>
                  ))}
                  {timeTicks(m, n).map((i, _, ticks) => {
                    const tx = m.x(i)
                    // cerca de los bordes no entra el rótulo entero: se omite (no se pisa con el vecino)
                    if (tx < 22 || tx > plotW - 22) return null
                    return (
                      <text
                        key={i}
                        data-time-tick={bars[i].start}
                        x={tx}
                        y={height - 4}
                        textAnchor="middle"
                        fontSize={10}
                        fill="var(--muted-foreground)"
                      >
                        {timeTickLabel(
                          bars[i].start,
                          ticks.length > 1 ? bars[i - (ticks[1] - ticks[0])]?.start : undefined,
                        )}
                      </text>
                    )
                  })}
                </>
              )
            }}
          </ViewportLive>

          {/* precio */}
          <g clipPath={`url(#${clipPrice})`}>
            <g ref={priceLayer} data-price-layer>
              {mode === 'Línea' ? (
                <LineArea
                  points={linePoints}
                  top={top.price}
                  bottom={top.price + h.price}
                  gridX={gridX}
                  trend={trend}
                />
              ) : (
                candles
              )}
            </g>
          </g>

          {/* RSI: banda y niveles fijos */}
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
          <line x1={0} x2={plotW} y1={yM(0)} y2={yM(0)} stroke="var(--chart-grid)" />

          {/* volumen, RSI y MACD: se mueven en el tiempo con el precio */}
          <g clipPath={`url(#${clipSub})`}>
            <g ref={xLayer}>
              {bars.slice(a, b).map((bar, k) => {
                const vh = (Number(bar.volume) / maxVol) * h.volume
                return (
                  <rect
                    key={bar.start}
                    x={x(a + k) - body / 2}
                    y={top.volume + h.volume - Math.min(vh, h.volume)}
                    width={body}
                    height={Math.min(vh, h.volume)}
                    fill={
                      Number(bar.close) >= Number(bar.open)
                        ? 'var(--chart-up)'
                        : 'var(--chart-down)'
                    }
                    opacity={0.45}
                  />
                )
              })}
              <path
                d={path('rsi14', yR)}
                fill="none"
                stroke="var(--chart-accent)"
                strokeWidth={1.5}
                vectorEffect={THIN}
              />
              {bars.slice(a, b).map((bar, k) => {
                const v = num(bar.histogram)
                if (v === null) return null
                return (
                  <rect
                    key={bar.start}
                    x={x(a + k) - body / 2}
                    y={Math.min(yM(v), yM(0))}
                    width={body}
                    height={Math.max(0.75, Math.abs(yM(v) - yM(0)))}
                    fill={v >= 0 ? 'var(--chart-up)' : 'var(--chart-down)'}
                    opacity={0.5}
                  />
                )
              })}
              <path
                d={path('macd', yM)}
                fill="none"
                stroke="var(--chart-accent)"
                strokeWidth={1.4}
                vectorEffect={THIN}
              />
              <path
                d={path('signal', yM)}
                fill="none"
                stroke="var(--chart-signal)"
                strokeWidth={1.4}
                strokeDasharray="4 3"
                vectorEffect={THIN}
              />
            </g>
          </g>
          <PaneLabel y={top.volume} text="Volumen" />
          <PaneLabel y={top.rsi} text="RSI 14" />
          <PaneLabel y={top.macd} text="MACD 12 · 26 · 9" />

          <ChartFollower store={measure.store} plot={measure.plot} />
        </svg>
        <ChartNavBar
          vp={vp}
          ruler={ruler}
          onRuler={setRuler}
          right={AXIS_W}
          bottom={top.price + h.price}
        />
      </div>
    </div>
  )
}

/** Apertura, máximo, mínimo e indicadores de la vela bajo el cursor (o de la última). */
function BarStats({
  store,
  bars,
  last,
}: {
  store: PointerStore
  bars: PriceBar[]
  /** última vela visible: la que se lee sin cursor */
  last: number
}) {
  const hover = usePointer(store, (s) => s.hover)
  const active = bars[hover ?? last]
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
