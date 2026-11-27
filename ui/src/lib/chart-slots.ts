/**
 * Los lugares de una grilla de gráficos (#286, #540): qué instrumento y timeframe muestra cada uno
 * y de dónde salen sus velas (gateway o, sin gateway, los datos de ejemplo). Lo comparten la
 * Terminal y el Multigráfico; el dibujo está en `components/ui/chart-slot.tsx`. No calcula nada.
 *
 * Un lugar puede mostrar un instrumento de IBKR (#556, `feed: 'ibkr'`): sus velas salen de
 * `GET /api/v1/ibkr/bars`, sin capas de liquidez (el motor no las calcula para IBKR), y son solo
 * datos: no se operan.
 */
import { fetchOlderIbkrBars, ibkrChartBars, useIbkrBars } from '@/lib/ibkr-api'
import { liveLabel, useIbkrLive, withLiveBars } from '@/lib/ibkr-live'
import { IBKR_TIMEFRAMES, type GatewayIbkrBars, type IbkrTimeframe } from '@/lib/ibkr-contracts'
import { catalogNames, fetchOlderBars, useInstruments, useLiquidityView } from '@/lib/liquidity-api'
import type { OlderPage } from '@/lib/liquidity-api'
import { chartBars, type ChartBar, type ChartTimeframe } from '@/lib/liquidity-bars'
import type { LiquidityViewData, ViewTimeframe } from '@/lib/liquidity-contracts'
import { LIQUIDITY_US100, TERMINAL_INSTRUMENTS } from '@/mocks'

/** `feed`: sin él, el broker simulado (vista de liquidez); `ibkr`, los datos de IBKR. */
export type ChartSel = { instrument: string; timeframe: ChartTimeframe; feed?: 'ibkr' }
export const TIMEFRAMES: ViewTimeframe[] = ['15m', '1h', '1d']
/** Lo que abre cada lugar de la grilla la primera vez. */
export const DEFAULT_SLOTS: ChartSel[] = [
  { instrument: 'US100', timeframe: '15m' },
  { instrument: 'DE40', timeframe: '15m' },
  { instrument: 'US500', timeframe: '1h' },
  { instrument: 'US30', timeframe: '1h' },
]

/**
 * Con el gateway en vivo, si los lugares siguen como vienen (nadie los cambió) y IBKR tiene
 * instrumentos, abren los de IBKR: hoy son los únicos con velas reales y frescas, y los del
 * broker simulado arrancaban vacíos (QA terminal #23, #275). Los primeros en 15 m y el resto en
 * 1 h. `null` si no hay que cambiar nada. Sin gateway no se llama: queda el ejemplo de siempre.
 */
export function preferIbkr(slots: readonly ChartSel[], ibkr: readonly string[]): ChartSel[] | null {
  const untouched = slots.every(
    (s, i) => selKey(s) === selKey(DEFAULT_SLOTS[i]) && s.timeframe === DEFAULT_SLOTS[i].timeframe,
  )
  if (!untouched || ibkr.length === 0) return null
  return DEFAULT_SLOTS.map((_, i) => ({
    instrument: ibkr[i % ibkr.length],
    timeframe: i < ibkr.length ? '15m' : '1h',
    feed: 'ibkr' as const,
  }))
}

const timeframesOf = (feed?: 'ibkr'): readonly ChartTimeframe[] =>
  feed === 'ibkr' ? IBKR_TIMEFRAMES : TIMEFRAMES

/** Los lugares guardados que sirven; los que faltan o están rotos vuelven a los de siempre. */
export function parseSlots(raw: unknown): ChartSel[] {
  const list = Array.isArray(raw) ? (raw as Partial<ChartSel>[]) : []
  return DEFAULT_SLOTS.map((d, i) => {
    const s = list[i]
    if (!s || typeof s.instrument !== 'string') return d
    const feed = s.feed === 'ibkr' ? ('ibkr' as const) : undefined
    if (s.feed !== undefined && !feed) return d
    if (!timeframesOf(feed).includes(s.timeframe as ChartTimeframe)) return d
    const sel: ChartSel = { instrument: s.instrument, timeframe: s.timeframe as ChartTimeframe }
    return feed ? { ...sel, feed } : sel
  })
}

/**
 * Cambia el instrumento (y la fuente) de un lugar. El timeframe se queda si la fuente lo tiene;
 * si no (`1m` no existe en el broker simulado, `1d` no existe en IBKR), pasa a 15 m.
 */
export function withInstrumentSel(sel: ChartSel, instrument: string, feed?: 'ibkr'): ChartSel {
  const timeframe = timeframesOf(feed).includes(sel.timeframe) ? sel.timeframe : '15m'
  return feed ? { instrument, timeframe, feed } : { instrument, timeframe }
}

/** Un lugar como texto para los selectores: `ibkr:MNQ` o `US100`. */
export const selKey = (sel: { instrument: string; feed?: 'ibkr' }) =>
  sel.feed === 'ibkr' ? `ibkr:${sel.instrument}` : sel.instrument

/** Al revés de `selKey`. */
export const parseSelKey = (key: string): { instrument: string; feed?: 'ibkr' } =>
  key.startsWith('ibkr:') ? { instrument: key.slice(5), feed: 'ibkr' } : { instrument: key }

/**
 * Instrumentos, timeframes y nombres legibles (#861) del catálogo del gateway; sin gateway, los de
 * ejemplo y sin nombres (se ve el id).
 */
export function useChartCatalog() {
  const catalog = useInstruments()
  return catalog.status === 'ok'
    ? {
        instruments: catalog.data.instruments.map((i) => i.id),
        timeframes: catalog.data.timeframes,
        names: catalogNames(catalog),
      }
    : { instruments: TERMINAL_INSTRUMENTS, timeframes: TIMEFRAMES, names: catalogNames(catalog) }
}

/** Los timeframes que ofrece un lugar: los del catálogo o, con IBKR, los de IBKR. */
export const slotTimeframes = (sel: ChartSel, catalog: readonly ViewTimeframe[]) =>
  sel.feed === 'ibkr' ? IBKR_TIMEFRAMES : catalog

export type SlotData = {
  sel: ChartSel
  loading: boolean
  /** de dónde salen las velas que se ven; `null` si no hay */
  source: 'motor' | 'ejemplo' | 'ibkr' | null
  /** el gateway no respondió (distinto de responder sin velas, #577) */
  offline: boolean
  /** la vista de liquidez (velas, niveles, FVG, `analysis`); `null` con IBKR o sin velas */
  view: LiquidityViewData | null
  /** la respuesta de IBKR, con su serie (contrato); `null` si el lugar no es de IBKR o no hay */
  ibkr: GatewayIbkrBars | null
  /** las velas para dibujar; vacío si no hay */
  bars: ChartBar[]
  /** velas más viejas (`before`), si la fuente las da */
  loadOlder?: (before: string) => Promise<ChartBar[] | OlderPage>
  /**
   * La última vela es la que está en formación (IBKR por streaming, #558): su inicio y la
   * etiqueta del tipo de dato ("en vivo" o "IBKR · delayed ~10 min"); `null` si no hay.
   */
  forming?: { start: string; label: string; partial: boolean } | null
}

/**
 * Las velas de un lugar: las del motor o, solo si el gateway no respondió, las de ejemplo de US100
 * en 15 m; con IBKR, las de IBKR (sin ejemplo). Si el gateway responde sin velas, el lugar queda
 * vacío: nunca se muestra el ejemplo como si fuera del motor (#577). Con `enabled` en falso (un
 * lugar que la distribución no muestra) no se pide nada.
 */
export function useChartSlot(sel: ChartSel, enabled = true): SlotData {
  const ibkrSel = sel.feed === 'ibkr'
  const remote = useLiquidityView(
    enabled && !ibkrSel
      ? { instrument: sel.instrument, timeframe: sel.timeframe as ViewTimeframe }
      : null,
  )
  const ibkrPick =
    enabled && ibkrSel
      ? { instrument: sel.instrument, timeframe: sel.timeframe as IbkrTimeframe }
      : null
  const remoteIbkr = useIbkrBars(ibkrPick)
  // Vela en formación (#558): la manda el motor por SSE; la UI solo la ubica al final.
  const liveIbkr = useIbkrLive(ibkrPick)
  if (ibkrSel) {
    const data =
      remoteIbkr.status === 'ok' && remoteIbkr.data.series && remoteIbkr.data.bars.length > 0
        ? remoteIbkr.data
        : null
    const contract = data?.series?.contract
    const merged = withLiveBars(data ? ibkrChartBars(data.bars) : [], contract, liveIbkr)
    const forming = merged.forming?.bar
      ? {
          start: merged.forming.bar.start,
          label: liveLabel(merged.forming),
          partial: merged.forming.bar.partial,
        }
      : null
    return {
      sel,
      loading: remoteIbkr.status === 'loading',
      source: data ? 'ibkr' : null,
      offline: remoteIbkr.status === 'error',
      view: null,
      ibkr: data,
      bars: merged.bars,
      forming,
      loadOlder:
        data && contract
          ? (before) => fetchOlderIbkrBars(data.instrument_id, data.timeframe, contract, before)
          : undefined,
    }
  }
  const offline = remote.status === 'error'
  const example =
    offline && sel.instrument === LIQUIDITY_US100.instrument_id && sel.timeframe === '15m'
      ? LIQUIDITY_US100
      : null
  const live: LiquidityViewData | null =
    remote.status === 'ok' && remote.data.candles.length > 0 ? remote.data : null
  const data = live ?? example
  return {
    sel,
    loading: remote.status === 'loading',
    source: live ? 'motor' : example ? 'ejemplo' : null,
    offline,
    view: data,
    ibkr: null,
    bars: data ? chartBars(data) : [],
    loadOlder: live
      ? (before) =>
          fetchOlderBars(
            live.instrument_id,
            live.timeframe ?? (sel.timeframe as ViewTimeframe),
            before,
          )
      : undefined,
  }
}
