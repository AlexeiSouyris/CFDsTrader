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
 * gateway con `before`). Se suman las velas y sus herramientas de análisis (#511); los niveles,
 * los FVG y las marcas de desplazamiento son los de la vista actual.
 *
 * Herramientas de análisis (#501, `chart-tools.tsx`): las capas del bloque `analysis` que manda el
 * motor (Fibonacci con su golden pocket, EMA diaria, spring/upthrust, fondos, cruces de medias,
 * puntos pivote y régimen del VIX), cada una activable desde el menú "Herramientas". Tampoco se
 * calculan acá: se ubican en la vela que dice el motor.
 *
 * Modo compacto (#545, prop `compact`): sin cabecera ni leyenda, para las grillas del
 * Multigráfico. Los controles (niveles, rango, herramientas y, si no se maneja desde afuera, el
 * tipo de gráfico) se dibujan en `controlsTarget` —la barra de arriba del Multigráfico— solo
 * cuando el gráfico es el activo. Sin `compact`, el gráfico se ve como siempre.
 *
 * Dibujos del usuario (#543, prop `drawTool`, `chart-drawings.tsx`): línea horizontal, línea de
 * tendencia y rectángulo, anclados a hora UTC y precio, guardados en el navegador por instrumento.
 * Son solo visuales. Sin `drawTool`, el gráfico no muestra ni toma dibujos.
 *
 * Cursor sincronizado (#544, prop `syncCursor`, `sync-cursor.tsx`): avisa la hora UTC de la vela
 * bajo el cursor y marca con una línea vertical la del cursor de otro gráfico. Sin zoom compartido.
 */
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Trash2 } from 'lucide-react'

import {
  CHART_MODES,
  LABEL_GAP,
  placeLabels,
  rangeTrend,
  useChartMeasure,
  type ChartMode,
} from '@/components/ui/chart-measure'
import { DrawingInput, DrawingShapes } from '@/components/ui/chart-drawings'
import { ChartFollower } from '@/components/ui/chart-follower'
import { ChartNavBar, ViewportLive } from '@/components/ui/chart-nav-bar'
import { matchRange, useChartNav, useViewport, type HistoryState } from '@/components/ui/chart-nav'
import { ChartReadout } from '@/components/ui/chart-readout'
import { SyncCursor } from '@/components/ui/sync-cursor'
import { ChartToolsMenu } from '@/components/ui/chart-tools'
import { LineArea } from '@/components/ui/line-area'
import { Segmented } from '@/components/ui/segmented'
import { Term } from '@/components/ui/term'
import type { ChartAnalysis, VixRegime } from '@/lib/analysis-contracts'
import {
  activeAt,
  analysisLayers,
  mergeLayers,
  type AnalysisLayers,
  MARKER_GLYPH,
  MARKER_NAME,
  MARKER_TOOL,
  VIX_LABEL,
} from '@/lib/analysis-layers'
import {
  drawingStore,
  posAt,
  timeAt,
  useDrawings,
  type Anchor,
  type Drawing,
  type DrawTool,
} from '@/lib/chart-drawings'
import type { CursorSync } from '@/lib/cursor-sync'
import { niceTicks, renderWindow, timeTicks, visibleRange } from '@/lib/chart-viewport'
import { dateTimeUtc, decimal, timeTickLabel } from '@/lib/format'
import { fvgKey, levelKey, levelName, levelShort, STATE_LABEL } from '@/lib/liquidity-labels'
import { useChartTools } from '@/lib/use-chart-tools'
import { useCoarsePointer } from '@/lib/use-coarse-pointer'
import { useElementWidth } from '@/lib/use-element-width'
import { cn } from '@/lib/utils'
import { mergeBars, type ChartBar, type ChartTimeframe } from '@/lib/liquidity-bars'
import type { OlderPage } from '@/lib/liquidity-api'
import type { FvgEvent, Imbalance, LevelState, LiquidityLevel } from '@/lib/liquidity-contracts'

type Range = { label: string; bars: number }
/** Rangos de cada timeframe (en velas); el segundo es el que se abre. */
const RANGES: Record<ChartTimeframe, Range[]> = {
  '1m': [
    { label: '2 horas', bars: 120 },
    { label: '6 horas', bars: 360 },
    { label: 'Todo', bars: Infinity },
  ],
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
const TF_LABEL: Record<ChartTimeframe, { short: string; long: [string, string] }> = {
  '1m': { short: '1 m', long: ['vela de 1 minuto', 'velas de 1 minuto'] },
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

/** Color de la píldora del VIX: el régimen, no una dirección del precio. */
const VIX_STYLE: Record<VixRegime, string> = {
  calm: 'border-border text-muted-foreground',
  normal: 'border-border text-foreground',
  elevated: 'border-[var(--chart-signal)] text-[var(--chart-signal)]',
  stress: 'border-[var(--chart-down)] text-[var(--chart-down)]',
}

/** "28.440000" → "28.44": solo quita ceros de más del texto, no redondea. */
const trimZeros = (t: string) => (t.includes('.') ? t.replace(/\.?0+$/, '') : t)

/** Un rótulo del eje de precio: un nivel de liquidez o una línea de una herramienta. */
type AxisLabel = { key: string; text: string; title: string; color: string; y: number }

const NO_DRAWINGS: readonly Drawing[] = []

const STATE_STYLE: Record<LevelState, { stroke: string; dash?: string; opacity: number }> = {
  intact: { stroke: 'var(--chart-accent)', opacity: 0.95 },
  swept: { stroke: 'var(--chart-signal)', dash: '5 3', opacity: 0.95 },
  broken: { stroke: 'var(--muted-foreground)', dash: '1.5 3', opacity: 0.7 },
}

export function LiquidityChart({
  instrument,
  timeframe = '15m',
  caption = 'liquidez',
  liquidity = true,
  bars,
  levels,
  fvgs,
  imbalance,
  signals = [],
  analysis,
  loadOlder,
  height = 440,
  ruler: rulerProp,
  onRulerChange,
  mode: modeProp,
  onModeChange,
  compact = false,
  controlsTarget,
  drawTool,
  onDrawToolChange,
  syncCursor,
  className,
}: {
  instrument: string
  /** `1m` solo para las velas de IBKR (#556), que no traen capas de liquidez */
  timeframe?: ChartTimeframe
  /** qué se ve, en la cabecera (por defecto, "liquidez"; las velas de IBKR dicen su fuente) */
  caption?: string
  /**
   * Si las velas traen capas de liquidez (por defecto, sí). En falso (las de IBKR, #556) no se
   * muestran el filtro de niveles ni la leyenda de niveles y FVG, que no hay.
   */
  liquidity?: boolean
  /** velas con su inicio y su cierre (`end`), tal como llegan */
  bars: ChartBar[]
  levels: LiquidityLevel[]
  fvgs: FvgEvent[]
  /** `imbalance.v1` por vela: la marca de desplazamiento la trae el motor */
  imbalance: Imbalance[]
  /** señales del motor (cierre de la vela de la señal y dirección), para marcarlas */
  signals?: { at: string; side: 'long' | 'short'; label: string }[]
  /**
   * Bloque `analysis` del motor (#501), con índices de `bars` (las velas que llegan, sin las más
   * viejas que se suman al desplazarse, que traen el suyo). Sin esto, el menú de herramientas
   * avisa que no hay.
   */
  analysis?: ChartAnalysis
  /**
   * Velas cerradas antes de `before` (#401): se piden al gateway al desplazarse hasta el borde
   * izquierdo, con su bloque `analysis` si lo hay (#511). Sin esto, el gráfico muestra solo las
   * velas que recibe.
   */
  loadOlder?: (before: string) => Promise<ChartBar[] | OlderPage>
  height?: number
  /**
   * Regla y tipo de gráfico manejados desde afuera (la barra de dibujo del Multigráfico, #540).
   * Sin esto, el gráfico los maneja solo.
   */
  ruler?: boolean
  onRulerChange?: (on: boolean) => void
  mode?: ChartMode
  onModeChange?: (m: ChartMode) => void
  /**
   * Modo compacto (#545): solo el gráfico y su barra de navegación, sin cabecera ni leyenda. La
   * cabecera mínima la pone quien lo usa.
   */
  compact?: boolean
  /**
   * En modo compacto, dónde van los controles del gráfico (la barra del gráfico activo). Sin
   * esto (un gráfico que no es el activo), no se dibujan.
   */
  controlsTarget?: HTMLElement | null
  /**
   * Dibujos del usuario (#543): la herramienta elegida (`none` = navegar y seleccionar). Con
   * esto, el gráfico muestra los dibujos guardados del instrumento y deja dibujar; al terminar un
   * dibujo avisa con `onDrawToolChange('none')`. Sin esto, no hay capa de dibujos.
   */
  drawTool?: DrawTool
  onDrawToolChange?: (t: DrawTool) => void
  /**
   * Cursor sincronizado (#544): con esto, el gráfico avisa la hora de la vela bajo su cursor y
   * marca la del cursor de los otros gráficos que comparten el mismo `CursorSync`.
   */
  syncCursor?: CursorSync
  className?: string
}) {
  const ranges = RANGES[timeframe]
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>('Todos')
  const [modeState, setModeState] = useState<ChartMode>('Velas')
  const [rulerState, setRulerState] = useState(false)
  const mode = modeProp ?? modeState
  const ruler = rulerProp ?? rulerState
  const setMode = (m: ChartMode) => {
    setModeState(m)
    onModeChange?.(m)
  }
  const setRuler = (on: boolean) => {
    setRulerState(on)
    onRulerChange?.(on)
  }
  const [boxRef, width] = useElementWidth<HTMLDivElement>(720, 300)
  const coarse = useCoarsePointer()
  const svgRef = useRef<SVGSVGElement>(null)
  const clipPrice = useId()
  const clipMarks = useId()

  // Historia (#401): las velas más viejas que se van pidiendo se suman a las que llegan.
  const [older, setOlder] = useState<ChartBar[]>([])
  const [history, setHistory] = useState<HistoryState>('idle')
  const all = useMemo(() => (older.length ? mergeBars(older, bars) : bars), [older, bars])
  // Herramientas de análisis (#501): índices del motor → cierres de vela, una vez por dato. Las
  // de las velas más viejas (#511) se suman antes de las de la vista.
  const [olderLayers, setOlderLayers] = useState<AnalysisLayers | null>(null)
  const current = useMemo(
    () => (analysis ? analysisLayers(analysis, bars) : null),
    [analysis, bars],
  )
  const layers = useMemo(
    () =>
      current && olderLayers && bars.length
        ? mergeLayers(olderLayers, current, bars[0].end)
        : (current ?? olderLayers),
    [current, olderLayers, bars],
  )
  const live = useRef({ all, history, layers, busy: false, alive: true })
  useLayoutEffect(() => {
    live.current.all = all
    live.current.history = history
    live.current.layers = layers
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
    const seenLayers = l.layers
    loadOlder(seen[0].start).then(
      (got) => {
        l.busy = false
        if (!live.current.alive) return
        const page = Array.isArray(got) ? { bars: got } : got
        const first = Date.parse(live.current.all[0].start)
        const fresh = page.bars.filter((b) => Date.parse(b.start) < first)
        if (fresh.length === 0) return setHistory('done')
        // se guardan también las que se veían: si la ventana de las últimas se corre, no queda hueco
        setOlder((o) => mergeBars(mergeBars(o, seen), fresh))
        const pageLayers = page.analysis ? analysisLayers(page.analysis, page.bars) : null
        setOlderLayers(
          pageLayers && seenLayers
            ? mergeLayers(pageLayers, seenLayers, seen[0].end)
            : (seenLayers ?? pageLayers),
        )
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

  // Dibujos del usuario (#543): cada punto es una hora UTC y un precio; se ubican en el eje de
  // índices con las velas que hay (también los de otro timeframe del mismo instrumento).
  const drawOn = drawTool !== undefined
  const saved = useDrawings(instrument)
  const drawings: readonly Drawing[] = drawOn && n > 0 ? saved : NO_DRAWINGS
  const [picked, setPicked] = useState<string | null>(null)
  const selected = drawings.some((d) => d.id === picked) ? picked : null
  const startMs = useMemo(() => all.map((bar) => Date.parse(bar.start)), [all])
  const anchorX = (p: Anchor, m: { start: number; slot: number }) =>
    (posAt(startMs, ends, Date.parse(p.t)) - m.start) * m.slot
  const focusChart = () => svgRef.current?.focus({ preventScroll: true })
  const removeSelected = () => {
    if (selected) drawingStore.remove(instrument, selected)
    setPicked(null)
  }

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
  const [tools, toggleTool] = useChartTools()
  const markers = layers ? layers.markers.filter((mk) => tools[MARKER_TOOL[mk.kind]]) : []
  const vix = tools.vix ? (layers?.vix ?? null) : null
  // Fibonacci: solo el último tramo confirmado (con decenas de tramos en la ventana, dibujarlos
  // todos tapa las velas), desde su confirmación hasta la última vela.
  const fibLegs = tools.fibonacci && layers?.fibonacci.length ? layers.fibonacci.slice(-1) : []
  /** centro de la vela que cerró a esa hora */
  const cx = (iso: string) => xAt(iso) - slot / 2
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
  // Cursor sincronizado (#544): la hora UTC (inicio) de la vela bajo el cursor, para los demás.
  const syncId = useId()
  useEffect(() => {
    if (!syncCursor) return
    let last: number | null | undefined
    const push = () => {
      const h = measure.store.get().hover
      const t = h === null ? null : (startMs[h] ?? null)
      if (t === last) return
      last = t
      syncCursor.set(syncId, t)
    }
    const unsub = measure.store.subscribe(push)
    return () => {
      unsub()
      syncCursor.set(syncId, null)
    }
  }, [syncCursor, measure.store, syncId, startMs])
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

  // Los controles del gráfico: en su cabecera o, en modo compacto, en la barra de afuera (#545).
  const controls = (
    <div className="flex flex-wrap gap-2" data-chart-controls={instrument}>
      {!(compact && modeProp !== undefined) && (
        <Segmented
          label="Tipo de gráfico"
          options={CHART_MODES}
          value={mode}
          onChange={(v) => setMode(v as ChartMode)}
        />
      )}
      {liquidity && (
        <Segmented
          label="Niveles"
          options={FILTERS}
          value={filter}
          onChange={(v) => setFilter(v as typeof filter)}
        />
      )}
      <Segmented
        label="Rango"
        options={ranges.map((r) => r.label)}
        value={range}
        onChange={(v) => vp.setRange((ranges.find((r) => r.label === v) ?? ranges[1]).bars)}
      />
      <ChartToolsMenu tools={tools} onToggle={toggleTool} available={layers !== null} />
    </div>
  )

  return (
    <div className={cn('w-full', className)} data-chart-compact={compact || undefined}>
      {compact ? (
        controlsTarget && createPortal(controls, controlsTarget)
      ) : (
        <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
          <div>
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
              <p className="text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase">
                {instrument} · {TF_LABEL[timeframe].short} · {caption}
              </p>
              {vix && (
                <span
                  data-vix={vix.regime}
                  className={cn(
                    'inline-flex h-5 items-center rounded-full border px-2 text-[10.5px] font-bold',
                    VIX_STYLE[vix.regime],
                  )}
                >
                  <Term id="vix">
                    VIX {decimal(trimZeros(vix.close))} · {VIX_LABEL[vix.regime]}
                  </Term>
                </span>
              )}
            </div>
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
          {controls}
        </div>
      )}

      <div ref={boxRef} className="chart-enter relative w-full">
        <svg
          ref={svgRef}
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          tabIndex={0}
          data-swipe-ignore
          aria-label={`${instrument}, ${count} ${TF_LABEL[timeframe].long[count === 1 ? 0 : 1]} con ${shownLevels.length} niveles de liquidez y ${shownFvgs.length} FVG. ${coarse ? 'Deslizá un dedo en horizontal para moverte en el tiempo y pellizcá con dos para acercar o alejar' : 'Arrastrá para moverte en el tiempo y usá la rueda para acercar o alejar'}; con el foco, flechas para desplazar y + y − para el zoom.`}
          className={cn(
            'block w-full touch-pan-y select-none focus-visible:outline-2 focus-visible:outline-ring',
            ruler ? 'cursor-crosshair' : 'cursor-grab active:cursor-grabbing',
          )}
          {...nav.handlers}
          onPointerDown={(e) => {
            // un clic fuera de los dibujos deja de seleccionar
            if (selected) setPicked(null)
            nav.handlers.onPointerDown(e)
          }}
          onKeyDown={(e) => {
            if (selected && (e.key === 'Delete' || e.key === 'Backspace')) {
              e.preventDefault()
              return removeSelected()
            }
            if (selected && e.key === 'Escape') setPicked(null)
            nav.onKeyDown(e)
          }}
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
              const items: AxisLabel[] = axisLevels.map((l) => {
                const v = Number(l.price)
                const arrow = v > m.hi ? ' ↑' : v < m.lo ? ' ↓' : ''
                return {
                  key: levelKey(l),
                  text: `${levelShort(l)}${arrow}`,
                  title: `${levelName(l)}: ${decimal(l.price)}`,
                  color: 'var(--chart-accent)',
                  y: m.y(v),
                }
              })
              // Herramientas (#501): las líneas del tramo y de los pivotes que valen en la última
              // vela visible, solo las que entran en la escala.
              const lastSeen = n ? ends[Math.max(0, Math.min(n, Math.ceil(m.end)) - 1)] : NaN
              const inScale = (v: number) => v >= m.lo && v <= m.hi
              const leg = fibLegs[0]
              for (const f of leg?.lines ?? []) {
                const v = Number(f.price)
                if (inScale(v))
                  items.push({
                    key: `fib-${f.kind}-${f.ratio}`,
                    text: `F ${decimal(f.ratio)}`,
                    title: `Fibonacci ${decimal(f.ratio)}: ${decimal(f.price)}`,
                    color: 'var(--chart-fib)',
                    y: m.y(v),
                  })
              }
              const piv = layers && tools.pivots ? activeAt(layers.pivots, lastSeen) : null
              for (const p of piv?.levels ?? []) {
                const v = Number(p.price)
                if (inScale(v))
                  items.push({
                    key: `pivot-${p.name}`,
                    text: p.name,
                    title: `Punto pivote ${p.name} (del día ${piv!.day}): ${decimal(p.price)}`,
                    color: 'var(--chart-pivot)',
                    y: m.y(v),
                  })
              }
              const labels = placeLabels(items, top + LABEL_UP, top + plotH - LABEL_DOWN)
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
                  {labels.map(({ key, text, title, color, y }) => (
                    <g key={key} data-level-label={y.toFixed(1)}>
                      <rect
                        x={plotW + 1}
                        y={y - LABEL_UP}
                        width={AXIS_W - 2}
                        height={11}
                        rx={3}
                        fill="var(--background)"
                        opacity={0.9}
                      />
                      <text x={plotW + 4} y={y + 3} fontSize={9} fontWeight={700} fill={color}>
                        <title>{title}</title>
                        {text}
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

              {/* Fibonacci (#501): el último tramo, desde su confirmación */}
              {fibLegs.map((f) => {
                const x0 = cx(f.from)
                const x1 = f.to ? cx(f.to) : xAt(null)
                if (x1 <= x0) return null
                const yl = yP(Number(f.pocket.low))
                const yh = yP(Number(f.pocket.high))
                return (
                  <g key={f.from} data-fib-leg={f.direction}>
                    <rect
                      data-golden-pocket
                      x={x0}
                      y={Math.min(yl, yh)}
                      width={x1 - x0}
                      height={Math.max(1, Math.abs(yl - yh))}
                      fill="var(--chart-fib)"
                      opacity={0.18}
                    >
                      <title>{`Golden pocket: ${decimal(f.pocket.low)} – ${decimal(f.pocket.high)}`}</title>
                    </rect>
                    <line
                      x1={cx(f.start.at)}
                      x2={cx(f.end.at)}
                      y1={yP(Number(f.start.price))}
                      y2={yP(Number(f.end.price))}
                      stroke="var(--chart-fib)"
                      strokeOpacity={0.45}
                      strokeDasharray="2 3"
                      vectorEffect={THIN}
                    />
                    {f.lines.map((l) => {
                      const y = yP(Number(l.price))
                      return (
                        <line
                          key={`${l.kind}-${l.ratio}`}
                          data-fib-ratio={l.ratio}
                          x1={x0}
                          x2={x1}
                          y1={y}
                          y2={y}
                          stroke="var(--chart-fib)"
                          strokeOpacity={l.kind === 'extension' ? 0.45 : 0.75}
                          strokeDasharray={l.kind === 'extension' ? '4 3' : undefined}
                          vectorEffect={THIN}
                        >
                          <title>{`Fibonacci ${decimal(l.ratio)}: ${decimal(l.price)}`}</title>
                        </line>
                      )
                    })}
                  </g>
                )
              })}

              {/* puntos pivote diarios (#501): cada día desde su primera vela */}
              {tools.pivots &&
                layers?.pivots.map((p) => {
                  const x0 = xAt(p.from) - slot
                  const x1 = p.to ? xAt(p.to) - slot : xAt(null)
                  if (x1 <= x0) return null
                  return (
                    <g key={p.day} data-pivots={p.day}>
                      {p.levels.map((l) => {
                        const y = yP(Number(l.price))
                        return (
                          <line
                            key={l.name}
                            x1={x0}
                            x2={x1}
                            y1={y}
                            y2={y}
                            stroke="var(--chart-pivot)"
                            strokeOpacity={l.name === 'P' ? 0.8 : 0.55}
                            strokeWidth={0.8}
                            strokeDasharray={l.name === 'P' ? undefined : '3 3'}
                            vectorEffect={THIN}
                          >
                            <title>{`Punto pivote ${l.name} (del día ${p.day}): ${decimal(l.price)}`}</title>
                          </line>
                        )
                      })}
                    </g>
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

              {/* EMA diaria (#501): el valor del último día terminado, en cada vela */}
              {tools.ema &&
                layers?.ema.map((run) => (
                  <polyline
                    key={run[0].at}
                    data-ema
                    points={run.map((p) => `${cx(p.at)},${yP(Number(p.price))}`).join(' ')}
                    fill="none"
                    stroke="var(--chart-ema)"
                    strokeWidth={1.4}
                    vectorEffect={THIN}
                  />
                ))}

              {/* velas que tocan el golden pocket: llena si cerró adentro */}
              {fibLegs.flatMap((f) =>
                f.events.map((ev) => (
                  <circle
                    key={`${f.from}-${ev.at}`}
                    data-pocket-touch={ev.closesInside ? 'inside' : 'touch'}
                    cx={cx(ev.at)}
                    cy={(yP(Number(f.pocket.low)) + yP(Number(f.pocket.high))) / 2}
                    r={2.2}
                    fill={ev.closesInside ? 'var(--chart-fib)' : 'var(--background)'}
                    stroke="var(--chart-fib)"
                    vectorEffect={THIN}
                  >
                    <title>
                      {ev.closesInside
                        ? 'Toca el golden pocket y cierra adentro'
                        : 'Toca el golden pocket'}
                    </title>
                  </circle>
                )),
              )}

              {/* spring, upthrust, fondo y cruces (#501): en la vela de la señal */}
              {markers.map((mk) => {
                const y = yP(Number(mk.anchor))
                return (
                  <text
                    key={`${mk.kind}-${mk.at}`}
                    data-marker={mk.kind}
                    x={cx(mk.at)}
                    y={mk.side === 'below' ? y + 12 : y - 5}
                    textAnchor="middle"
                    fontSize={9}
                    fontWeight={800}
                    fill="var(--chart-mark)"
                    stroke="var(--background)"
                    strokeWidth={3}
                    paintOrder="stroke"
                  >
                    <title>{`${MARKER_NAME[mk.kind]}: ${mk.detail}`}</title>
                    {MARKER_GLYPH[mk.kind]}
                  </text>
                )
              })}

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

              {/* dibujos del usuario (#543): siguen al encuadre con la capa de precio */}
              {drawOn && (
                <DrawingShapes
                  drawings={drawings}
                  xy={(p) => ({ x: anchorX(p, baseMap), y: yP(p.p) })}
                  span={[-5 * width, 6 * width]}
                  digits={digits}
                  selected={selected}
                  selectable={drawTool === 'none' && !ruler}
                  onSelect={(id) => {
                    setPicked(id)
                    focusChart()
                  }}
                />
              )}
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

              {/* cursor de otro gráfico (#544): la vela que contiene esa hora */}
              {syncCursor && (
                <SyncCursor
                  hub={syncCursor}
                  self={syncId}
                  starts={startMs}
                  ends={ends}
                  x={baseMap.x}
                  top={top}
                  bottom={top + plotH}
                />
              )}
            </g>
          </g>

          <ChartFollower store={measure.store} plot={measure.plot} />

          {drawOn && drawTool !== 'none' && !ruler && n > 0 && (
            <DrawingInput
              key={drawTool}
              tool={drawTool}
              svgRef={svgRef}
              width={width}
              height={height}
              plotW={plotW}
              top={top}
              bottom={top + plotH}
              toAnchor={(x, y) => {
                const m = vp.link.geo().m
                const t = timeAt(startMs, ends, m.start + x / m.slot)
                const p = m.lo + (1 - (y - m.top) / m.h) * (m.hi - m.lo)
                if (!Number.isFinite(t) || !Number.isFinite(p)) return null
                return { t: new Date(t).toISOString(), p }
              }}
              xy={(p) => {
                const m = vp.link.geo().m
                return { x: anchorX(p, m), y: m.y(p.p) }
              }}
              onCreate={(d) => {
                drawingStore.add(instrument, d)
                setPicked(d.id)
                onDrawToolChange?.('none')
                focusChart()
              }}
            />
          )}
        </svg>
        {selected && (
          <button
            type="button"
            onClick={removeSelected}
            className="absolute top-2 left-2 inline-flex h-8 items-center gap-1.5 rounded-full border border-border bg-background/90 px-3 text-[11.5px] font-bold text-foreground shadow-sm focus-visible:outline-2 focus-visible:outline-ring"
          >
            <Trash2 className="size-3.5" aria-hidden />
            Borrar dibujo
          </button>
        )}
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

      {!compact && (
        <ul className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-muted-foreground">
          {liquidity && (
            <>
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
                  <line
                    x1={0}
                    x2={22}
                    y1={3}
                    y2={3}
                    stroke="var(--chart-accent)"
                    strokeWidth={1.6}
                  />
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
            </>
          )}
          {coarse ? (
            <>
              <li>
                Un dedo en horizontal te mueve en el tiempo · dos dedos: zoom · eje de precio:
                arrastrá para estirarlo, doble toque vuelve a automático
              </li>
              <li>Mantené apretado y arrastrá (o usá la regla) para medir; un toque la borra</li>
            </>
          ) : (
            <>
              <li>
                Arrastrá para moverte en el tiempo · rueda: zoom · eje de precio: arrastrá para
                estirarlo, doble clic vuelve a automático
              </li>
              <li>Shift + arrastrar (o la regla) mide; un clic o Esc borra la medición</li>
            </>
          )}
          {layers && tools.ema && layers.ema.length > 0 && (
            <li className="flex items-center gap-1.5">
              <svg width={22} height={6} aria-hidden>
                <line x1={0} x2={22} y1={3} y2={3} stroke="var(--chart-ema)" strokeWidth={1.6} />
              </svg>
              EMA {layers.emaPeriod} diaria
            </li>
          )}
          {layers && tools.fibonacci && layers.fibonacci.length > 0 && (
            <li className="flex items-center gap-1.5">
              <span
                className="inline-block h-2.5 w-4 rounded-[2px] bg-[var(--chart-fib)]/30"
                aria-hidden
              />
              Fibonacci (F) y golden pocket
            </li>
          )}
          {layers && tools.pivots && layers.pivots.length > 0 && (
            <li className="flex items-center gap-1.5">
              <svg width={22} height={6} aria-hidden>
                <line
                  x1={0}
                  x2={22}
                  y1={3}
                  y2={3}
                  stroke="var(--chart-pivot)"
                  strokeDasharray="3 3"
                />
              </svg>
              Puntos pivote (P, R1–R3, S1–S3)
            </li>
          )}
          {markers.length > 0 && (
            <li>
              {[...new Set(markers.map((mk) => mk.kind))]
                .map((k) => `${MARKER_GLYPH[k]} ${MARKER_NAME[k].toLowerCase()}`)
                .join(' · ')}
            </li>
          )}
          {signals.length > 0 && (
            <li className="flex items-center gap-1.5">
              <span className="inline-block size-2 rounded-[2px] bg-up" aria-hidden />
              <span className="inline-block size-2 rounded-[2px] bg-down" aria-hidden />
              señal del motor
            </li>
          )}
        </ul>
      )}
    </div>
  )
}
