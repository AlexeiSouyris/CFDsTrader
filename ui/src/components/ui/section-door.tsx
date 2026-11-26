/**
 * Puerta a una sección desde el Inicio (#581): tarjeta de vidrio con el acento de su sección, una
 * línea de qué hay adentro, un dato vivo chico (si el motor lo manda) y su atajo de teclado.
 * Idea de grilla tomada de los "bento" de la propuesta (docs/design/2026-11-rediseno-ui.md §3);
 * el código es propio. No calcula nada: el dato llega ya escrito.
 */
import { ArrowUpRight, type LucideIcon } from 'lucide-react'
import { motion, useReducedMotion } from 'motion/react'
import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

/** Las secciones de `index.css` (`data-section`): cada puerta toma el acento de la suya. */
export type Section = 'terminal' | 'charts' | 'research' | 'news' | 'fund' | 'journal' | 'trophies'

export type Door = {
  /** la vista a la que lleva (su hash) */
  view: string
  section: Section
  title: string
  description: string
  icon: LucideIcon
  /** tecla que la abre desde el Inicio */
  shortcut: string
  /** cuánto ocupa en la grilla grande */
  size?: 'wide' | 'tall'
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
  const reduce = useReducedMotion()
  const Icon = door.icon
  return (
    <motion.a
      href={`#${door.view}`}
      data-section={door.section}
      data-door={door.view}
      aria-keyshortcuts={door.shortcut}
      onClick={(e) => {
        if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return
        e.preventDefault()
        onOpen(door.view)
      }}
      whileHover={reduce ? undefined : { y: -3 }}
      transition={{ type: 'spring', stiffness: 420, damping: 30 }}
      className={cn(
        // la forma es la del Inicio (no la de la sección): todas las puertas iguales
        'glass group relative flex min-h-36 min-w-0 flex-col gap-3 overflow-hidden rounded-section p-4 text-foreground',
        'focus-visible:outline-2 focus-visible:outline-accent',
        door.size === 'tall' && 'col-span-2 lg:row-span-2 lg:min-h-0',
        door.size === 'wide' && 'lg:col-span-2',
      )}
    >
      {/* la marca del acento, arriba: solo cromo, nunca un dato */}
      <span
        aria-hidden="true"
        className="absolute inset-x-4 top-0 h-0.5 rounded-b-full bg-accent"
      />
      <span className="flex items-start justify-between gap-2">
        <span
          aria-hidden="true"
          className="grid size-10 shrink-0 place-items-center rounded-xl border border-accent/40 bg-accent/10 text-accent"
        >
          <Icon className="size-5" />
        </span>
        <span className="flex items-center gap-1.5">
          <kbd
            aria-hidden="true"
            className="hidden rounded-md border border-border px-1.5 py-0.5 font-mono text-[11px] font-semibold text-muted-foreground sm:inline-block"
          >
            {door.shortcut}
          </kbd>
          <ArrowUpRight
            aria-hidden="true"
            className="size-4 text-muted-foreground transition-transform duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5 group-hover:text-accent motion-reduce:transition-none"
          />
        </span>
      </span>
      <span className="grid gap-1">
        <strong
          className={cn(
            'leading-tight font-extrabold tracking-[-0.02em]',
            door.size === 'tall' ? 'text-[clamp(20px,2vw,26px)]' : 'text-[17px]',
          )}
        >
          {door.title}
        </strong>
        <span className="text-[12.5px] leading-snug text-muted-foreground">{door.description}</span>
      </span>
      {datum && (
        <span className="mt-auto font-mono text-[11.5px] leading-snug text-muted-foreground tabular-nums">
          {datum}
        </span>
      )}
    </motion.a>
  )
}
