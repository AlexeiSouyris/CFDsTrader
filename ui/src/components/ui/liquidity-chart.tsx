/**
 * Gráfico de liquidez en vivo (#231, spec §5.4.1): velas de 15m, 1h o 1d con los niveles de
 * liquidez, los FVG y las velas de desplazamiento que manda el motor.
 *
 * Sigue el estilo de `trading-chart.tsx` (SVG propio, sin librería de gráficos). **No calcula
 * nada:** ni niveles, ni estados, ni zonas. Solo los ubica en pantalla; `Number()` se usa para la
 * posición y los textos se muestran tal cual llegan.
 *
 * Lleva la herramienta de medición (#281, `chart-measure.tsx`): una medición visual del usuario
 * sobre la escala del eje Y, no lógica de trading. Se ve con velas o en modo línea (#377), y la
 * cabecera muestra el cambio del rango, de la vela bajo el cursor o de la medición.
 *
 * Navegable (#401, `chart-nav.tsx`): arrastrar desplaza en el tiempo, la rueda hace zoom anclado
 * en el cursor y, al llegar al borde izquierdo, pide velas más viejas (`loadOlder`, que lee el
 * gateway con `before`). Solo se suman velas: los niveles, los FVG y las marcas de
 * desplazamiento son los de la vista actual.
 */
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react'

import {
  CHART_MODES,
  LABEL_GAP,
  placeLabels,
  rangeTrend,
  useChartMeasure,
  type ChartMode,
} from '@/components/ui/chart-measure'
import { ChartFollower } from '@/components/ui/chart-follower'
import { ChartNavBar, ViewportLive } from '@/components/ui/chart-nav-bar'
import { matchRange, useChartNav, useViewport, type HistoryState } from '@/components/ui/chart-nav'
import { ChartReadout } from '@/components/ui/chart-readout'
import { LineArea } from '@/components/ui/line-area'
import { Segmented } from '@/components/ui/segmented'
import { niceTicks, renderWindow, timeTicks, visibleRange } from '@/lib/chart-viewport'
import { dateTimeUtc, decimal, timeTickLabel } from '@/lib/format'
import { fvgKey, levelKey, levelName, levelShort, STATE_LABEL } from '@/lib/liquidity-labels'
import { useElementWidth } from '@/lib/use-element-width'
import { cn } from '@/lib/utils'
import { mergeBars, type ChartBar } from '@/lib/liquidity-bars'
import type {
  FvgEvent,
  Imbalance,
  LevelState,
  LiquidityLevel,
  ViewTimeframe,
} from '@/lib/liquidity-contracts'

type Range = { label: string; bars: number }
/** Rangos de cada timeframe (en velas); el segundo es el que se abre. */
const RANGES: Record<ViewTimeframe, Range[]> = {
  '15m': [
    { label: '1 día', bars: 96 },
    { label: '2 días', bars: 192 },
    { label: 'Todo', bars: Infinity },
  ],
  '1h': [
    { label: '3 días', bars: 72 },
    { label: '1 semana', bars: 120 },
    { label: 'Todo', bars: Infinity },
  ],
  '1d': [
    { label: '3 meses', bars: 63 },
    { label: '6 meses', bars: 126 },
    { label: 'Todo', bars: Infinity },
  ],
}
const TF_LABEL: Record<ViewTimeframe, { short: string; long: [string, string] }> = {
  '15m': { short: '15 m', long: ['vela de 15 minutos', 'velas de 15 minutos'] },
  '1h': { short: '1 h', long: ['vela de 1 hora', 'velas de 1 hora'] },
  '1d': { short: '1 d', long: ['vela diaria', 'velas diarias'] },
}
const FILTERS = ['Todos', 'Activos'] as const

const AXIS_W = 70
/** caja de cada rótulo del eje: de `y - LABEL_UP` a `y + LABEL_DOWN` */
const LABEL_UP = 6
const LABEL_DOWN = 5
/** trazos que no cambian de grosor mientras la capa se estira durante el zoom (#401) */
const THIN = 'non-scaling-stroke'

const STATE_STYLE: Record<LevelState, { stroke: string; dash?: string; opacity: number }> = {
  intact: { stroke: 'var(--chart-accent)', opacity: 0.95 },
  swept: { stroke: 'var(--chart-signal)', dash: '5 3', opacity: 0.95 },
  broken: { stroke: 'var(--muted-foreground)', dash: '1.5 3', opacity: 0.7 },
}

export function LiquidityChart({
  instrument,
  timeframe = '15m',
  bars,
  levels,
  fvgs,
  imbalance,
  signals = [],
  loadOlder,
  height = 440,
  className,
}: {
  instrument: string
  timeframe?: ViewTimeframe
  /** velas con su inicio y su cierre (`end`), tal como llegan */
  bars: ChartBar[]
  levels: LiquidityLevel[]
  fvgs: FvgEvent[]
  /** `imbalance.v1` por vela: la marca de desplazamiento la trae el motor */
  imbalance: Imbalance[]
  /** señales del motor (cierre de la vela de la señal y dirección), para marcarlas */
  signals?: { at: string; side: 'long' | 'short'; label: string }[]
  /**
   * Velas cerradas antes de `before` (#401): se piden al gateway al desplazarse hasta el borde
   * izquierdo. Sin esto, el gráfico muestra solo las velas que recibe.
   */
  loadOlder?: (before: string) => Promise<ChartBar[]>
  height?: number
  className?: string
}) {
  const ranges = RANGES[timeframe]
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>('Todos')
  const [mode, setMode] = useState<ChartMode>('Velas')
  const [ruler, setRuler] = useState(false)
  const [boxRef, width] = useElementWidth<HTMLDivElement>(720, 300)
  const svgRef = useRef<SVGSVGElement>(null)
  const clipPrice = useId()
  const clipMarks = useId()

  // Historia (#401): las velas más viejas que se van pidiendo se suman a las que llegan.
  const [older, setOlder] = useState<ChartBar[]>([])
  const [history, setHistory] = useState<HistoryState>('idle')
  const all = useMemo(() => (older.length ? mergeBars(older, bars) : bars), [older, bars])
  const live = useRef({ all, history, busy: false, alive: true })
  useLayoutEffect(() => {
    live.current.all = all
    live.current.history = history
  })
  useEffect(() => {
    const l = live.current
    l.alive = true
    return () => {
      l.alive = false
    }
  }, [])
  const loadMore = (retry = false) => {
    const l = live.current
    if (!loadOlder || l.busy || l.all.length === 0) return
    if (l.history === 'done' || (l.history === 'error' && !retry)) return
    l.busy = true
    setHistory('loading')
    const seen = l.all
    loadOlder(seen[0].start).then(
      (page) => {
        l.busy = false
        if (!live.current.alive) return
        const first = Date.parse(live.current.all[0].start)
        const fresh = page.filter((b) => Date.parse(b.start) < first)
        if (fresh.length === 0) return setHistory('done')
        // se guardan también las que se veían: si la ventana de las últimas se corre, no queda hueco
        setOlder((o) => mergeBars(mergeBars(o, seen), fresh))
        setHistory('idle')
      },
      () => {
        l.busy = false
        if (live.current.alive) setHistory('error')
      },
    )
  }

  const n = all.length
  const plotW = width - AXIS_W
  const top = 6
  const plotH = height - top - 22
  const starts = useMemo(() => all.map((b) => b.start), [all])
  const closeTexts = useMemo(() => all.map((b) => b.close), [all])
  const closes = useMemo(() => all.map((b) => Number(b.close)), [all])
  const lows = useMemo(() => all.map((b) => Number(b.low)), [all])
  const highs = useMemo(() => all.map((b) => Number(b.high)), [all])

  const vp = useViewport({
    starts,
    lows,
    highs,
    initial: ranges[1].bars,
    plotW,
    top,
    h: plotH,
    onNearStart: () => loadMore(),
  })
  const { base, baseMap, priceLayer, xLayer } = vp
  const [a, b] = renderWindow(base, n)
  const [va, vb] = visibleRange(base, n)
  const count = vb - va
  const slot = baseMap.slot
  const body = Math.max(1.5, Math.min(slot * 0.62, 10))
  const yP = baseMap.y
  const range = matchRange(base, n, ranges)

  // Ubica una hora (UTC) en el eje de índices: cuántas velas ya cerraron a esa hora. Cuenta
  // velas, no minutos, así que los huecos (noches, fines de semana) no dejan espacio.
  const ends = useMemo(() => all.map((bar) => Date.parse(bar.end)), [all])
  const idxAt = (iso: string | null) => {
    if (iso === null) return n
    const t = Date.parse(iso)
    let lo = 0
    let hi = ends.length
    while (lo < hi) {
      const mid = (lo + hi) >> 1
      if (ends[mid] <= t) lo = mid + 1
      else hi = mid
    }
    return lo
  }
  /** borde derecho de las velas que cerraron a esa hora, en lo dibujado */
  const xAt = (iso: string | null) => (idxAt(iso) - base.start) * slot
  const lastClose = n ? new Date(ends[n - 1]) : null

  // Activo = intacto y sin vencer (`expired_at` de liquidity_level.v1). Los vencidos se dibujan
  // atenuados hasta la vela en que vencieron.
  const expired = (l: LiquidityLevel) => l.state === 'intact' && l.expired_at != null
  const levelEnd = (l: LiquidityLevel) => l.state_changed_at ?? l.expired_at ?? null
  const shownLevels = levels.filter(
    (l) =>
      (filter === 'Todos' || (l.state === 'intact' && !expired(l))) &&
      lastClose !== null &&
      Date.parse(l.exists_from) <= lastClose.getTime() &&
      idxAt(levelEnd(l)) > 0,
  )
  const axisLevels = shownLevels.filter((l) => l.state === 'intact' && !expired(l))
  const fvgEnd = (f: FvgEvent) => f.fill?.at ?? f.expiry?.at ?? null
  const shownFvgs = fvgs.filter((f) => idxAt(fvgEnd(f)) > 0)
  const last = all[n - 1]
  const digits = (last?.close.split('.')[1] ?? '').length
  const trend = rangeTrend(closes.slice(va, vb), digits)
  const measure = useChartMeasure(svgRef, {
    width,
    height,
    plotW,
    top,
    bottom: top + plotH,
    slot,
    starts,
    closes,
    yOf: yP,
    digits,
    mode,
    frame: vp.frame,
    trend,
  })
  const nav = useChartNav(svgRef, {
    vp,
    width,
    height,
    plotW,
    top,
    bottom: top + plotH,
    measure,
    ruler,
  })

  // Las velas (o la línea) se arman una vez por tramo dibujado, ancho y modo: ni el cursor ni el
  // arrastre las tocan (el arrastre las mueve con la transformación de la capa).
  const candles = useMemo(() => {
    if (mode === 'Línea') return null
    const out = []
    for (let i = a; i < b; i++) {
      const bar = all[i]
      const color = Number(bar.close) >= Number(bar.open) ? 'var(--chart-up)' : 'var(--chart-down)'
      const yo = yP(Number(bar.open))
      const yc = yP(Number(bar.close))
      const x = baseMap.x(i)
      out.push(
        <g key={bar.start} data-candle={bar.start}>
          <line
            x1={x}
            x2={x}
            y1={yP(Number(bar.high))}
            y2={yP(Number(bar.low))}
            stroke={color}
            vectorEffect={THIN}
          />
          <rect
            x={x - body / 2}
            y={Math.min(yo, yc)}
            width={body}
            height={Math.max(1, Math.abs(yc - yo))}
            fill={color}
          />
        </g>,
      )
    }
    return out
  }, [mode, all, a, b, baseMap, yP, body])
  const linePoints = useMemo(() => {
    const out: ([number, number] | null)[] = []
    for (let i = a; i < b; i++)
      out.push(Number.isFinite(closes[i]) ? [baseMap.x(i), yP(closes[i])] : null)
    return out
  }, [closes, a, b, baseMap, yP])
  const gridX = useMemo(
    () => timeTicks(base, n, 4, [a, b]).map((i) => baseMap.x(i)),
    [base, n, a, b, baseMap],
  )

  return (
    <div className={cn('w-full', className)}>
      <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase">
            {instrument} · {TF_LABEL[timeframe].short} · liquidez
          </p>
          <ChartReadout
            store={measure.store}
            starts={starts}
            closes={closeTexts}
            rangeLabel={range || `${count} ${TF_LABEL[timeframe].long[count === 1 ? 0 : 1]}`}
            from={va}
            to={vb - 1}
          />
          {lastClose && (
            <p className="mt-1 text-[11px] text-muted-foreground">
              Última vela cerrada: {dateTimeUtc(lastClose.toISOString())}
            </p>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          <Segmented
            label="Tipo de gráfico"
            options={CHART_MODES}
            value={mode}
            onChange={(v) => setMode(v as ChartMode)}
          />
          <Segmented
            label="Niveles"
            options={FILTERS}
            value={filter}
            onChange={(v) => setFilter(v as typeof filter)}
          />
          <Segmented
            label="Rango"
            options={ranges.map((r) => r.label)}
            value={range}
            onChange={(v) => vp.setRange((ranges.find((r) => r.label === v) ?? ranges[1]).bars)}
          />
        </div>
      </div>

      <div ref={boxRef} className="chart-enter relative w-full">
        <svg
          ref={svgRef}
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          tabIndex={0}
          aria-label={`${instrument}, ${count} ${TF_LABEL[timeframe].long[count === 1 ? 0 : 1]} con ${shownLevels.length} niveles de liquidez y ${shownFvgs.length} FVG. Arrastrá para moverte en el tiempo y usá la rueda para acercar o alejar; con el foco, flechas para desplazar y + y − para el zoom.`}
          className={cn(
            'block w-full touch-none select-none focus-visible:outline-2 focus-visible:outline-ring',
            ruler ? 'cursor-crosshair' : 'cursor-grab active:cursor-grabbing',
          )}
          {...nav.handlers}
          onKeyDown={nav.onKeyDown}
        >
          <defs>
            <clipPath id={clipPrice}>
              <rect x={0} y={top} width={plotW} height={plotH} />
            </clipPath>
            <clipPath id={clipMarks}>
              <rect x={0} y={0} width={plotW} height={height} />
            </clipPath>
          </defs>

          {/* eje de precio, rótulos de los niveles y fechas: siguen al encuadre en cada cuadro */}
          <ViewportLive vp={vp}>
            {(m) => {
              // Rótulos de los niveles intactos en el eje: los de fuera de escala van al borde con
              // una flecha. Se separan para que no se pisen y nunca salen del panel (geometría).
              const labels = placeLabels(
                axisLevels.map((l) => {
                  const v = Number(l.price)
                  const arrow = v > m.hi ? ' ↑' : v < m.lo ? ' ↓' : ''
                  return { l, arrow, y: m.y(v) }
                }),
                top + LABEL_UP,
                top + plotH - LABEL_DOWN,
              )
              const { ticks, digits: td } = niceTicks(m.lo, m.hi)
              return (
                <>
                  {ticks.map((t) => {
                    const y = m.y(t)
                    // los precios del eje que chocarían con un rótulo se ocultan (queda la grilla)
                    const hidden = labels.some((lb) => Math.abs(lb.y - y) < LABEL_GAP)
                    return (
                      <g key={t}>
                        <line x1={0} x2={plotW} y1={y} y2={y} stroke="var(--chart-grid)" />
                        {!hidden && (
                          <text
                            data-axis-tick
                            x={width - 4}
                            y={y + 3}
                            textAnchor="end"
                            fontSize={10}
                            fill="var(--muted-foreground)"
                          >
                            {decimal(t.toFixed(td))}
                          </text>
                        )}
                      </g>
                    )
                  })}
                  {labels.map(({ l, arrow, y }) => (
                    <g key={levelKey(l)} data-level-label={y.toFixed(1)}>
                      <rect
                        x={plotW + 1}
                        y={y - LABEL_UP}
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
                  {timeTicks(m, n, 4).map((i, _, ticks) => {
                    const tx = m.x(i)
                    // cerca de los bordes no entra el rótulo entero: se omite (no se pisa con el vecino)
                    if (tx < 22 || tx > plotW - 22) return null
                    return (
                      <text
                        key={i}
                        data-time-tick={all[i].start}
                        x={tx}
                        y={height - 4}
                        textAnchor="middle"
                        fontSize={10}
                        fill="var(--muted-foreground)"
                      >
                        {timeTickLabel(
                          all[i].start,
                          ticks.length > 1 ? all[i - (ticks[1] - ticks[0])]?.start : undefined,
                        )}
                      </text>
                    )
                  })}
                </>
              )
            }}
          </ViewportLive>

          <g clipPath={`url(#${clipPrice})`}>
            <g ref={priceLayer} data-price-layer>
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
                    vectorEffect={THIN}
                  />
                )
              })}

              {/* velas, o la línea de cierre con su área */}
              {mode === 'Línea' ? (
                <LineArea
                  points={linePoints}
                  top={top}
                  bottom={top + plotH}
                  gridX={gridX}
                  trend={trend}
                />
              ) : (
                candles
              )}

              {/* niveles: desde que existen hasta la vela que cambió su estado */}
              {shownLevels.map((l) => {
                const v = Number(l.price)
                const s = STATE_STYLE[l.state]
                const x0 = xAt(l.exists_from)
                const x1 = xAt(levelEnd(l))
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
                      vectorEffect={THIN}
                    />
                    {l.state !== 'intact' && (
                      <circle cx={x1} cy={y} r={2.6} fill={s.stroke}>
                        <title>{`${levelName(l)}: ${STATE_LABEL[l.state].toLowerCase()}`}</title>
                      </circle>
                    )}
                  </g>
                )
              })}
            </g>
          </g>

          <g clipPath={`url(#${clipMarks})`}>
            <g ref={xLayer}>
              {/* velas de desplazamiento (marca del motor) */}
              {imbalance.map((d) => {
                if (d.displacement === null) return null
                const k = idxAt(d.at)
                if (k <= a || k > b) return null
                const x = xAt(d.at) - slot / 2
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

              {/* señales del motor: una línea vertical en la vela de la señal y su rótulo */}
              {signals.map((sg) => {
                const k = idxAt(sg.at)
                if (k <= 0) return null
                const x = xAt(sg.at) - slot / 2
                const color = sg.side === 'long' ? 'var(--chart-up)' : 'var(--chart-down)'
                return (
                  <g key={`${sg.at}-${sg.side}`} data-signal={sg.side}>
                    <line
                      x1={x}
                      x2={x}
                      y1={top + 14}
                      y2={top + plotH}
                      stroke={color}
                      strokeOpacity={0.35}
                      strokeDasharray="2 3"
                      vectorEffect={THIN}
                    />
                    <rect x={x - 3} y={top + 2} width={6} height={6} rx={1.5} fill={color}>
                      <title>{`Señal ${sg.side === 'long' ? 'larga' : 'corta'}: ${sg.label}`}</title>
                    </rect>
                  </g>
                )
              })}
            </g>
          </g>

          <ChartFollower store={measure.store} plot={measure.plot} />
        </svg>
        <ChartNavBar
          vp={vp}
          ruler={ruler}
          onRuler={setRuler}
          right={AXIS_W}
          bottom={top + plotH}
          history={history}
          onRetry={() => loadMore(true)}
        />
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
        <li>
          Arrastrá para moverte en el tiempo · rueda: zoom · eje de precio: arrastrá para estirarlo,
          doble clic vuelve a automático
        </li>
        <li>Shift + arrastrar (o la regla) mide; un clic o Esc borra la medición</li>
        {signals.length > 0 && (
          <li className="flex items-center gap-1.5">
            <span className="inline-block size-2 rounded-[2px] bg-up" aria-hidden />
            <span className="inline-block size-2 rounded-[2px] bg-down" aria-hidden />
            señal del motor
          </li>
        )}
      </ul>
    </div>
  )
}
