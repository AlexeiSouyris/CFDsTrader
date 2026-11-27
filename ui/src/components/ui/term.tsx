/**
 * Un término técnico que se puede tocar para ver su definición y su ejemplo (fase 10, #171).
 * El texto sale de `docs/GLOSARIO.md` (ver `lib/glossary.ts`). Si el id no existe, queda texto plano.
 */
import { useId, useRef, useState, type ReactNode } from 'react'

import { Floating } from '@/components/ui/floating'
import { GLOSSARY_BY_ID } from '@/lib/glossary'
import { useDismiss } from '@/lib/floating'
import { cn } from '@/lib/utils'

export function Term({
  id,
  children,
  className,
}: {
  id: string
  children?: ReactNode
  className?: string
}) {
  const entry = GLOSSARY_BY_ID.get(id)
  const [open, setOpen] = useState(false)
  const popId = useId()
  const ref = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLSpanElement>(null)
  // la definición va en una capa sobre todo (#570): dentro de una tarjeta, la de abajo la tapaba
  useDismiss(open, () => setOpen(false), [ref, panelRef], ref)

  if (!entry) return <span className={className}>{children}</span>
  return (
    <span className="relative inline-block">
      <button
        ref={ref}
        type="button"
        data-inline-hit
        aria-expanded={open}
        aria-controls={popId}
        onClick={() => setOpen((o) => !o)}
        className={cn(
          'cursor-help [letter-spacing:inherit] [text-transform:inherit] underline decoration-dotted decoration-from-font underline-offset-[3px] hover:decoration-solid',
          className,
        )}
      >
        {children ?? entry.title}
      </button>
      {open && (
        <Floating
          as="span"
          anchor={ref}
          panelRef={panelRef}
          id={popId}
          role="dialog"
          aria-label={entry.title}
          className="block w-72 panel-raised max-w-[80vw] rounded-2xl p-3.5 text-left text-[12px] leading-relaxed font-normal tracking-normal normal-case"
        >
          <strong className="block text-[13px] font-bold">{entry.title}</strong>
          <span className="mt-1 block whitespace-pre-line text-foreground/90">{entry.body}</span>
          {entry.example && (
            <span className="mt-2 block whitespace-pre-line text-muted-foreground">
              <em className="font-semibold not-italic">Ejemplo: </em>
              {entry.example}
            </span>
          )}
        </Floating>
      )}
    </span>
  )
}
