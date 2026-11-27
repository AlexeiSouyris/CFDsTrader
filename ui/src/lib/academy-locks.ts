/**
 * Candados de la Academia (#771, `docs/design/2026-11-academia.md` §4): si una herramienta de la
 * app está abierta lo dice el motor (`academy_progress.v1`); la UI solo lo lee. Sin progreso del
 * motor (cargando o sin gateway) nada se bloquea: el candado es una ayuda para aprender, no un
 * control de seguridad, y abrir una herramienta no cambia nada del motor, del riesgo ni del paper.
 */
import { useAcademy, type AcademyState } from '@/lib/academy-api'

export type ToolLockState =
  | { locked: false }
  | {
      locked: true
      /** `locked_tools_visible` del catálogo: false esconde la herramienta en vez de mostrarla */
      visible: boolean
      name: string
      lessonId: string
      lessonTitle: string | null
    }

/**
 * La lección que la Academia abre la próxima vez que se muestre (desde el candado de una
 * herramienta). Se usa una sola vez.
 */
let focused: string | null = null
export const focusLesson = (id: string) => {
  focused = id
}
export function takeFocusedLesson(): string | null {
  const id = focused
  focused = null
  return id
}

export function useToolLock(tool: string): ToolLockState {
  return lockOf(useAcademy(), tool)
}

/**
 * El candado de varias herramientas a la vez (las capas del menú Herramientas, #836), con una sola
 * lectura de la Academia.
 */
export function useToolLocks<T extends string>(tools: readonly T[]): Record<T, ToolLockState> {
  const academy = useAcademy()
  return Object.fromEntries(tools.map((t) => [t, lockOf(academy, t)])) as Record<T, ToolLockState>
}

function lockOf(academy: AcademyState, tool: string): ToolLockState {
  if (academy.status !== 'ok') return { locked: false }
  const { catalog, progress } = academy
  const def = catalog.tools.find((t) => t.id === tool)
  const state = progress.tools.find((t) => t.tool_id === tool)
  // una herramienta que no está en el catálogo no tiene candado
  if (!def || !state || state.unlocked) return { locked: false }
  const lesson = catalog.levels.flatMap((l) => l.lessons).find((l) => l.id === state.lesson_id)
  return {
    locked: true,
    visible: catalog.thresholds.locked_tools_visible,
    name: def.name,
    lessonId: state.lesson_id,
    lessonTitle: lesson?.title ?? null,
  }
}
