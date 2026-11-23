/**
 * DATOS DE EJEMPLO del prototipo (#164). Nada de esto viene del motor ni de IG.
 *
 * - Precios y montos van como texto decimal, igual que en `contracts/` (candle.v1).
 * - Los indicadores (RSI, MACD) llegan **ya calculados**: la UI nunca los calcula (CLAUDE.md §1).
 *   El JSON se generó una vez fuera de la app, como lo enviaría el motor.
 * - Las series de comparación y el resto se generan con un PRNG fijo, para que la maqueta
 *   sea siempre igual. Generar datos de ejemplo no es lógica de trading.
 */
import type { FundMovement, FundSummary } from '@/lib/fund-api'

import us100 from './us100-1h.json'

export type PriceBar = {
  start: string
  open: string
  high: string
  low: string
  close: string
  volume: string
  rsi14: string | null
  macd: string | null
  signal: string | null
  histogram: string | null
}

export type PriceSeries = {
  instrument_id: string
  timeframe: string
  note: string
  candles: PriceBar[]
}

export const US100_1H = us100 as PriceSeries

/** PRNG determinista (mulberry32): misma maqueta en cada carga. */
function rng(seed: number) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = Math.imul(a ^ (a >>> 15), a | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

export type Member = { id: string; name: string; initials: string }

/** Miembros del fondo (spec §1.2: 6 miembros). Los nombres reales los define el grupo. */
export const MEMBERS: Member[] = [
  { id: 'alexei', name: 'Alexei', initials: 'AS' },
  { id: 'm2', name: 'Miembro 2', initials: 'M2' },
  { id: 'm3', name: 'Miembro 3', initials: 'M3' },
  { id: 'm4', name: 'Miembro 4', initials: 'M4' },
  { id: 'm5', name: 'Miembro 5', initials: 'M5' },
  { id: 'm6', name: 'Miembro 6', initials: 'M6' },
]

/** El usuario "logueado" de la maqueta. */
export const CURRENT_MEMBER = MEMBERS[0]

export type SparkPoint = { t: string; v: string }
export type SparkRow = {
  id: string
  label: string
  sublabel: string
  points: SparkPoint[]
  /** cambio del período, ya calculado (lo enviaría el motor) */
  changePct: string
}

function dailySeries(seed: number, base: number, trend: number, days = 90): SparkPoint[] {
  const r = rng(seed)
  const start = Date.UTC(2026, 7, 24)
  let drift = 0
  return Array.from({ length: days }, (_, i) => {
    drift = drift * 0.86 + (r() - 0.5) * 0.02
    const v = base * (1 + trend * (i / days) + Math.sin(i / 6) * 0.01 + drift)
    return {
      t: new Date(start + i * 86_400_000).toISOString().replace('.000', ''),
      v: v.toFixed(1),
    }
  })
}

function withChange(row: Omit<SparkRow, 'changePct'>): SparkRow {
  const first = Number(row.points[0].v)
  const last = Number(row.points[row.points.length - 1].v)
  // Solo para armar el ejemplo; en la app real este número llega del motor.
  return { ...row, changePct: (((last - first) / first) * 100).toFixed(2) }
}

/** Comparación de índices (90 días). */
export const INDEX_ROWS: SparkRow[] = [
  {
    id: 'US100',
    label: 'US Tech 100',
    sublabel: 'NASDAQ 100',
    points: dailySeries(1, 24800, 0.09),
  },
  { id: 'US500', label: 'US 500', sublabel: 'S&P 500', points: dailySeries(2, 6420, 0.05) },
  { id: 'US30', label: 'Wall Street', sublabel: 'Dow Jones', points: dailySeries(3, 46100, 0.02) },
  { id: 'DE40', label: 'Germany 40', sublabel: 'DAX', points: dailySeries(4, 23900, -0.015) },
].map(withChange)

export type HeatmapCompany = {
  sym: string
  name: string
  /** capitalización en billones de USD (tamaño del cuadro) */
  cap: string
  /** cambio del día en % (color del cuadro) */
  chg: string
  price: string
}

/** Componentes del NASDAQ 100 con números inventados para la maqueta. */
export const HEATMAP: HeatmapCompany[] = [
  ['NVDA', 'NVIDIA', '4.10', '2.84', '168.20'],
  ['MSFT', 'Microsoft', '3.62', '0.91', '487.10'],
  ['AAPL', 'Apple', '3.48', '-0.42', '231.40'],
  ['AMZN', 'Amazon', '2.31', '1.37', '216.80'],
  ['GOOGL', 'Alphabet', '2.20', '-0.88', '182.30'],
  ['META', 'Meta', '1.71', '-1.64', '678.90'],
  ['AVGO', 'Broadcom', '1.38', '2.11', '293.70'],
  ['TSLA', 'Tesla', '1.02', '-2.73', '318.60'],
  ['NFLX', 'Netflix', '0.52', '0.34', '1205.10'],
  ['COST', 'Costco', '0.42', '-0.21', '944.30'],
  ['AMD', 'AMD', '0.33', '3.42', '204.50'],
  ['PEP', 'PepsiCo', '0.20', '0.12', '146.20'],
  ['ADBE', 'Adobe', '0.16', '-1.12', '378.40'],
  ['CSCO', 'Cisco', '0.27', '0.48', '68.10'],
  ['INTC', 'Intel', '0.11', '-0.97', '24.60'],
  ['QCOM', 'Qualcomm', '0.17', '1.05', '158.90'],
].map(([sym, name, cap, chg, price]) => ({ sym, name, cap, chg, price }))

export type BookLevel = { price: string; size: string; total: string }
export type OrderBook = { mid: string; spread: string; bids: BookLevel[]; asks: BookLevel[] }

function book(): OrderBook {
  const r = rng(77)
  const mid = 28365.3
  const make = (side: 1 | -1) => {
    let total = 0
    return Array.from({ length: 16 }, (_, i) => {
      const size = 2 + r() * 9 + i * 0.6
      total += size
      return {
        price: (mid + side * (0.6 + i * 1.4)).toFixed(1),
        size: size.toFixed(2),
        total: total.toFixed(2),
      }
    })
  }
  return { mid: mid.toFixed(1), spread: '1.2', bids: make(-1), asks: make(1) }
}

/** Profundidad de mercado de ejemplo. Los totales acumulados vienen dados, como del proveedor. */
export const US100_BOOK = book()

export type TradeRow = {
  id: string
  at: string
  instrument: string
  side: 'Largo' | 'Corto'
  size: string
  entry: string
  /** P/L neto en EUR, ya con costos (lo calcula el motor, spec §11). null si está abierta. */
  netPnl: string | null
  status: 'Abierta' | 'Ganada' | 'Perdida'
  mode: 'simulación'
}

/** Diario de operaciones en simulación (paper). */
export const TRADES: TradeRow[] = [
  {
    id: 'p-0119',
    at: '2026-11-20T15:45:00Z',
    instrument: 'US100',
    side: 'Largo',
    size: '0.20',
    entry: '28204.6',
    netPnl: null,
    status: 'Abierta',
    mode: 'simulación',
  },
  {
    id: 'p-0118',
    at: '2026-11-19T16:30:00Z',
    instrument: 'US500',
    side: 'Corto',
    size: '0.50',
    entry: '6712.4',
    netPnl: '-18.40',
    status: 'Perdida',
    mode: 'simulación',
  },
  {
    id: 'p-0117',
    at: '2026-11-19T10:15:00Z',
    instrument: 'DE40',
    side: 'Largo',
    size: '0.30',
    entry: '23518.0',
    netPnl: '41.25',
    status: 'Ganada',
    mode: 'simulación',
  },
  {
    id: 'p-0116',
    at: '2026-11-18T15:00:00Z',
    instrument: 'US100',
    side: 'Largo',
    size: '0.20',
    entry: '27942.1',
    netPnl: '63.10',
    status: 'Ganada',
    mode: 'simulación',
  },
  {
    id: 'p-0115',
    at: '2026-11-17T17:45:00Z',
    instrument: 'US30',
    side: 'Corto',
    size: '0.10',
    entry: '46980.5',
    netPnl: '-22.75',
    status: 'Perdida',
    mode: 'simulación',
  },
  {
    id: 'p-0114',
    at: '2026-11-12T14:30:00Z',
    instrument: 'US100',
    side: 'Corto',
    size: '0.20',
    entry: '27511.8',
    netPnl: '12.60',
    status: 'Ganada',
    mode: 'simulación',
  },
]

export type FolderFile = {
  name: string
  meta: string
  source: 'fed' | 'ecb' | 'nota'
  href?: string
}
export type MemberFolder = { owner: Member; files: FolderFile[] }

/** Carpetas de noticias: cada miembro guarda las que le importan. Todos ven todas; cada uno
 *  modifica solo la suya (eso lo tiene que imponer el servidor, la UI solo lo refleja). */
export const FOLDERS: MemberFolder[] = [
  {
    owner: MEMBERS[0],
    files: [
      { name: 'FOMC: comunicado', meta: 'Fed · 19 nov', source: 'fed' },
      { name: 'BCE: decisión de tipos', meta: 'BCE · 30 oct', source: 'ecb' },
      { name: 'Mis notas del NASDAQ', meta: 'Nota · hoy', source: 'nota' },
    ],
  },
  {
    owner: MEMBERS[1],
    files: [
      { name: 'Minutas del FOMC', meta: 'Fed · 19 nov', source: 'fed' },
      { name: 'Discurso de Waller', meta: 'Fed · 20 nov', source: 'fed' },
    ],
  },
  {
    owner: MEMBERS[2],
    files: [{ name: 'Estadísticas del BCE', meta: 'BCE · 21 nov', source: 'ecb' }],
  },
]

export type Trophy = {
  period: 'día' | 'semana'
  winner: Member
  /** resultado de la competencia en simulación, ya calculado (en R, spec §8) */
  result: string
  date: string
  serial: string
}

/** Competencia interna en simulación: no es dinero real ni una promesa de ganancia. */
export const TROPHIES: Trophy[] = [
  { period: 'día', winner: MEMBERS[2], result: '+2.4 R', date: '2026-11-21', serial: 'D-0021' },
  {
    period: 'semana',
    winner: MEMBERS[0],
    result: '+6.1 R',
    date: '2026-11-17 / 21',
    serial: 'S-0004',
  },
]

export type Notice = { id: string; title: string; detail: string }
export const NOTICES: Notice[] = [
  {
    id: 'n1',
    title: 'Propuesta vencida',
    detail: 'US100 largo de las 15:45 venció sin aprobar (simulación).',
  },
  {
    id: 'n2',
    title: 'Noticia relevante',
    detail: 'Fed: comunicado del FOMC, relevancia alta para US100.',
  },
]

export type Kpi = {
  id: string
  label: string
  value: string
  unit: string
  detail: string
  tone?: 'up' | 'down'
}

/** Indicadores del panel (los calcula el motor: valor de la unidad, P/L, drawdown). */
export const KPIS: Kpi[] = [
  {
    id: 'nav',
    label: 'Valor de la unidad',
    value: '1.0412',
    unit: 'EUR',
    detail: 'desde el inicio de la simulación',
  },
  {
    id: 'pnl',
    label: 'P/L de la semana',
    value: '75.80',
    unit: 'EUR',
    detail: 'simulación, con costos',
    tone: 'up',
  },
  {
    id: 'dd',
    label: 'Drawdown máximo',
    value: '-3.10',
    unit: '%',
    detail: 'en la simulación',
    tone: 'down',
  },
  { id: 'props', label: 'Propuestas', value: '2', unit: '', detail: 'esperando al responsable' },
]

export type RankingRow = { member: Member; result: string; trades: number }

/** Tabla de la competencia de la semana (simulación; la regla la define el grupo). */
export const RANKING: RankingRow[] = [
  { member: MEMBERS[0], result: '6.1', trades: 9 },
  { member: MEMBERS[2], result: '4.8', trades: 7 },
  { member: MEMBERS[1], result: '2.2', trades: 11 },
  { member: MEMBERS[4], result: '-0.6', trades: 5 },
  { member: MEMBERS[3], result: '-1.4', trades: 8 },
  { member: MEMBERS[5], result: '-2.9', trades: 6 },
]

// --- Fase 10, adelanto (#171) ---

/** Quién aprueba propuestas (spec §1: roles responsable y miembro). */
export const RESPONSIBLE_ID = 'alexei'

/** Una propuesta con los campos de `contracts/proposal.v1.schema.json`. */
export type Proposal = {
  id: string
  source: 'backtest' | 'paper' | 'live'
  run_id: string
  instrument_id: string
  side: 'long' | 'short'
  signal_at: string
  reference: string
  stop: string
  tp1: string
  size: string
  leverage: string
  score: number
  breakdown: { component: BreakdownComponent; points: number }[]
  triggers: ('candle_pattern' | 'double_break' | 'level_rejection' | 'sweep')[]
}
export type BreakdownComponent =
  | 'trend'
  | 'sma_side'
  | 'candle_pattern'
  | 'double_break'
  | 'level_rejection'
  | 'sweep'
  | 'fvg_or_order_block'
  | 'recent_choch'
  | 'news'

/**
 * Lo que la pantalla necesita y `proposal.v1` todavía no trae: el vencimiento (spec §6, 10
 * minutos) y la probabilidad de §13 con su rango. **A definir en un contrato** antes de la app real.
 */
export type ProposalExtras = {
  expiresAt: string
  probability:
    | { status: 'ok'; pct: string; lowPct: string; highPct: string; cases: number }
    | { status: 'insufficient'; cases: number; needed: number }
}
export type PendingProposal = { proposal: Proposal; extras: ProposalExtras }

// Las horas de ejemplo se arman alrededor de "ahora" para que el vencimiento se vea correr.
const quarter = Math.floor(Date.now() / 900_000) * 900_000
const iso = (ms: number) => new Date(ms).toISOString().replace('.000Z', 'Z')

/** Propuestas pendientes de ejemplo (paper). Los números los arma el motor, no la UI. */
export const PROPOSALS: PendingProposal[] = [
  {
    proposal: {
      id: `paper-demo:US100:${iso(quarter)}:long`,
      source: 'paper',
      run_id: 'paper-demo',
      instrument_id: 'US100',
      side: 'long',
      signal_at: iso(quarter),
      reference: '28204.6',
      stop: '28150.2',
      tp1: '28313.4',
      size: '0.19',
      leverage: '5.15',
      score: 6,
      breakdown: [
        { component: 'trend', points: 2 },
        { component: 'sma_side', points: 1 },
        { component: 'sweep', points: 2 },
        { component: 'fvg_or_order_block', points: 1 },
      ],
      triggers: ['sweep'],
    },
    extras: {
      expiresAt: iso(Date.now() + 8 * 60_000),
      probability: { status: 'ok', pct: '58', lowPct: '49', highPct: '66', cases: 84 },
    },
  },
  {
    proposal: {
      id: `paper-demo:DE40:${iso(quarter)}:short`,
      source: 'paper',
      run_id: 'paper-demo',
      instrument_id: 'DE40',
      side: 'short',
      signal_at: iso(quarter),
      reference: '23518.0',
      stop: '23561.5',
      tp1: '23431.0',
      size: '0.24',
      leverage: '5.42',
      score: 5,
      breakdown: [
        { component: 'trend', points: 2 },
        { component: 'candle_pattern', points: 1 },
        { component: 'level_rejection', points: 1 },
        { component: 'recent_choch', points: 1 },
      ],
      triggers: ['candle_pattern', 'level_rejection'],
    },
    extras: {
      expiresAt: iso(Date.now() + 4 * 60_000),
      probability: { status: 'insufficient', cases: 21, needed: 30 },
    },
  },
]

export type FundMember = {
  member: Member
  units: string
  netContributions: string
  value: string
  pnl: string
}
export type FundMove = {
  id: string
  member: Member
  kind: 'aporte' | 'retiro'
  amount: string
  units: string
  nav: string
  at: string
}

/** Un movimiento ya liquidado: `contracts/fund_movement.v1` (el tipo vive en `lib/fund-api`). */
export type { FundMovement }

const opening = (member: Member, n: number, amount: string): FundMovement => ({
  id: `fund:2026-10-05:${n}`,
  settled_at: '2026-10-05T06:00:00Z',
  member_id: member.id,
  kind: 'contribution',
  amount,
  units: amount,
  nav: '1',
  currency: 'EUR',
})

/** El fondo (spec §14). Todo llega calculado por el motor (`fund`): la UI solo lo muestra. */
export const FUND = {
  currency: 'EUR',
  nav: '1.0412',
  navAt: '2026-11-20T21:00:00Z',
  total: '1041.20',
  units: '1000.00',
  members: [
    {
      member: MEMBERS[0],
      units: '300.00',
      netContributions: '300.00',
      value: '312.36',
      pnl: '12.36',
    },
    {
      member: MEMBERS[1],
      units: '200.00',
      netContributions: '200.00',
      value: '208.24',
      pnl: '8.24',
    },
    {
      member: MEMBERS[2],
      units: '150.00',
      netContributions: '150.00',
      value: '156.18',
      pnl: '6.18',
    },
    {
      member: MEMBERS[3],
      units: '150.00',
      netContributions: '150.00',
      value: '156.18',
      pnl: '6.18',
    },
    {
      member: MEMBERS[4],
      units: '100.00',
      netContributions: '100.00',
      value: '104.12',
      pnl: '4.12',
    },
    {
      member: MEMBERS[5],
      units: '100.00',
      netContributions: '100.00',
      value: '104.12',
      pnl: '4.12',
    },
  ] satisfies FundMember[],
  /** Historial liquidado (`fund_movement.v1`): de acá salen las cuentas, nunca al revés. */
  history: [
    opening(MEMBERS[0], 1, '300.00'),
    opening(MEMBERS[1], 2, '200.00'),
    opening(MEMBERS[2], 3, '150.00'),
    opening(MEMBERS[3], 4, '150.00'),
    opening(MEMBERS[4], 5, '100.00'),
    opening(MEMBERS[5], 6, '100.00'),
  ] satisfies FundMovement[],
  moves: [
    {
      id: 'mv3',
      member: MEMBERS[2],
      kind: 'aporte',
      amount: '50.00',
      units: '48.02',
      nav: '1.0412',
      at: '2026-11-23T07:00:00Z',
    },
    {
      id: 'mv2',
      member: MEMBERS[5],
      kind: 'retiro',
      amount: '20.00',
      units: '19.21',
      nav: '1.0412',
      at: '2026-11-23T07:00:00Z',
    },
  ] satisfies FundMove[],
}

/**
 * El fondo de ejemplo con la forma de `fund_summary.v1`, para cuando el gateway no responde.
 * Es `FUND` copiado campo por campo (sin cuentas): los números los escribió una persona.
 */
export const FUND_EXAMPLE: FundSummary = {
  currency: FUND.currency,
  basis: 'last_settlement',
  valued_at: FUND.navAt,
  nav: FUND.nav,
  total_value: FUND.total,
  total_units: FUND.units,
  members: FUND.members.map((m) => ({
    member_id: m.member.id,
    units: m.units,
    value: m.value,
    net_contributions: m.netContributions,
    pnl: m.pnl,
  })),
}

/** El historial de ejemplo (`fund_movement.v1[]`). */
export const FUND_HISTORY_EXAMPLE: FundMovement[] = FUND.history

export * from './backtest'

export * from './liquidity'

export * from './terminal'
