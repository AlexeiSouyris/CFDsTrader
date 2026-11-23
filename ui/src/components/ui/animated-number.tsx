/**
 * Un número que cambia con un contador suave (#286). **Solo presentación:** el valor llega ya
 * calculado (texto decimal) y al final se muestra exactamente ese texto. Los pasos intermedios
 * usan `Number` solo para dibujar la transición, como el resto de la UI para ubicar cosas.
 *
 * No anima al montarse (lo primero que se ve es el valor real) ni con `prefers-reduced-motion`.
 * Mientras anima, el valor final queda para los lectores de pantalla.
 */
import { useEffect, useRef, useState } from 'react'

const DURATION_MS = 600

const reducedMotion = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true

export function AnimatedNumber({
  value,
  format,
}: {
  /** texto decimal, por ejemplo "10342.18" */
  value: string
  format: (text: string) => string
}) {
  const [shown, setShown] = useState<string | null>(null)
  const from = useRef(value)

  useEffect(() => {
    const start = Number(from.current)
    const end = Number(value)
    from.current = value
    if (!Number.isFinite(start) || !Number.isFinite(end) || start === end || reducedMotion()) return
    const digits = (value.split('.')[1] ?? '').length
    const t0 = performance.now()
    let frame = requestAnimationFrame(function tick(now) {
      const k = Math.min(1, (now - t0) / DURATION_MS)
      const eased = 1 - (1 - k) ** 3
      if (k >= 1) {
        setShown(null)
        return
      }
      setShown((start + (end - start) * eased).toFixed(digits))
      frame = requestAnimationFrame(tick)
    })
    return () => {
      cancelAnimationFrame(frame)
      setShown(null)
    }
  }, [value])

  if (shown === null) return <>{format(value)}</>
  return (
    <>
      <span aria-hidden="true">{format(shown)}</span>
      <span className="sr-only">{format(value)}</span>
    </>
  )
}
