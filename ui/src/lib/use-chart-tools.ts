/**
 * Qué herramientas de análisis están prendidas (#501), compartido por todos los gráficos de la
 * página y guardado en el navegador (`analysis-layers.ts`).
 */
import { useEffect, useState } from 'react'

import { loadTools, saveTools, type AnalysisTool, type ToolState } from '@/lib/analysis-layers'

/** Aviso entre gráficos de la misma página: una preferencia cambiada en uno llega a todos. */
const TOOLS_EVENT = 'cfdstrader:chart-tools'

/** Las herramientas prendidas, compartidas por todos los gráficos y guardadas en el navegador. */
export function useChartTools(): [ToolState, (id: AnalysisTool, on: boolean) => void] {
  const [tools, setTools] = useState<ToolState>(loadTools)
  useEffect(() => {
    const sync = () => setTools(loadTools())
    window.addEventListener(TOOLS_EVENT, sync)
    return () => window.removeEventListener(TOOLS_EVENT, sync)
  }, [])
  const toggle = (id: AnalysisTool, on: boolean) => {
    const next = { ...tools, [id]: on }
    setTools(next)
    saveTools(next)
    window.dispatchEvent(new Event(TOOLS_EVENT))
  }
  return [tools, toggle]
}
