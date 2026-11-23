/**
 * Lectura del contexto de mercado desde el gateway (#294), solo `GET`:
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `GET /api/v1/sentiment` | `sentiment.v1[]`: el último de cada instrumento |
 * | `GET /api/v1/sentiment?instrument=<id>` | `sentiment.v1[]`: la historia, de la más vieja a la más nueva |
 * | `GET /api/v1/analogs` | `analog_report.v1[]`: el último de cada instrumento y fuente |
 * | `GET /api/v1/emotions` | `emotional_state.v1[]`: el último de cada instrumento, fuente y base (#220) |
 */
import type { AnalogReport, EmotionalState, SentimentIndex } from '@/lib/context-contracts'
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as T
}

export const fetchLatestSentiment = (signal?: AbortSignal) =>
  getJson<SentimentIndex[]>('/api/v1/sentiment', signal)

export const fetchSentimentHistory = (instrument: string, signal?: AbortSignal) =>
  getJson<SentimentIndex[]>(
    `/api/v1/sentiment?instrument=${encodeURIComponent(instrument)}`,
    signal,
  )

export const fetchAnalogs = (signal?: AbortSignal) =>
  getJson<AnalogReport[]>('/api/v1/analogs', signal)

export const useLatestSentiment = () => useRemote('sentiment', fetchLatestSentiment, REFRESH_MS)

export const useSentimentHistory = (instrument: string | null) =>
  useRemote(
    instrument && `sentiment:${instrument}`,
    (signal) => fetchSentimentHistory(instrument!, signal),
    REFRESH_MS,
  )

export const useAnalogs = () => useRemote('analogs', fetchAnalogs, REFRESH_MS)

export const fetchEmotions = (signal?: AbortSignal) =>
  getJson<EmotionalState[]>('/api/v1/emotions', signal)

export const useEmotions = () => useRemote('emotions', fetchEmotions, REFRESH_MS)
