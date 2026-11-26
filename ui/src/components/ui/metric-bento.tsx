/**
 * Bento de métricas (#581; celdas lisas separadas por reglas de 1 px desde #708): una grilla de
 * cifras de tamaños distintos, cada una con su etiqueta y su detalle. Idea tomada de los "stats
 * bento" de la propuesta (docs/design/2026-11-rediseno-ui.md §3); el código es propio. Muestra el texto que le pasan, tal como llega: no calcula nada.
 */
import type { ReactNode } from 'react'

import { ExampleBadge } from '@/components/ui/data-honesty'
import { cn } from '@/lib/utils'

export type Metric = {
  id: string
  label: string
  value: ReactNode
  detail?: ReactNode
  /** ocupa dos columnas en la grilla grande */
  wide?: boolean
  /** dato de ejemplo (sin fuente real todavía): lleva el sello */
  example?: boolean
}

export function MetricBento({ label, metrics }: { label: string; metrics: Metric[] }) {
  return (
    <dl
      aria-label={label}
      className="grid grid-cols-2 gap-px overflow-hidden rounded-section border border-border bg-border lg:grid-cols-4"
    >
      {metrics.map((m) => (
        <div
          key={m.id}
          data-metric={m.id}
          className={cn(
            'flex min-w-0 flex-col justify-between gap-1 bg-card px-2.5 py-2',
            m.wide && 'col-span-2',
          )}
        >
          <dt className="flex flex-wrap items-center justify-between gap-1.5 text-[11px] font-bold text-muted-foreground">
            {m.label}
            {m.example && <ExampleBadge />}
          </dt>
          <dd className="min-w-0 font-mono text-[clamp(14px,1.2vw,16px)] leading-tight font-semibold break-words tabular-nums">
            {m.value}
          </dd>
          {m.detail && (
            <dd className="text-[11px] leading-snug text-muted-foreground">{m.detail}</dd>
          )}
        </div>
      ))}
    </dl>
  )
}
