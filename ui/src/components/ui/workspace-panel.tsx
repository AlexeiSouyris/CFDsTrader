/**
 * Un panel del espacio de trabajo del Inicio (#708): superficie lisa con borde de 1 px, una barra
 * de título fina en mayúsculas y, a la derecha, el enlace a la vista completa con su tecla.
 * Solo presentación: el contenido llega armado.
 */
import { ArrowUpRight } from 'lucide-react'
import type { ReactNode } from 'react'

import { cn } from '@/lib/utils'

export function WorkspacePanel({
  id,
  title,
  open,
  className,
  children,
}: {
  /** id del título: la paleta salta ahí (`Inicio: …`) */
  id: string
  title: string
  /** la vista completa: nombre, hash, tecla (F1–F10) y qué hacer al elegirla */
  open?: { label: string; view: string; hotkey?: string; onOpen: (view: string) => void }
  className?: string
  children: ReactNode
}) {
  return (
    <section
      aria-labelledby={id}
      className={cn('panel flex min-w-0 flex-col rounded-section', className)}
    >
      <header className="flex min-h-8 items-center justify-between gap-3 border-b border-border px-2.5">
        <h2 id={id} className="text-[11px] font-extrabold tracking-[0.12em] uppercase">
          {title}
        </h2>
        {open && (
          <a
            href={`#${open.view}`}
            aria-keyshortcuts={open.hotkey}
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return
              e.preventDefault()
              open.onOpen(open.view)
            }}
            className="inline-flex min-h-7 items-center gap-1.5 text-[11px] font-semibold text-muted-foreground hover:text-foreground"
          >
            {open.label}
            {open.hotkey && (
              <kbd aria-hidden className="hidden text-[10px] font-normal sm:inline">
                {open.hotkey}
              </kbd>
            )}
            <ArrowUpRight aria-hidden className="size-3.5" />
          </a>
        )}
      </header>
      <div className="min-w-0 flex-1 px-2.5 py-1.5">{children}</div>
    </section>
  )
}
