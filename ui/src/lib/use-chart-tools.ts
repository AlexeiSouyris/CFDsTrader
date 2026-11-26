/**
 * Qué herramientas de análisis están prendidas (#501), compartido por todos los gráficos de la
 * página y guardado en el navegador (`analysis-layers.ts`).
 *
 * Cada capa tiene además su candado de la Academia (#836): bloqueada, no se dibuja aunque esté
 * prendida. La preferencia guardada no se toca, así que al abrirse vuelve como estaba.
 */
import { useEffect, useState } from 'react'

import { useToolLocks, type ToolLockState } from '@/lib/academy-locks'
import {
  loadTools,
  saveTools,
  TOOLS,
  type AnalysisTool,
  type ToolState,
} from '@/lib/analysis-layers'

/** Aviso entre gráficos de la misma página: una preferencia cambiada en uno llega a todos. */
const TOOLS_EVENT = 'cfdstrader:chart-tools'

const LOCK_IDS = [...new Set(TOOLS.map((t) => t.lock))]

export type ChartToolLocks = Record<AnalysisTool, ToolLockState>

/**
 * Las herramientas que se dibujan (prendidas y abiertas en la Academia), cómo prender o apagar
 * una (la preferencia, compartida por todos los gráficos y guardada en el navegador) y el
 * candado de cada una.
 */
export function useChartTools(): [
  ToolState,
  (id: AnalysisTool, on: boolean) => void,
  ChartToolLocks,
] {
  const [saved, setSaved] = useState<ToolState>(loadTools)
  const byId = useToolLocks(LOCK_IDS)
  useEffect(() => {
    const sync = () => setSaved(loadTools())
    window.addEventListener(TOOLS_EVENT, sync)
    return () => window.removeEventListener(TOOLS_EVENT, sync)
  }, [])
  const toggle = (id: AnalysisTool, on: boolean) => {
    const next = { ...saved, [id]: on }
    setSaved(next)
    saveTools(next)
    window.dispatchEvent(new Event(TOOLS_EVENT))
  }
  const locks = Object.fromEntries(TOOLS.map((t) => [t.id, byId[t.lock]])) as ChartToolLocks
  const shown = Object.fromEntries(
    TOOLS.map((t) => [t.id, saved[t.id] && !locks[t.id].locked]),
  ) as ToolState
  return [shown, toggle, locks]
}
