/**
 * Fases 8 y 8b desde el gateway (#331), solo `GET`:
 *
 * | Ruta | Respuesta |
 * |---|---|
 * | `GET /api/v1/paper_comparisons` | `paper_comparison.v1[]`: la última de cada instrumento |
 * | `GET /api/v1/paper_comparisons/{id}` | esa `paper_comparison.v1` |
 * | `GET /api/v1/learning_report` | el último `learning_report.v1`, o `null` |
 * | `GET /api/v1/learning_report/votes` | `change_vote_status.v1[]`: el estado de cada propuesta ahora |
 *
 * Todo lo calcula el motor. La UI no recalcula nada: muestra lo que llega. Sin gateway no hay datos
 * de ejemplo. **No se vota desde la UI** (cuentas y 2FA son una decisión aparte).
 */
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'

type Utc = string
/** Decimal con signo como texto. */
type Signed = string

export type ComparisonTotals = { trades: number; net_pnl: Signed }

export type ComparisonLeg = {
  entry_time: Utc
  entry_price: string
  exit_time: Utc
  exit_price: string
  size: string
  commission: Signed
  swap: Signed
  net_pnl: Signed
}

export const CAUSES = [
  'fill_price',
  'size',
  'commission',
  'swap',
  'weekend_close',
  'risk_rejection',
  'missing_in_paper',
  'extra_in_paper',
] as const
export type Cause = (typeof CAUSES)[number]

/** `contracts/paper_comparison.v1`. */
export type PaperComparison = {
  comparison_id: string
  instrument_id: string
  from: Utc
  to: Utc
  created_at: Utc
  paper: ComparisonTotals
  backtest: ComparisonTotals
  /** P/L paper − P/L backtest. */
  difference: Signed
  /** La diferencia repartida por causa (la suma es la diferencia: lo garantiza el motor). */
  causes: Record<Cause, Signed>
  matched: {
    decision_at: Utc
    side: 'long' | 'short'
    order_id: string
    paper: ComparisonLeg
    backtest: ComparisonLeg
    difference: { entry_price: Signed; exit_price: Signed; size: Signed; net_pnl: Signed }
    weekend_close: boolean
  }[]
  missing: { decision_at: Utc; side: 'long' | 'short'; backtest_net_pnl: Signed; paper: string }[]
  extra: {
    decision_at: Utc
    side: 'long' | 'short'
    order_id: string
    paper_net_pnl: Signed
    backtest: string
  }[]
  paper_rejections: Record<string, number>
}

export type Signal =
  | 'trend'
  | 'sma_side'
  | 'candle_pattern'
  | 'double_break'
  | 'level_rejection'
  | 'sweep'
  | 'fvg_or_order_block'
  | 'recent_choch'
  | 'news'

export type SignalStats = {
  cases: number
  hits: number
  hit_rate: string | null
  mean_r: Signed | null
}

export type CalibrationRow = {
  from: string
  to: string
  cases: number
  hits: number
  mean_shown: string | null
  frequency: string | null
  low: string | null
  high: string | null
  insufficient: boolean
}

export type ChangeProposal = {
  change_id: string
  kind: 'recalibrate' | 'review_signal_weight'
  text: string
  evidence: string
  /** Hoy siempre `pendiente_de_voto`: el informe nunca cambia nada solo. */
  status: 'pendiente_de_voto'
}

/** `contracts/learning_report.v1`. */
export type LearningReport = {
  report_id: string
  created_at: Utc
  from: Utc
  to: Utc
  sources: ('backtest' | 'paper' | 'live')[]
  proposals: number
  calibration: {
    rows: CalibrationRow[]
    without_probability: number
    sample: {
      proposal_id: string
      signal_at: Utc
      shown: string
      hit_tp1: boolean
      bucket: number
    }[]
  }
  signals: {
    signal: Signal
    with: SignalStats
    without: SignalStats
    difference_r: Signed | null
    small_sample: boolean
  }[]
  change_proposals: ChangeProposal[]
}

/**
 * Cómo se muestra el estado del voto de una propuesta. La UI solo lo muestra; votar no se hace
 * desde acá.
 */
export type VoteStatus = { label: string; detail?: string }

/**
 * `contracts/change_vote_status.v1`: el estado de una propuesta contado por el motor
 * (`cfds_journal::votes::tally`, reglas de `config/votes.toml`). La UI no cuenta votos.
 */
export type ChangeVoteStatus = {
  change_id: string
  status: 'pendiente_de_voto' | 'aprobada' | 'rechazada'
  in_favor: number
  against: number
}

const STATUS_LABEL: Record<ChangeVoteStatus['status'], string> = {
  pendiente_de_voto: 'pendiente de voto',
  aprobada: 'aprobada',
  rechazada: 'rechazada',
}

/** El estado del motor, en palabras (solo texto: los números llegan contados). */
export function voteLabel(s: ChangeVoteStatus): VoteStatus {
  return {
    label: STATUS_LABEL[s.status],
    detail: `${s.in_favor} a favor, ${s.against} en contra`,
  }
}

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as T
}

export const fetchPaperComparisons = (signal?: AbortSignal) =>
  getJson<PaperComparison[]>('/api/v1/paper_comparisons', signal)

export const fetchLearningReport = (signal?: AbortSignal) =>
  getJson<LearningReport | null>('/api/v1/learning_report', signal)

export const usePaperComparisons = () =>
  useRemote('paper-comparisons', fetchPaperComparisons, REFRESH_MS)

export const useLearningReport = () => useRemote('learning-report', fetchLearningReport, REFRESH_MS)

export const fetchVoteStatuses = (signal?: AbortSignal) =>
  getJson<ChangeVoteStatus[]>('/api/v1/learning_report/votes', signal)

export const useVoteStatuses = () => useRemote('learning-votes', fetchVoteStatuses, REFRESH_MS)
