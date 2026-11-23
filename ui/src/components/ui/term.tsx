/**
 * Un término técnico que se puede tocar para ver su definición y su ejemplo (fase 10, #171).
 * El texto sale de `docs/GLOSARIO.md` (ver `lib/glossary.ts`). Si el id no existe, queda texto plano.
 */
import { useEffect, useId, useRef, useState, type ReactNode } from 'react'

import { GLOSSARY_BY_ID } from '@/lib/glossary'
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
  const ref = useRef<HTMLSpanElement>(null)

  useEffect(() => {
    if (!open) return
    const close = (e: Event) => {
      if (
        e instanceof KeyboardEvent ? e.key === 'Escape' : !ref.current?.contains(e.target as Node)
      )
        setOpen(false)
    }
    document.addEventListener('pointerdown', close)
    document.addEventListener('keydown', close)
    return () => {
      document.removeEventListener('pointerdown', close)
      document.removeEventListener('keydown', close)
    }
  }, [open])

  if (!entry) return <span className={className}>{children}</span>
  return (
    <span ref={ref} className="relative inline-block">
      <button
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
        <span
          id={popId}
          role="dialog"
          aria-label={entry.title}
          className="absolute top-full left-0 z-40 mt-2 block w-72 border border-[var(--glass-rim)] bg-[var(--card)] shadow-[0_18px_40px_-12px_rgba(0,0,0,0.6)] max-w-[80vw] rounded-2xl p-3.5 text-left text-[12px] leading-relaxed font-normal tracking-normal normal-case"
        >
          <strong className="block text-[13px] font-bold">{entry.title}</strong>
          <span className="mt-1 block whitespace-pre-line text-foreground/90">{entry.body}</span>
          {entry.example && (
            <span className="mt-2 block whitespace-pre-line text-muted-foreground">
              <em className="font-semibold not-italic">Ejemplo: </em>
              {entry.example}
            </span>
          )}
        </span>
      )}
    </span>
  )
}
