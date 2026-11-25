/**
 * El diario de paper desde el gateway (#615, #616), solo `GET`:
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `GET /api/v1/journal` | `gateway_journal.v1`: propuestas de paper con su resultado simulado |
 *
 * Lo guarda el runner de paper del motor (spec §15b) y el gateway lo devuelve tal cual, de la señal
 * más nueva a la más vieja. La UI no calcula nada: muestra lo que llega. Sin gateway no hay datos
 * de ejemplo.
 */
import type { Outcome } from '@/lib/backtest-contracts'
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'
import type { Proposal } from '@/mocks'

/** Una entrada de `gateway_journal.v1`: la propuesta y su resultado simulado, o null. */
export type JournalEntry = { proposal: Proposal; outcome: Outcome | null }

/** `contracts/gateway_journal.v1`. */
export type JournalPage = { entries: JournalEntry[]; limit: number }

export async function fetchJournal(signal?: AbortSignal): Promise<JournalPage> {
  const res = await fetch('/api/v1/journal', {
    signal,
    headers: { Accept: 'application/json' },
  })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as JournalPage
}

export const useJournal = () => useRemote('journal', fetchJournal, REFRESH_MS)
