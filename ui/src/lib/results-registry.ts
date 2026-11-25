/**
 * El registro de resultados (`docs/analysis/registro-de-resultados.csv`, CLAUDE.md §5b): cada
 * prueba del proyecto es una fila. Se lee al compilar (`?raw`, como el glosario) y se muestra tal
 * cual: la UI no recalcula ni reinterpreta ningún valor, solo separa las columnas.
 */
import source from '../../../docs/analysis/registro-de-resultados.csv?raw'

export type RegistryRow = {
  id: string
  fecha: string
  programa: string
  prueba: string
  instrumentos: string
  periodo: string
  metrica: string
  valor_escenario_eur: string
  veredicto: string
  informe: string
  issue_pr: string
}

/** Separa un CSV (RFC 4180: comillas dobles, "" escapada, saltos de línea dentro de comillas). */
export function parseCsv(text: string): string[][] {
  const rows: string[][] = []
  let row: string[] = []
  let field = ''
  let quoted = false
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') {
        field += '"'
        i++
      } else if (c === '"') quoted = false
      else field += c
    } else if (c === '"') quoted = true
    else if (c === ',') {
      row.push(field)
      field = ''
    } else if (c === '\n' || c === '\r') {
      if (c === '\r' && text[i + 1] === '\n') i++
      row.push(field)
      if (row.some((f) => f !== '')) rows.push(row)
      row = []
      field = ''
    } else field += c
  }
  row.push(field)
  if (row.some((f) => f !== '')) rows.push(row)
  return rows
}

/** Las filas del registro, en el orden del archivo (de la más vieja a la más nueva). */
export function parseRegistry(text: string): RegistryRow[] {
  const [header = [], ...rows] = parseCsv(text)
  return rows.map((cells) => {
    const get = (name: string) => cells[header.indexOf(name)] ?? ''
    return {
      id: get('id'),
      fecha: get('fecha'),
      programa: get('programa'),
      prueba: get('prueba'),
      instrumentos: get('instrumentos'),
      periodo: get('periodo'),
      metrica: get('metrica'),
      valor_escenario_eur: get('valor_escenario_eur'),
      veredicto: get('veredicto'),
      informe: get('informe'),
      issue_pr: get('issue_pr'),
    }
  })
}

export const REGISTRY: RegistryRow[] = parseRegistry(source)

/** Los programas (estrategias o rondas) del registro, en el orden en que aparecen por primera vez. */
export const registryPrograms = (rows: RegistryRow[]) => [
  ...new Set(rows.map((r) => r.programa).filter(Boolean)),
]
