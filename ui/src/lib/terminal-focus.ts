/**
 * El instrumento que el buscador del encabezado (#362) pide mostrar en el Terminal.
 *
 * El hash de la app es solo el id de la vista (`#terminal`), así que el instrumento viaja aparte:
 * `focusInstrument` lo deja pendiente y avisa con un evento. El Terminal lo lee al montar
 * (`focusedInstrument`, y lo olvida con `clearFocusedInstrument` en un efecto, así el modo
 * estricto de React no lo pierde) o, si ya estaba abierto, al recibir el evento. Solo navegación
 * de la interfaz: no pide ni calcula nada.
 */
export const FOCUS_EVENT = 'cfdstrader:focus-instrument'

let pending: string | null = null

export function focusInstrument(id: string) {
  pending = id
  window.dispatchEvent(new CustomEvent<string>(FOCUS_EVENT, { detail: id }))
}

/** El instrumento pendiente, si hay. */
export const focusedInstrument = (): string | null => pending

export function clearFocusedInstrument() {
  pending = null
}

/** Abre el buscador del encabezado desde cualquier parte (por ejemplo, la página 404). */
export const OPEN_SEARCH_EVENT = 'cfdstrader:open-search'
export const openSearch = () => window.dispatchEvent(new Event(OPEN_SEARCH_EVENT))

/**
 * El timeframe que la paleta de comandos (#581) pide para el gráfico activo del Terminal: mismo
 * patrón que el instrumento (pendiente al montar o evento si ya está abierto).
 */
export const TIMEFRAME_EVENT = 'cfdstrader:focus-timeframe'

let pendingTimeframe: string | null = null

export function focusTimeframe(tf: string) {
  pendingTimeframe = tf
  window.dispatchEvent(new CustomEvent<string>(TIMEFRAME_EVENT, { detail: tf }))
}

/** El timeframe pendiente, si hay. */
export const focusedTimeframe = (): string | null => pendingTimeframe

export function clearFocusedTimeframe() {
  pendingTimeframe = null
}
