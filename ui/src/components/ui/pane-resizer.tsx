/**
 * Divisor entre paneles (#581): una línea de 1 px que se arrastra con el mouse o el dedo y se mueve
 * con el teclado (flechas, Shift + flechas para pasos largos, Inicio y Fin). Sigue el patrón
 * "window splitter" de WAI-ARIA: `role="separator"` enfocable con su valor actual, mínimo y máximo.
 * Solo presentación: el ancho lo guarda quien lo usa.
 */
import { useRef, type KeyboardEvent, type PointerEvent } from 'react'

import { cn } from '@/lib/utils'

const STEP = 16
const BIG_STEP = 64

export function PaneResizer({
  label,
  controls,
  value,
  min,
  max,
  onChange,
  side,
  className,
}: {
  /** qué ancho cambia, para el lector de pantalla */
  label: string
  /** id del panel que cambia de ancho */
  controls: string
  /** ancho actual del panel, en px */
  value: number
  min: number
  max: number
  onChange: (px: number) => void
  /** de qué lado está el panel: a la izquierda crece al ir a la derecha; a la derecha, al revés */
  side: 'left' | 'right'
  className?: string
}) {
  const drag = useRef<{ x: number; w: number } | null>(null)
  const clamp = (px: number) => Math.round(Math.min(max, Math.max(min, px)))
  const dir = side === 'left' ? 1 : -1

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? BIG_STEP : STEP
    let next: number | null = null
    if (e.key === 'ArrowRight') next = value + step * dir
    else if (e.key === 'ArrowLeft') next = value - step * dir
    else if (e.key === 'Home') next = min
    else if (e.key === 'End') next = max
    if (next === null) return
    e.preventDefault()
    onChange(clamp(next))
  }

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return
    e.preventDefault()
    e.currentTarget.setPointerCapture?.(e.pointerId)
    e.currentTarget.focus()
    drag.current = { x: e.clientX, w: value }
  }
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return
    onChange(clamp(drag.current.w + (e.clientX - drag.current.x) * dir))
  }
  const end = () => {
    drag.current = null
  }

  return (
    <div
      role="separator"
      tabIndex={0}
      aria-orientation="vertical"
      aria-label={label}
      aria-controls={controls}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuetext={`${value} px`}
      data-pane-resizer={side}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={end}
      onPointerCancel={end}
      className={cn(
        // zona de agarre de 9 px centrada en la línea; la línea se pinta con el acento al usarla
        'group absolute inset-y-0 z-10 w-[9px] cursor-col-resize touch-none outline-none',
        className,
      )}
    >
      <span
        aria-hidden="true"
        className="absolute inset-y-0 left-1 w-px bg-transparent transition-colors group-hover:bg-accent group-focus-visible:w-0.5 group-focus-visible:bg-accent group-active:bg-accent"
      />
    </div>
  )
}
