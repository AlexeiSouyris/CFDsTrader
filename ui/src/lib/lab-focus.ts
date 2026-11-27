/**
 * Qué herramienta del laboratorio pidió otra vista (#807): la Academia (nivel 12) abre el
 * Laboratorio y pide el simulador de cuenta fondeada. Se toma una sola vez al montar.
 */
export type LabTool = 'propsim'

let pending: LabTool | null = null

export const requestLabTool = (tool: LabTool) => {
  pending = tool
}

export function takeLabTool(): LabTool | null {
  const t = pending
  pending = null
  return t
}
