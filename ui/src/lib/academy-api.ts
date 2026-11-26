/**
 * La Academia desde el gateway (#719, #769):
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `GET /api/v1/academy/catalog` | `academy_catalog.v1` (sin las claves) |
 * | `GET /api/v1/academy/progress` | `academy_progress.v1` (lo calcula el motor) |
 *
 * Un solo estado compartido (catálogo y progreso) para la vista, los candados y Ajustes: se pide
 * una vez y se vuelve a pedir después de cada cambio (`reloadAcademy`). La UI no decide si algo se
 * aprobó ni qué herramienta está abierta.
 */
import { useEffect, useSyncExternalStore } from 'react'

import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import { request } from '@/lib/paper-api'

const BASE = '/api/v1/academy'

export const fetchCatalog = (signal?: AbortSignal) =>
  request<AcademyCatalog>(`${BASE}/catalog`, { signal })
export const fetchProgress = (signal?: AbortSignal) =>
  request<AcademyProgress>(`${BASE}/progress`, { signal })

export type AcademyState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ok'; catalog: AcademyCatalog; progress: AcademyProgress }

let state: AcademyState = { status: 'loading' }
let started = false
let pending: Promise<void> | null = null
const listeners = new Set<() => void>()

function set(next: AcademyState) {
  state = next
  for (const l of listeners) l()
}

/**
 * Vuelve a pedir el catálogo y el progreso. Si falla y ya había datos, se quedan los últimos (el
 * motor no cambió nada que se sepa); si no había, queda el error.
 */
export function reloadAcademy(): Promise<void> {
  pending ??= Promise.all([fetchCatalog(), fetchProgress()])
    .then(
      ([catalog, progress]) => set({ status: 'ok', catalog, progress }),
      (e: unknown) => {
        if (state.status !== 'ok') {
          set({ status: 'error', message: e instanceof Error ? e.message : String(e) })
        }
      },
    )
    .finally(() => {
      pending = null
    })
  return pending
}

const subscribe = (l: () => void) => {
  listeners.add(l)
  return () => listeners.delete(l)
}

/** El catálogo y el progreso del motor, compartidos por toda la app. */
export function useAcademy(): AcademyState {
  const s = useSyncExternalStore(subscribe, () => state)
  useEffect(() => {
    if (started) return
    started = true
    void reloadAcademy()
  }, [])
  return s
}

/** Solo para las pruebas: vuelve al estado inicial. */
export function resetAcademy() {
  state = { status: 'loading' }
  started = false
  pending = null
}
