/**
 * Herramientas de análisis en el gráfico (#501). **No calcula nada:** el motor manda el bloque
 * `analysis` (Fibonacci, golden pocket, EMA diaria, spring/upthrust, fondos, cruces de medias,
 * puntos pivote y régimen del VIX) con índices de vela; acá solo se cambia cada índice por el
 * cierre de esa vela (`end`), para que el dibujo siga sirviendo cuando el gráfico suma velas más
 * viejas al desplazarse. Los precios siguen como texto, tal como llegan.
 *
 * Cada herramienta se prende o apaga desde el menú "Herramientas"; la preferencia queda en
 * `localStorage` (si el navegador no deja, se usa la de siempre: solo la EMA prendida).
 */
import type { ChartAnalysis, VixRegime } from '@/lib/analysis-contracts'
import { decimal } from '@/lib/format'
import type { ChartBar } from '@/lib/liquidity-bars'

export type AnalysisTool =
  'ema' | 'fibonacci' | 'wyckoff' | 'bottoms' | 'crosses' | 'pivots' | 'vix'

/** Las herramientas en el orden del menú; `term` es el id de su definición en `docs/GLOSARIO.md`. */
export const TOOLS: { id: AnalysisTool; label: string; term: string }[] = [
  { id: 'ema', label: 'EMA diaria', term: 'ema' },
  { id: 'fibonacci', label: 'Fibonacci y golden pocket', term: 'fibonacci' },
  { id: 'wyckoff', label: 'Spring y upthrust', term: 'spring' },
  { id: 'bottoms', label: 'Precio de fondo', term: 'precio-de-fondo' },
  { id: 'crosses', label: 'Cruce dorado / de la muerte', term: 'cruce-dorado' },
  { id: 'pivots', label: 'Puntos pivote diarios', term: 'puntos-pivote' },
  { id: 'vix', label: 'Régimen del VIX', term: 'vix' },
]

export type ToolState = Record<AnalysisTool, boolean>

/** Apagadas por defecto, salvo la EMA. */
export const DEFAULT_TOOLS: ToolState = {
  ema: true,
  fibonacci: false,
  wyckoff: false,
  bottoms: false,
  crosses: false,
  pivots: false,
  vix: false,
}

export const TOOLS_KEY = 'cfdstrader.chart.tools'

/** Lee qué herramientas están prendidas; si no hay nada guardado, está roto o no se puede leer, las de siempre. */
export function loadTools(): ToolState {
  try {
    const raw = JSON.parse(localStorage.getItem(TOOLS_KEY) ?? 'null') as unknown
    if (!raw || typeof raw !== 'object') return { ...DEFAULT_TOOLS }
    const saved = raw as Record<string, unknown>
    const out = { ...DEFAULT_TOOLS }
    for (const t of TOOLS) if (typeof saved[t.id] === 'boolean') out[t.id] = saved[t.id] as boolean
    return out
  } catch {
    return { ...DEFAULT_TOOLS }
  }
}

export function saveTools(state: ToolState) {
  try {
    localStorage.setItem(TOOLS_KEY, JSON.stringify(state))
  } catch {
    // modo privado o almacenamiento bloqueado: las capas andan igual, solo no se recuerdan
  }
}

/** Un precio en el cierre (`at`, UTC) de una vela. */
export type AtPrice = { at: string; price: string }

export type FibLine = { kind: 'retracement' | 'extension'; ratio: string; price: string }

export type FibLayer = {
  direction: 'up' | 'down'
  /** se dibuja desde el cierre de la vela que confirma el tramo… */
  from: string
  /** …hasta el cierre de la que confirma el siguiente (null = hasta la última vela) */
  to: string | null
  start: AtPrice
  end: AtPrice
  lines: FibLine[]
  pocket: { low: string; high: string }
  /** velas que tocan el golden pocket de este tramo y si cierran adentro */
  events: { at: string; closesInside: boolean }[]
}

export type MarkerKind = 'spring' | 'upthrust' | 'bottom' | 'golden' | 'death'

export type Marker = {
  kind: MarkerKind
  /** cierre de la vela de la señal */
  at: string
  /** debajo del mínimo o arriba del máximo de esa vela */
  side: 'below' | 'above'
  /** el mínimo o el máximo de la vela, para ubicar la marca */
  anchor: string
  /** texto para el tooltip, con los valores del motor */
  detail: string
}

export type PivotLayer = {
  day: string
  from: string
  to: string | null
  levels: { name: string; price: string }[]
}

export type AnalysisLayers = {
  emaPeriod: number
  /** tramos seguidos de la EMA (un null la corta) */
  ema: AtPrice[][]
  fibonacci: FibLayer[]
  markers: Marker[]
  pivots: PivotLayer[]
  /** el VIX de la última vela (null sin dato) */
  vix: { close: string; regime: VixRegime } | null
}

export const MARKER_NAME: Record<MarkerKind, string> = {
  spring: 'Spring',
  upthrust: 'Upthrust',
  bottom: 'Precio de fondo',
  golden: 'Cruce dorado',
  death: 'Cruce de la muerte',
}

/** Lo que se lee en el gráfico (una o dos letras). */
export const MARKER_GLYPH: Record<MarkerKind, string> = {
  spring: 'S',
  upthrust: 'U',
  bottom: 'F',
  golden: 'CD',
  death: 'CM',
}

/** De qué herramienta es cada marca. */
export const MARKER_TOOL: Record<MarkerKind, AnalysisTool> = {
  spring: 'wyckoff',
  upthrust: 'wyckoff',
  bottom: 'bottoms',
  golden: 'crosses',
  death: 'crosses',
}

export const VIX_LABEL: Record<VixRegime, string> = {
  calm: 'calma',
  normal: 'normal',
  elevated: 'elevado',
  stress: 'estrés',
}

const PIVOT_NAMES = [
  ['r3', 'R3'],
  ['r2', 'R2'],
  ['r1', 'R1'],
  ['pp', 'P'],
  ['s1', 'S1'],
  ['s2', 'S2'],
  ['s3', 'S3'],
] as const

/**
 * Pasa el bloque `analysis` del motor a capas para dibujar. `bars` son las velas de la vista
 * (las de `candles`, en el mismo orden): los índices del bloque se refieren a ellas. Un índice
 * fuera de las velas se descarta.
 */
export function analysisLayers(analysis: ChartAnalysis, bars: ChartBar[]): AnalysisLayers {
  const at = (i: number) => bars[i]?.end
  const ema: AtPrice[][] = []
  let run: AtPrice[] = []
  analysis.ema_daily.forEach((v, i) => {
    const t = at(i)
    if (v === null || t === undefined) {
      if (run.length) ema.push(run)
      run = []
    } else run.push({ at: t, price: v })
  })
  if (run.length) ema.push(run)

  const fibonacci: FibLayer[] = []
  analysis.fibonacci.forEach((leg, k) => {
    const from = at(leg.confirmed_index)
    const s = at(leg.start_index)
    const e = at(leg.end_index)
    if (from === undefined || s === undefined || e === undefined) return
    const next = analysis.fibonacci[k + 1]
    fibonacci.push({
      direction: leg.direction,
      from,
      to: next ? (at(next.confirmed_index) ?? null) : null,
      start: { at: s, price: leg.start_price },
      end: { at: e, price: leg.end_price },
      lines: [
        ...leg.retracements.map((r) => ({ kind: 'retracement' as const, ...r })),
        ...leg.extensions.map((r) => ({ kind: 'extension' as const, ...r })),
      ],
      pocket: { low: leg.golden_pocket_low, high: leg.golden_pocket_high },
      events: analysis.golden_pocket
        .filter((ev) => ev.leg === k && ev.touches && at(ev.index) !== undefined)
        .map((ev) => ({ at: at(ev.index)!, closesInside: ev.closes_inside })),
    })
  })

  const markers: Marker[] = []
  const mark = (kind: MarkerKind, i: number, detail: string) => {
    const bar = bars[i]
    if (!bar) return
    const side = kind === 'upthrust' || kind === 'death' ? 'above' : 'below'
    markers.push({ kind, at: bar.end, side, anchor: side === 'above' ? bar.high : bar.low, detail })
  }
  for (const w of analysis.wyckoff)
    mark(
      w.kind,
      w.confirmed_index,
      `nivel ${decimal(w.level)} (${w.touches} toques), ${w.kind === 'spring' ? 'mínimo' : 'máximo'} ${decimal(w.extreme)}`,
    )
  for (const b of analysis.bottoms)
    mark(
      'bottom',
      b.confirmed_index,
      `fondo ${decimal(b.price)}, desde el máximo ${decimal(b.prior_high)}`,
    )
  const mc = analysis.ma_cross
  for (const c of mc.crosses)
    mark(
      c.kind,
      c.known_from_index,
      `${mc.kind.toUpperCase()} ${mc.fast_period} y ${mc.slow_period} diarias, día ${c.day}`,
    )

  const pivots: PivotLayer[] = []
  analysis.floor_pivots.forEach((p, k) => {
    const from = at(p.from_index)
    if (from === undefined) return
    const next = analysis.floor_pivots[k + 1]
    pivots.push({
      day: p.day,
      from,
      to: next ? (at(next.from_index) ?? null) : null,
      levels: PIVOT_NAMES.map(([key, name]) => ({ name, price: p[key] })),
    })
  })

  const lastVix = analysis.vix.length ? analysis.vix[analysis.vix.length - 1] : null
  return { emaPeriod: analysis.ema_daily_period, ema, fibonacci, markers, pivots, vix: lastVix }
}

/**
 * Junta las capas de velas más viejas (`older`, las que se piden al desplazarse, #511) con las de
 * velas más nuevas (`newer`), que empiezan en el cierre `from` (UTC) de su primera vela. De
 * `older` queda solo lo anterior a `from`; un tramo o unos puntos pivote que seguían vigentes
 * terminan ahí, y los del mismo día siguen como una sola entrada. No calcula nada: solo ordena
 * por hora lo que mandó el motor para cada pedido.
 */
export function mergeLayers(
  older: AnalysisLayers,
  newer: AnalysisLayers,
  from: string,
): AnalysisLayers {
  const cut = Date.parse(from)
  const before = (iso: string) => Date.parse(iso) < cut
  const upTo = (to: string | null) => (to === null || !before(to) ? from : to)

  const ema = older.ema.map((run) => run.filter((p) => before(p.at))).filter((r) => r.length)
  const [first, ...rest] = newer.ema
  // la EMA sigue sin corte si la vista nueva arranca justo donde termina la vieja
  if (ema.length && first?.[0]?.at === from)
    ema[ema.length - 1] = [...ema[ema.length - 1], ...first]
  else if (first) ema.push(first)
  ema.push(...rest)

  const fibonacci = [
    ...older.fibonacci
      .filter((f) => before(f.from))
      .map((f) => ({ ...f, to: upTo(f.to), events: f.events.filter((e) => before(e.at)) })),
    ...newer.fibonacci,
  ]

  const pivots = older.pivots.filter((p) => before(p.from)).map((p) => ({ ...p, to: upTo(p.to) }))
  const [head, ...tail] = newer.pivots
  const last = pivots[pivots.length - 1]
  if (head && last && last.day === head.day && head.from === from) {
    pivots[pivots.length - 1] = { ...head, from: last.from }
  } else if (head) pivots.push(head)
  pivots.push(...tail)

  return {
    emaPeriod: newer.emaPeriod,
    ema,
    fibonacci,
    markers: [...older.markers.filter((m) => before(m.at)), ...newer.markers],
    pivots,
    vix: newer.vix,
  }
}

/**
 * La entrada que vale en el cierre `t` (ms) de una lista con `from` y `to` en orden: la última
 * que empezó a más tardar en `t`. Para elegir qué rótulos van al eje (geometría de pantalla).
 */
export function activeAt<T extends { from: string; to: string | null }>(
  items: T[],
  t: number,
): T | null {
  let out: T | null = null
  for (const it of items) {
    if (Date.parse(it.from) > t) break
    out = it.to === null || Date.parse(it.to) > t ? it : null
  }
  return out
}
