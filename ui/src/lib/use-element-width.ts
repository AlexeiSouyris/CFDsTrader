import { useEffect, useLayoutEffect, useRef, useState } from 'react'

/** Ancho real de un elemento, para dibujar SVG 1:1 (el texto no se estira al cambiar el ancho). */
export function useElementWidth<T extends HTMLElement>(initial = 720, min = 260) {
  const ref = useRef<T>(null)
  const [width, setWidth] = useState(initial)
  // Antes de pintar se toma el ancho real: si no, el primer cuadro se dibujaba con `initial` y en
  // pantallas angostas el SVG se salía de su tarjeta hasta que llegaba el ResizeObserver (#415).
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    // el mismo ancho que da el ResizeObserver (`contentRect`): sin el relleno
    const cs = getComputedStyle(el)
    const w =
      el.clientWidth - (parseFloat(cs.paddingLeft) || 0) - (parseFloat(cs.paddingRight) || 0)
    if (w > 0) setWidth(Math.max(min, Math.round(w)))
  }, [min])
  useEffect(() => {
    const el = ref.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(([e]) => setWidth(Math.max(min, Math.round(e.contentRect.width))))
    ro.observe(el)
    return () => ro.disconnect()
  }, [min])
  return [ref, width] as const
}
