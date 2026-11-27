/**
 * Puerta a una sección desde el Inicio (#581; fila densa desde #708): el ícono con el acento de su
 * sección, el nombre, una línea de qué hay adentro (o un dato vivo chico, si el motor lo manda) y
 * su atajo de teclado. No calcula nada: el dato llega ya escrito.
 */
import { ArrowUpRight, type LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

/** Las secciones de `index.css` (`data-section`): cada puerta toma el acento de la suya. */
export type Section =
  'hub' | 'terminal' | 'charts' | 'research' | 'news' | 'fund' | 'journal' | 'trophies'

export type Door = {
  /** la vista a la que lleva (su hash) */
  view: string
  section: Section
  title: string
  description: string
  icon: LucideIcon
  /** tecla que la abre desde el Inicio */
  shortcut: string
}

export function SectionDoor({
  door,
  datum,
  onOpen,
}: {
  door: Door
  /** un dato chico que ya viene del motor, o nada */
  datum?: ReactNode
  onOpen: (view: string) => void
}) {
  const Icon = door.icon
  return (
    <a
      href={`#${door.view}`}
      data-section={door.section}
      data-door={door.view}
      aria-keyshortcuts={door.shortcut}
      onClick={(e) => {
        if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return
        e.preventDefault()
        onOpen(door.view)
      }}
      className={cn(
        // una fila densa del espacio de trabajo (#708): la marca del acento a la izquierda
        'group grid min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-x-2.5 border-l-2 border-transparent py-1.5 pr-1 pl-2 text-foreground hover:border-accent hover:bg-surface',
        'focus-visible:outline-2 focus-visible:outline-accent',
      )}
    >
      <Icon aria-hidden="true" className="size-4 text-accent" />
      <span className="grid min-w-0">
        <strong className="text-[12.5px] leading-tight font-bold">{door.title}</strong>
        <span className="truncate text-[11px] leading-snug text-muted-foreground">
          {datum ?? door.description}
        </span>
      </span>
      <span className="flex items-center gap-1.5">
        <kbd
          aria-hidden="true"
          className="hidden rounded-sm border border-border px-1.5 text-[10px] text-muted-foreground sm:inline-block"
        >
          {door.shortcut}
        </kbd>
        <ArrowUpRight
          aria-hidden="true"
          className="size-3.5 text-muted-foreground group-hover:text-accent"
        />
      </span>
    </a>
  )
}
