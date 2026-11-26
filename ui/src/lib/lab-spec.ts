/**
 * El borrador del constructor del laboratorio (#778) y cómo se arma la `strategy_spec.v1`. Solo
 * traduce lo que eligió la persona a la forma del contrato, con los bloques y parámetros que
 * manda el catálogo del motor (`strategy_catalog.v1`): **no valida rangos ni calcula nada** (eso
 * lo hace el motor en `POST /api/v1/lab/validate`, CLAUDE.md §1). Un parámetro opcional apagado no
 * va en la spec (el motor nunca lo rellena).
 */
import type {
  BlockSchema,
  LabTimeframe,
  ParamSchema,
  ParamValue,
  SpecBlock,
  SpecValidation,
  StrategyCatalog,
  StrategySpec,
} from '@/lib/lab-contracts'

/** Un parámetro tal como está en el formulario: si va en la spec y su texto. */
export type ParamDraft = { on: boolean; value: string }
export type BlockDraft = { type: string; params: Record<string, ParamDraft> }

export type Draft = {
  instrument_id: string
  timeframe: LabTimeframe
  direction: StrategySpec['direction']
  entry: BlockDraft[]
  filters: BlockDraft[]
  stop: BlockDraft
  exit: BlockDraft
  risk_pct: string
  cost_multiplier: string
  /** "AAAA-MM-DD" (00:00 UTC) */
  from: string
  to: string
  initial_equity: string
}

const text = (v: ParamValue | null) => (v === null ? '' : String(v))

/** Un bloque nuevo con lo que propone el catálogo: los parámetros con `default` ya prendidos. */
export function blockDraft(b: BlockSchema): BlockDraft {
  const params: Record<string, ParamDraft> = {}
  for (const p of b.params) {
    const proposed = p.default ?? null
    params[p.name] = { on: p.required || proposed !== null, value: text(proposed) }
  }
  return { type: b.type, params }
}

/** Los bloques de un lugar que sirven para el timeframe. */
export const blocksFor = (cat: StrategyCatalog, slot: BlockSchema['slot'], tf: LabTimeframe) =>
  cat.blocks.filter((b) => b.slot === slot && b.timeframes.includes(tf))

export const schemaOf = (cat: StrategyCatalog, type: string) =>
  cat.blocks.find((b) => b.type === type)

/** El borrador inicial: el primer mercado y timeframe del catálogo y sus primeros bloques. */
export function initialDraft(cat: StrategyCatalog): Draft {
  const market = cat.markets[0]
  const tf = market?.timeframes[0] ?? '1h'
  const first = (slot: BlockSchema['slot']) => {
    const b = blocksFor(cat, slot, tf)[0]
    return b ? blockDraft(b) : { type: '', params: {} }
  }
  return {
    instrument_id: market?.instrument_id ?? '',
    timeframe: tf,
    direction: 'both',
    entry: [first('entry')],
    filters: [],
    stop: first('stop'),
    exit: first('exit'),
    risk_pct: cat.limits.max_risk_pct,
    cost_multiplier: cat.limits.min_cost_multiplier,
    from: '',
    to: '',
    initial_equity: '',
  }
}

/** El valor de un parámetro en la forma del contrato (sin juzgar el rango). */
function paramValue(p: ParamSchema, raw: string): ParamValue {
  const v = raw.trim()
  switch (p.kind) {
    case 'integer':
      return /^-?\d+$/.test(v) ? Number(v) : v
    case 'boolean':
      return v === 'true'
    case 'decimal':
      return v.replace(',', '.')
    default:
      return v
  }
}

function toBlock(cat: StrategyCatalog, d: BlockDraft): SpecBlock {
  const schema = schemaOf(cat, d.type)
  const params: Record<string, ParamValue> = {}
  for (const p of schema?.params ?? []) {
    const draft = d.params[p.name]
    if (draft?.on) params[p.name] = paramValue(p, draft.value)
  }
  return { type: d.type, params }
}

const decimalText = (s: string) => s.trim().replace(',', '.')

/**
 * La spec del borrador, o lo que falta completar para poder mandarla (fechas, capital). Lo demás
 * (rangos, combinaciones, fuera de muestra) lo juzga el motor.
 */
export function toSpec(
  cat: StrategyCatalog,
  d: Draft,
): { spec: StrategySpec; missing: [] } | { spec: null; missing: string[] } {
  const missing: string[] = []
  const day = /^\d{4}-\d{2}-\d{2}$/
  if (!day.test(d.from)) missing.push('desde')
  if (!day.test(d.to)) missing.push('hasta')
  if (missing.length > 0) return { spec: null, missing }
  return {
    spec: {
      catalog_version: cat.catalog_version,
      market: { instrument_id: d.instrument_id, source: 'futures', timeframe: d.timeframe },
      direction: d.direction,
      entry: d.entry.map((b) => toBlock(cat, b)),
      filters: d.filters.map((b) => toBlock(cat, b)),
      stop: toBlock(cat, d.stop),
      exit: toBlock(cat, d.exit),
      risk_pct: decimalText(d.risk_pct),
      cost_multiplier: decimalText(d.cost_multiplier),
      period: { from: `${d.from}T00:00:00Z`, to: `${d.to}T00:00:00Z` },
      mode: 'exploratory',
    },
    missing: [],
  }
}

/** El capital del pedido como texto decimal, o `null` si no es un número positivo escrito. */
export function equityText(s: string): string | null {
  const v = s.replace(/\s/g, '').replace(',', '.')
  return /^[1-9]\d*(\.\d+)?$/.test(v) ? v : null
}

/** Los mensajes del motor para un campo (JSON Pointer exacto o de adentro). */
export function errorsAt(v: SpecValidation | null, path: string): string[] {
  if (!v) return []
  return v.errors
    .filter((e) => e.path === path || e.path.startsWith(`${path}/`))
    .map((e) => e.message)
}

/** "ma_cross · average=ema, fast=20, slow=50": cómo se ve un bloque en la lista de variantes. */
export const blockSummary = (b: SpecBlock) =>
  `${b.type} · ${Object.entries(b.params)
    .map(([k, v]) => `${k}=${String(v)}`)
    .join(', ')}`
