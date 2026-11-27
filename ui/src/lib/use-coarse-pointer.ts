/**
 * ¿El puntero principal es un dedo (`pointer: coarse`)? Para mostrar la ayuda de gestos que
 * corresponde (#424): con mouse, rueda y Shift; con el dedo, deslizar, pellizcar y mantener.
 * Sigue los cambios (una tablet a la que se le conecta un mouse).
 */
import { useSyncExternalStore } from 'react'

const QUERY = '(pointer: coarse)'

const media = () =>
  typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia(QUERY)
    : null

function subscribe(onChange: () => void) {
  const m = media()
  m?.addEventListener?.('change', onChange)
  return () => m?.removeEventListener?.('change', onChange)
}

const snapshot = () => media()?.matches === true

export function useCoarsePointer(): boolean {
  return useSyncExternalStore(subscribe, snapshot, () => false)
}
