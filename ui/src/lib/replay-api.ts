/**
 * El replay de la Academia desde el gateway (#723, #772):
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `POST /api/v1/academy/replay` | `replay_session.v1#/$defs/request` → `replay_session.v1` |
 * | `POST /api/v1/academy/replay/{id}/step` | `replay_step.v1#/$defs/request` → `replay_step.v1` |
 *
 * El cursor vive en el motor: la UI pide «avanzar N» y dibuja solo las velas que vuelven. No hay
 * forma de pedir una vela posterior al cursor ni la fecha real antes de terminar. Las sesiones
 * viven en la memoria del gateway (la más vieja se cierra al llenarse).
 */
import { postJson } from '@/lib/academy-api'
import type { ReplayBar, ReplaySession, ReplayStep, SessionRequest } from '@/lib/replay-contracts'

const BASE = '/api/v1/academy/replay'
const session = (id: string) => `${BASE}/${encodeURIComponent(id)}`

/** La sesión que se está viendo: las velas recibidas, el cursor y si llegó al final. */
export type Live = { session: ReplaySession; bars: ReplayBar[]; cursor: number; done: boolean }

/** La sesión abierta sigue al volver a la vista (vive en el gateway hasta que se cierre). */
export let kept: Live | null = null
export const keep = (l: Live | null) => {
  kept = l
}

export const openReplay = (req: SessionRequest) => postJson<ReplaySession>(BASE, req)

export const stepReplay = (id: string, bars: number) =>
  postJson<ReplayStep>(`${session(id)}/step`, { bars })

/**
 * Suma las velas nuevas de un paso a las que ya se ven, por su número: una vela repetida no se
 * duplica. No calcula nada: solo junta lo que mandó el motor.
 */
export function withStep(bars: ReplayBar[], step: ReplayStep): ReplayBar[] {
  const last = bars.at(-1)?.index ?? -1
  return [...bars, ...step.bars.filter((b) => b.index > last)]
}

/** «día 2 · 14:30»: la vela sin su fecha real (día de la sesión y hora UTC del inicio). */
export const barLabel = (b: ReplayBar) => `día ${b.day} · ${b.time}`
