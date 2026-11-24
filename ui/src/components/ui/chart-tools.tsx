/**
 * Menú "Herramientas" del gráfico (#501): prende y apaga las capas de análisis que manda el
 * motor. Cada una lleva su definición de `docs/GLOSARIO.md` (con `Term`). Son herramientas para
 * mirar: no son señales ni recomendaciones.
 */
import { Layers } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'

import { Term } from '@/components/ui/term'
import { TOOLS, type AnalysisTool, type ToolState } from '@/lib/analysis-layers'
import { cn } from '@/lib/utils'

/** ancho del panel (`w-72`), para saber hacia dónde abrirlo */
const PANEL_W = 288

export function ChartToolsMenu({
  tools,
  onToggle,
  available,
}: {
  tools: ToolState
  onToggle: (id: AnalysisTool, on: boolean) => void
  /** si el gráfico trae el bloque `analysis` del motor */
  available: boolean
}) {
  const [open, setOpen] = useState(false)
  /** el panel se abre hacia la izquierda del botón (como en escritorio) o, si ahí no entra, hacia la derecha */
  const [alignRight, setAlignRight] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const panelId = useId()
  const on = TOOLS.filter((t) => tools[t.id]).length

  useEffect(() => {
    if (!open) return
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent) {
        if (e.key === 'Escape') setOpen(false)
      } else if (!ref.current?.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('pointerdown', close)
    document.addEventListener('keydown', close)
    return () => {
      document.removeEventListener('pointerdown', close)
      document.removeEventListener('keydown', close)
    }
  }, [open])

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => {
          const box = ref.current?.getBoundingClientRect()
          setAlignRight(box ? box.right - PANEL_W >= 8 : true)
          setOpen((o) => !o)
        }}
        className={cn(
          'inline-flex h-8 items-center gap-1.5 rounded-full border border-border bg-background/60 px-3 text-[12px] font-bold text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring',
          open && 'text-foreground',
        )}
      >
        <Layers className="size-3.5" aria-hidden />
        Herramientas
        {on > 0 && (
          <span className="rounded-full bg-foreground/10 px-1.5 text-[10.5px] tabular-nums">
            {on}
          </span>
        )}
      </button>
      {open && (
        <div
          id={panelId}
          role="dialog"
          aria-label="Herramientas de análisis"
          className={cn(
            'absolute top-full z-40 mt-2 w-72 max-w-[calc(100vw-2.5rem)] rounded-2xl border border-[var(--glass-rim)] bg-[var(--card)] p-3 text-left shadow-[0_18px_40px_-12px_rgba(0,0,0,0.6)]',
            alignRight ? 'right-0' : 'left-0',
          )}
        >
          <p className="mb-2 text-[11.5px] leading-snug text-muted-foreground">
            Capas para mirar el precio, calculadas por el motor. No son señales ni recomendaciones.
            Tocá cada nombre para ver su definición.
          </p>
          {!available && (
            <p role="note" className="mb-2 text-[11.5px] leading-snug text-muted-foreground">
              Este gráfico no las trae: el motor las manda con las velas de 15 m del CFD de IG.
            </p>
          )}
          <ul className="grid gap-1">
            {TOOLS.map((t) => (
              <li key={t.id} className="flex items-center gap-2 text-[12.5px]">
                <input
                  id={`${panelId}-${t.id}`}
                  type="checkbox"
                  checked={tools[t.id]}
                  onChange={(e) => onToggle(t.id, e.target.checked)}
                  className="size-4 accent-[var(--primary)]"
                />
                <label htmlFor={`${panelId}-${t.id}`} className="sr-only">
                  {t.label}
                </label>
                <Term id={t.term} className="text-left">
                  {t.label}
                </Term>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}
