/**
 * Cambio de pestaña con el trackpad y el teclado (#286). Solo navegación de la interfaz.
 *
 * - **Trackpad:** el deslizamiento horizontal con dos dedos llega como eventos `wheel` con
 *   `deltaX` dominante. Se acumula y, al pasar el umbral, avanza (`+1`) o retrocede (`-1`) una
 *   pestaña; después espera un rato para que la inercia del gesto no salte varias.
 *   No se dispara sobre un elemento con desplazamiento horizontal propio (tablas anchas, gráficos)
 *   ni sobre lo marcado con `data-swipe-ignore`, ni con `Ctrl` (es el zoom con pellizco), ni
 *   cuando otro ya usó el evento (un gráfico que se desplaza con el trackpad, #415).
 *   Se cancela el evento que usa, así el navegador no hace además su "atrás/adelante".
 * - **Teclado:** `Ctrl+←/→` y `⌘+[` / `⌘+]`. No actúa mientras se escribe en un campo.
 * - **Dedo (#307):** un deslizamiento horizontal rápido (más de `touchMinPx`, bastante más
 *   horizontal que vertical, en menos de `touchMaxMs`). Hacia la izquierda avanza. No empieza
 *   pegado a un borde de la pantalla (ahí el iPhone hace su "atrás"), ni sobre algo que usa el
 *   dedo para sí (un gráfico con `touch-none`, la herramienta de medición) o que se desplaza en
 *   horizontal.
 */
import { useEffect, useRef } from 'react'

export type SwipeOptions = {
  /** píxeles horizontales acumulados para cambiar de pestaña */
  threshold?: number
  /** espera después de un cambio, en ms (absorbe la inercia del trackpad) */
  cooldownMs?: number
  /** si pasa este tiempo sin eventos, lo acumulado vuelve a cero */
  idleMs?: number
  /** recorrido horizontal mínimo del dedo, en px */
  touchMinPx?: number
  /** duración máxima del deslizamiento con el dedo, en ms */
  touchMaxMs?: number
  /** franja de cada borde de la pantalla donde no empieza (la usa el sistema), en px */
  touchEdgePx?: number
}

export const SWIPE_DEFAULTS: Required<SwipeOptions> = {
  threshold: 120,
  cooldownMs: 700,
  idleMs: 250,
  touchMinPx: 70,
  touchMaxMs: 600,
  touchEdgePx: 24,
}

/** ¿El dedo empezó sobre algo que lo usa para sí (gráficos con `touch-none`, medición)? */
const ownsTouch = (target: EventTarget | null) =>
  target instanceof Element && target.closest('.touch-none, [data-swipe-ignore]') !== null

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
  const { threshold, cooldownMs, idleMs, touchMinPx, touchMaxMs, touchEdgePx } = {
    ...SWIPE_DEFAULTS,
    ...options,
  }
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
      // Ctrl es el pellizco; si un gráfico ya usó la rueda (desplaza o hace zoom), no es para
      // la pestaña (#415): sin esto, desplazar el gráfico con el trackpad cambiaba de vista.
      if (e.ctrlKey || e.defaultPrevented || ownsTouch(e.target)) return
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

    let touch: { x: number; y: number; t: number } | null = null
    const onTouchStart = (e: TouchEvent) => {
      const p = e.touches[0]
      touch =
        e.touches.length === 1 &&
        p.clientX > touchEdgePx &&
        p.clientX < window.innerWidth - touchEdgePx &&
        !ownsTouch(e.target) &&
        !scrollsHorizontally(e.target)
          ? { x: p.clientX, y: p.clientY, t: Date.now() }
          : null
    }
    const onTouchEnd = (e: TouchEvent) => {
      const start = touch
      touch = null
      const p = e.changedTouches[0]
      if (!start || !p || Date.now() - start.t > touchMaxMs) return
      const dx = p.clientX - start.x
      const dy = p.clientY - start.y
      if (Math.abs(dx) < touchMinPx || Math.abs(dx) < 2 * Math.abs(dy)) return
      step.current(dx < 0 ? 1 : -1)
    }
    const onTouchCancel = () => {
      touch = null
    }

    window.addEventListener('wheel', onWheel, { passive: false })
    window.addEventListener('keydown', onKey)
    window.addEventListener('touchstart', onTouchStart, { passive: true })
    window.addEventListener('touchend', onTouchEnd)
    window.addEventListener('touchcancel', onTouchCancel)
    return () => {
      window.removeEventListener('wheel', onWheel)
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('touchstart', onTouchStart)
      window.removeEventListener('touchend', onTouchEnd)
      window.removeEventListener('touchcancel', onTouchCancel)
    }
  }, [enabled, threshold, cooldownMs, idleMs, touchMinPx, touchMaxMs, touchEdgePx])
}
