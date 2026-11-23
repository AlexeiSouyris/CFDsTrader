/**
 * Cambio de pestaña con el trackpad y el teclado (#286). Solo navegación de la interfaz.
 *
 * - **Trackpad:** el deslizamiento horizontal con dos dedos llega como eventos `wheel` con
 *   `deltaX` dominante. Se acumula y, al pasar el umbral, avanza (`+1`) o retrocede (`-1`) una
 *   pestaña; después espera un rato para que la inercia del gesto no salte varias.
 *   No se dispara sobre un elemento con desplazamiento horizontal propio (tablas anchas, gráficos)
 *   ni sobre lo marcado con `data-swipe-ignore`, ni con `Ctrl` (es el zoom con pellizco).
 *   Se cancela el evento que usa, así el navegador no hace además su "atrás/adelante".
 * - **Teclado:** `Ctrl+←/→` y `⌘+[` / `⌘+]`. No actúa mientras se escribe en un campo.
 */
import { useEffect, useRef } from 'react'

export type SwipeOptions = {
  /** píxeles horizontales acumulados para cambiar de pestaña */
  threshold?: number
  /** espera después de un cambio, en ms (absorbe la inercia del trackpad) */
  cooldownMs?: number
  /** si pasa este tiempo sin eventos, lo acumulado vuelve a cero */
  idleMs?: number
}

export const SWIPE_DEFAULTS: Required<SwipeOptions> = {
  threshold: 120,
  cooldownMs: 700,
  idleMs: 250,
}

/** ¿El elemento (o uno de sus padres) se desplaza solo en horizontal, o pide que se lo ignore? */
export function scrollsHorizontally(target: EventTarget | null): boolean {
  let el = target instanceof Element ? target : null
  while (el && el !== document.body && el !== document.documentElement) {
    if (el.hasAttribute('data-swipe-ignore')) return true
    const { overflowX } = getComputedStyle(el)
    if ((overflowX === 'auto' || overflowX === 'scroll') && el.scrollWidth > el.clientWidth)
      return true
    el = el.parentElement
  }
  return false
}

const typing = (t: EventTarget | null) =>
  t instanceof HTMLElement &&
  (t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName))

export function useTabSwipe(
  onStep: (dir: 1 | -1) => void,
  enabled: boolean,
  options: SwipeOptions = {},
) {
  const { threshold, cooldownMs, idleMs } = { ...SWIPE_DEFAULTS, ...options }
  const step = useRef(onStep)
  useEffect(() => {
    step.current = onStep
  })

  useEffect(() => {
    if (!enabled) return
    let acc = 0
    let last = 0
    let lockedUntil = 0

    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey) return
      const dx = e.deltaMode === 1 ? e.deltaX * 16 : e.deltaX
      if (Math.abs(dx) <= Math.abs(e.deltaY)) return
      if (scrollsHorizontally(e.target)) return
      e.preventDefault()
      const now = Date.now()
      if (now < lockedUntil) return
      if (now - last > idleMs) acc = 0
      last = now
      acc += dx
      if (Math.abs(acc) >= threshold) {
        step.current(acc > 0 ? 1 : -1)
        acc = 0
        lockedUntil = now + cooldownMs
      }
    }

    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || typing(e.target) || e.altKey || e.shiftKey) return
      let dir: 1 | -1 | 0 = 0
      if (e.ctrlKey && !e.metaKey) dir = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
      else if (e.metaKey && !e.ctrlKey) dir = e.key === ']' ? 1 : e.key === '[' ? -1 : 0
      if (!dir) return
      e.preventDefault()
      step.current(dir)
    }

    window.addEventListener('wheel', onWheel, { passive: false })
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('wheel', onWheel)
      window.removeEventListener('keydown', onKey)
    }
  }, [enabled, threshold, cooldownMs, idleMs])
}
