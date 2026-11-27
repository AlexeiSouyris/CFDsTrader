/**
 * Los ganchos del lienzo de Gráficos y Multigráfico (#581, etapa 5): cuándo se atenúa la barra
 * flotante y cuánto alto queda para el gráfico. Solo presentación.
 */
import { useEffect, useLayoutEffect, useState, type RefObject } from 'react'

/** Cuánto tiene que estar quieto el mouse sobre el gráfico para que la barra se atenúe. */
export const IDLE_MS = 2500

/**
 * `true` cuando el puntero está quieto sobre `area` (o se fue) y nadie usa la barra: ni el mouse
 * encima ni el foco de teclado adentro. Mover el mouse, tocar o cualquier tecla la despiertan.
 */
export function useIdle(
  area: RefObject<HTMLElement | null>,
  bar: RefObject<HTMLElement | null>,
  ms = IDLE_MS,
) {
  const [idle, setIdle] = useState(false)
  useEffect(() => {
    const el = area.current
    if (!el) return
    let timer = 0
    const busy = () => {
      const b = bar.current
      if (!b) return false
      // el foco de teclado adentro la mantiene; el que queda tras un clic, no
      try {
        return b.matches(':hover') || b.querySelector(':focus-visible') !== null
      } catch {
        return b.matches(':hover')
      }
    }
    const wake = () => {
      setIdle(false)
      window.clearTimeout(timer)
      timer = window.setTimeout(() => setIdle(!busy()), ms)
    }
    wake()
    el.addEventListener('pointermove', wake)
    el.addEventListener('pointerdown', wake)
    el.addEventListener('focusin', wake)
    window.addEventListener('keydown', wake)
    return () => {
      window.clearTimeout(timer)
      el.removeEventListener('pointermove', wake)
      el.removeEventListener('pointerdown', wake)
      el.removeEventListener('focusin', wake)
      window.removeEventListener('keydown', wake)
    }
  }, [area, bar, ms])
  return idle
}

const LG = '(min-width: 1024px)'

/**
 * El alto que queda desde `ref` hasta el borde de abajo de la ventana (sin el desplazamiento de la
 * página), para que el gráfico llene la pantalla. En el celular deja lugar al dock de abajo.
 */
export function useFillHeight(ref: RefObject<HTMLElement | null>, min = 320) {
  const [avail, setAvail] = useState(640)
  useLayoutEffect(() => {
    const measure = () => {
      const el = ref.current
      if (!el) return
      const top = el.getBoundingClientRect().top + window.scrollY
      const wide = window.matchMedia?.(LG).matches ?? true
      setAvail(Math.max(min, Math.round(window.innerHeight - top - (wide ? 24 : 104))))
    }
    measure()
    window.addEventListener('resize', measure)
    return () => window.removeEventListener('resize', measure)
  }, [ref, min])
  return avail
}

/** `true` en pantallas anchas (desde `lg`), donde se ve más de un gráfico a la vez. */
export function useWide() {
  const query = () => window.matchMedia?.(LG).matches ?? true
  const [wide, setWide] = useState(query)
  useEffect(() => {
    const mq = window.matchMedia?.(LG)
    if (!mq) return
    const on = () => setWide(mq.matches)
    mq.addEventListener?.('change', on)
    return () => mq.removeEventListener?.('change', on)
  }, [])
  return wide
}
