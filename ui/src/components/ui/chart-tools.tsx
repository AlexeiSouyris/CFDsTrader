/**
 * Menú "Herramientas" del gráfico (#501): prende y apaga las capas de análisis que manda el
 * motor. Cada una lleva su definición de `docs/GLOSARIO.md` (con `Term`). Son herramientas para
 * mirar: no son señales ni recomendaciones.
 */
import { Layers } from 'lucide-react'
import { useId, useRef, useState } from 'react'

import { Floating } from '@/components/ui/floating'
import { Term } from '@/components/ui/term'
import { TOOLS, type AnalysisTool, type ToolState } from '@/lib/analysis-layers'
import { useDismiss } from '@/lib/floating'
import { cn } from '@/lib/utils'

const NOT_AVAILABLE =
  'Este gráfico no trae herramientas de análisis: por ahora solo las tienen las velas de 15 minutos de los CFD de IG.'

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
  const ref = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const panelId = useId()
  // sin capas (#580): el botón se ve deshabilitado, sin contador, y al abrirlo explica por qué
  const on = available ? TOOLS.filter((t) => tools[t.id]).length : 0
  const noteId = `${panelId}-note`
  // el panel va en una capa sobre todo (#570): si no, el gráfico de abajo lo tapa
  useDismiss(open, () => setOpen(false), [ref, panelRef], ref)

  return (
    <>
      <button
        ref={ref}
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        aria-disabled={!available || undefined}
        title={available ? undefined : NOT_AVAILABLE}
        onClick={() => setOpen((o) => !o)}
        className={cn(
          'inline-flex h-8 items-center gap-1.5 rounded-full border border-border bg-background/60 px-3 text-[12px] font-bold text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring',
          open && 'text-foreground',
          !available && 'border-dashed opacity-60',
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
        <Floating
          anchor={ref}
          panelRef={panelRef}
          align="end"
          id={panelId}
          role="dialog"
          aria-label="Herramientas de análisis"
          className="w-72 max-w-[calc(100vw-1rem)] rounded-2xl border border-[var(--glass-rim)] bg-[var(--card)] p-3 text-left shadow-[0_18px_40px_-12px_rgba(0,0,0,0.6)]"
        >
          <p className="mb-2 text-[11.5px] leading-snug text-muted-foreground">
            Capas para mirar el precio, calculadas por el motor. No son señales ni recomendaciones.
            Tocá cada nombre para ver su definición.
          </p>
          {!available && (
            <p
              id={noteId}
              role="note"
              className="mb-2 rounded-lg border border-dashed border-border px-2 py-1.5 text-[11.5px] leading-snug text-foreground"
            >
              {NOT_AVAILABLE}
            </p>
          )}
          <ul className="grid gap-1">
            {TOOLS.map((t) => (
              <li key={t.id} className="flex items-center gap-2 text-[12.5px]">
                <input
                  id={`${panelId}-${t.id}`}
                  type="checkbox"
                  checked={available && tools[t.id]}
                  disabled={!available}
                  aria-describedby={available ? undefined : noteId}
                  onChange={(e) => onToggle(t.id, e.target.checked)}
                  className="size-4 accent-[var(--primary)] disabled:opacity-50"
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
        </Floating>
      )}
    </>
  )
}
