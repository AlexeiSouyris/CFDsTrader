/**
 * Honestidad de los datos (#577): el sello único "EJEMPLO" para lo que todavía no tiene fuente
 * real, y el aviso "datos de hace N días" para lo que llega viejo. Toda la UI usa estas piezas, así
 * el ejemplo se ve igual en todas partes y nunca pasa por un dato del motor. No calcula nada.
 */
import type { ReactNode } from 'react'

import { staleDays } from '@/lib/format'
import { cn } from '@/lib/utils'

/** El sello "EJEMPLO": datos inventados para mostrar el diseño, no del motor. */
export function ExampleBadge({ className }: { className?: string }) {
  return (
    <span
      data-example-badge
      title="Datos de ejemplo: no vienen del motor."
      className={cn(
        'inline-flex shrink-0 items-center rounded-md border border-dashed border-[var(--chart-signal)] bg-[var(--chart-signal)]/10 px-1.5 py-0.5 text-[10px] leading-none font-extrabold tracking-[0.12em] text-[var(--chart-signal)] normal-case',
        className,
      )}
    >
      EJEMPLO<span className="sr-only">: datos de ejemplo, no vienen del motor</span>
    </span>
  )
}

/** Una línea con el sello y el porqué, arriba de una sección que es toda de ejemplo. */
export function ExampleNotice({
  children,
  className,
}: {
  children: ReactNode
  className?: string
}) {
  return (
    <p
      role="note"
      data-example-notice
      className={cn(
        'flex flex-wrap items-center gap-x-2 gap-y-1 rounded-xl border border-dashed border-[var(--chart-signal)]/60 px-3 py-2 text-[12px] leading-snug text-muted-foreground',
        className,
      )}
    >
      <ExampleBadge />
      <span>{children}</span>
    </p>
  )
}

/** "datos de hace N días" si `at` es más viejo que `STALE_AFTER_DAYS`; si no, nada. */
export function StaleNote({
  at,
  className,
}: {
  at: string | null | undefined
  className?: string
}) {
  const days = staleDays(at)
  if (days === null) return null
  return (
    <span
      data-stale-note
      className={cn(
        'inline-flex items-center rounded-md bg-down/10 px-1.5 py-0.5 text-[11px] font-bold whitespace-nowrap text-down',
        className,
      )}
    >
      datos de hace {days} días
    </span>
  )
}
