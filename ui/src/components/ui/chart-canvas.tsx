/**
 * El lienzo de Gráficos y Multigráfico (#581, etapa 5, `data-section="charts"`): el gráfico ocupa
 * todo el alto disponible y los controles van en una barra fina que flota arriba y se atenúa
 * cuando no se usa. Sin vidrio: superficies lisas y líneas de 1 px. Solo presentación: no pide ni
 * calcula nada.
 */
import type { ReactNode, RefObject } from 'react'

import { cn } from '@/lib/utils'

/**
 * La barra flotante de arriba: fina, opaca (sin vidrio) y fija al desplazar (en escritorio). Se atenúa con
 * `idle`; el mouse encima o el foco de teclado adentro la vuelven a mostrar entera. Atenuada no baja
 * de 85 %: así su texto sigue con contraste AA (4,5:1) sobre la superficie oscura y la clara (#617;
 * con 35 % daba 1,9:1).
 */
export function FloatingBar({
  label,
  idle,
  barRef,
  children,
}: {
  label: string
  idle: boolean
  barRef: RefObject<HTMLDivElement | null>
  children: ReactNode
}) {
  return (
    <div
      ref={barRef}
      role="toolbar"
      aria-label={label}
      data-floating-bar
      data-idle={idle || undefined}
      className={cn(
        'relative z-30 flex min-w-0 flex-wrap items-center gap-2 rounded-section border border-border bg-card px-2 py-1.5 shadow-[0_8px_24px_-16px_var(--shadow)] transition-opacity duration-300 lg:sticky lg:top-2',
        idle && 'opacity-85 hover:opacity-100 has-[:focus-visible]:opacity-100',
      )}
    >
      {children}
    </div>
  )
}

/** El contenedor de la sección: superficie lisa con el acento azul de análisis. */
export function ChartCanvas({
  canvasRef,
  children,
  ...rest
}: {
  canvasRef: RefObject<HTMLDivElement | null>
  children: ReactNode
} & { [k: `data-${string}`]: string | undefined }) {
  return (
    <div
      ref={canvasRef}
      data-section="charts"
      {...rest}
      className="grid min-w-0 gap-2 rounded-section bg-section p-1.5 text-foreground sm:p-2"
    >
      {children}
    </div>
  )
}
