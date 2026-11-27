/**
 * Cuadrícula de 7×7 puntos que se encienden por cuadros. Inspirado en "Dot Loader" (21st.dev),
 * reescrito (#164): sin intervalos colgados y con `prefers-reduced-motion` (queda un cuadro fijo).
 */
import { useEffect, useState, type ComponentProps } from 'react'
import { useReducedMotion } from 'motion/react'

import { cn } from '@/lib/utils'

/** Una vela japonesa que sube y baja, dibujada en la cuadrícula de 7×7 (índices 0–48). */
const CANDLE_FRAMES: number[][] = [
  [3, 10, 17, 24, 31, 38, 45, 16, 18, 23, 25, 30, 32],
  [3, 10, 17, 24, 31, 38, 9, 11, 16, 18, 23, 25],
  [10, 17, 24, 31, 38, 45, 2, 4, 9, 11, 16, 18, 23, 25],
  [17, 24, 31, 38, 45, 9, 11, 16, 18, 23, 25, 30, 32],
  [3, 10, 17, 24, 31, 38, 45, 23, 25, 30, 32, 37, 39],
  [3, 10, 17, 24, 31, 38, 16, 18, 23, 25, 30, 32],
]

export function DotLoader({
  frames = CANDLE_FRAMES,
  interval = 140,
  className,
  dotClassName,
  ...props
}: {
  frames?: number[][]
  interval?: number
  dotClassName?: string
} & ComponentProps<'div'>) {
  const reduced = useReducedMotion()
  const [i, setI] = useState(0)
  useEffect(() => {
    if (reduced) return
    const t = window.setInterval(() => setI((x) => (x + 1) % frames.length), interval)
    return () => window.clearInterval(t)
  }, [frames.length, interval, reduced])
  const lit = new Set(frames[i] ?? [])
  return (
    <div {...props} className={cn('grid w-fit grid-cols-7 gap-0.5', className)} aria-hidden="true">
      {Array.from({ length: 49 }, (_, k) => (
        <span
          key={k}
          className={cn(
            'size-1.5 rounded-[2px] bg-foreground/15 transition-colors duration-100',
            lit.has(k) && 'bg-foreground',
            dotClassName,
          )}
        />
      ))}
    </div>
  )
}

/** Pantalla de carga de la app. */
export function LoadingScreen({ label = 'Cargando el panel' }: { label?: string }) {
  return (
    <div role="status" className="grid min-h-svh place-items-center">
      <div className="panel flex items-center gap-4 rounded-sm px-5 py-4">
        <DotLoader />
        <p className="text-[13px] font-semibold">{label}…</p>
      </div>
    </div>
  )
}
