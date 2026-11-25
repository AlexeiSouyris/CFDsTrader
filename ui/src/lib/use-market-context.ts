import { useState } from 'react'

import { useAnalogs, useEmotions, useLatestSentiment, useSentimentHistory } from '@/lib/context-api'

/**
 * Lo que mandó el motor para el instrumento elegido: índice, historia, análogos y estados. Lo usan
 * la vista de contexto del Panel y la columna "Contexto" de Noticias (#581), así la elección vive en un solo lugar.
 */
export function useMarketContext() {
  const latest = useLatestSentiment()
  const analogs = useAnalogs()
  const emotions = useEmotions()
  const [picked, setPicked] = useState<string | null>(null)

  const ids = [
    ...new Set([
      ...(latest.status === 'ok' ? latest.data.map((s) => s.instrument_id) : []),
      ...(analogs.status === 'ok' ? analogs.data.map((a) => a.instrument_id) : []),
      ...(emotions.status === 'ok' ? emotions.data.map((e) => e.instrument_id) : []),
    ]),
  ].sort()
  // Sin elección, el primero que tenga índice guardado: abrir en uno vacío no muestra nada (#357).
  const withIndex =
    latest.status === 'ok'
      ? ids.find((id) => latest.data.some((s) => s.instrument_id === id))
      : undefined
  const selected = picked && ids.includes(picked) ? picked : (withIndex ?? ids[0] ?? null)
  const history = useSentimentHistory(selected)
  const index =
    latest.status === 'ok' ? latest.data.find((s) => s.instrument_id === selected) : undefined
  const report =
    analogs.status === 'ok' ? analogs.data.find((a) => a.instrument_id === selected) : undefined
  const moods =
    emotions.status === 'ok' ? emotions.data.filter((e) => e.instrument_id === selected) : []
  return { latest, analogs, emotions, ids, selected, setPicked, history, index, report, moods }
}
