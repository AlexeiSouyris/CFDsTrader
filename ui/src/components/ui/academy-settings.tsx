/**
 * Ajustes → Academia (#771, `docs/design/2026-11-academia.md` §4.1): el dueño abre a mano las
 * herramientas con candado (todas o una) y las vuelve a bloquear. Cada cambio es un evento del
 * motor (`tool_override`, quién y cuándo) y el estado que se muestra es el que recalcula el motor.
 * No cambia nada del motor, del riesgo ni del paper: solo qué se ve en la UI.
 */
import { useState } from 'react'

import { postEvent, useAcademy } from '@/lib/academy-api'
import type { EventRequest } from '@/lib/academy-contracts'

const button =
  'min-h-8 rounded-sm border border-border px-2 text-[11.5px] font-semibold hover:border-rule hover:bg-surface disabled:opacity-60'

export function AcademySettings() {
  const academy = useAcademy()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  if (academy.status !== 'ok') {
    return (
      <p className="text-[11.5px] text-muted-foreground">
        {academy.status === 'loading'
          ? 'Pidiendo la Academia al motor…'
          : 'Sin conexión con el motor: los candados no se aplican.'}
      </p>
    )
  }
  const { catalog, progress } = academy
  const send = (tool_id: string, action: 'unlock' | 'relock') => {
    const req: EventRequest = { kind: 'tool_override', tool_id, action }
    setBusy(true)
    setError(null)
    postEvent(req)
      .catch((e: unknown) =>
        setError(`El motor no lo guardó (${e instanceof Error ? e.message : String(e)}).`),
      )
      .finally(() => setBusy(false))
  }
  const names = new Map(catalog.tools.map((t) => [t.id, t.name]))
  return (
    <div className="grid gap-2">
      <button
        type="button"
        disabled={busy}
        onClick={() => send('*', progress.unlock_all ? 'relock' : 'unlock')}
        className={button}
      >
        {progress.unlock_all ? 'Volver a bloquear todo' : 'Desbloquear todo'}
      </button>
      <ul aria-label="Herramientas con candado" className="grid max-h-[40svh] overflow-y-auto">
        {progress.tools.map((t) => (
          <li
            key={t.tool_id}
            className="flex min-h-9 items-center justify-between gap-2 border-t border-border text-[11.5px]"
          >
            <span className="min-w-0">
              <span className="block truncate font-semibold">
                {names.get(t.tool_id) ?? t.tool_id}
              </span>
              <span className="text-muted-foreground">
                {!t.unlocked
                  ? `bloqueada · lección ${t.lesson_id}`
                  : t.by === 'lesson'
                    ? `abierta por la lección ${t.lesson_id}`
                    : 'abierta a mano'}
              </span>
            </span>
            {/* abierta por su lección no hay nada que hacer a mano */}
            {t.by !== 'lesson' && !progress.unlock_all && (
              <button
                type="button"
                disabled={busy}
                aria-label={`${t.unlocked ? 'Bloquear' : 'Abrir'} ${names.get(t.tool_id) ?? t.tool_id}`}
                onClick={() => send(t.tool_id, t.unlocked ? 'relock' : 'unlock')}
                className={button}
              >
                {t.unlocked ? 'Bloquear' : 'Abrir'}
              </button>
            )}
          </li>
        ))}
      </ul>
      {error && (
        <p role="alert" className="text-[11.5px] text-down">
          {error}
        </p>
      )}
    </div>
  )
}
