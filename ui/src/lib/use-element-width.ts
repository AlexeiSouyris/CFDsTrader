import { useEffect, useRef, useState } from 'react'

/** Ancho real de un elemento, para dibujar SVG 1:1 (el texto no se estira al cambiar el ancho). */
export function useElementWidth<T extends HTMLElement>(initial = 720, min = 260) {
  const ref = useRef<T>(null)
  const [width, setWidth] = useState(initial)
  useEffect(() => {
    const el = ref.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(([e]) => setWidth(Math.max(min, Math.round(e.contentRect.width))))
    ro.observe(el)
    return () => ro.disconnect()
  }, [min])
  return [ref, width] as const
}
