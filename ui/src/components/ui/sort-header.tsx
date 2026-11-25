/**
 * El encabezado de una columna que se puede ordenar (#581, etapa 6): un botón dentro del `th`, con
 * `aria-sort` para los lectores de pantalla. Ordenar es presentación (`lib/sort-rows.ts`).
 */
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'

import type { Sort } from '@/lib/sort-rows'
import { cn } from '@/lib/utils'

export function SortHeader<K extends string>({
  label,
  column,
  sort,
  onSort,
  className,
  right = false,
}: {
  label: string
  column: K
  sort: Sort<K>
  onSort: (column: K) => void
  className?: string
  /** columna de números: alineada a la derecha */
  right?: boolean
}) {
  const dir = sort?.key === column ? sort.dir : null
  const Icon = dir === 'asc' ? ArrowUp : dir === 'desc' ? ArrowDown : ArrowUpDown
  return (
    <th
      scope="col"
      aria-sort={dir === 'asc' ? 'ascending' : dir === 'desc' ? 'descending' : undefined}
      className={cn(right && 'text-right', className)}
    >
      <button
        type="button"
        onClick={() => onSort(column)}
        title={`Ordenar por ${label.toLowerCase()}`}
        className={cn(
          'inline-flex items-center gap-1 rounded-sm uppercase hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
          right && 'flex-row-reverse',
          dir && 'text-accent',
        )}
      >
        {label}
        <Icon className={cn('size-3', !dir && 'opacity-40')} aria-hidden />
      </button>
    </th>
  )
}
