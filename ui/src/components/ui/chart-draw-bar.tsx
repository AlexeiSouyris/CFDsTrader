/**
 * La barra de dibujo de un gráfico (#540, #543): navegar, medir, línea horizontal, línea de
 * tendencia, rectángulo, velas o línea, y borrar los dibujos del instrumento. La comparten el
 * Multigráfico y la Terminal (#552) y maneja las herramientas del gráfico activo. Todo es visual:
 * no genera señales ni órdenes. Medir y dibujar llevan su candado de la Academia (#827): bloqueados,
 * el botón lleva a la lección que los abre.
 */
import {
  CandlestickChart,
  LineChart,
  Minus,
  MousePointer2,
  RectangleHorizontal,
  Ruler,
  Slash,
  Trash2,
  type LucideIcon,
} from 'lucide-react'

import { ToolLockButton } from '@/components/ui/tool-lock'
import { DRAW_LABEL, drawingStore, useDrawings, type ChartTools } from '@/lib/chart-drawings'
import { cn } from '@/lib/utils'

const DRAW: {
  id: string
  label: string
  icon: LucideIcon
  on: (t: ChartTools) => boolean
  set: Partial<ChartTools>
  /** el id de su candado en la Academia, si tiene */
  tool?: string
}[] = [
  {
    id: 'nav',
    label: 'Navegar',
    icon: MousePointer2,
    on: (t) => !t.ruler && t.draw === 'none',
    set: { ruler: false, draw: 'none' },
  },
  {
    id: 'ruler',
    label: 'Medir',
    icon: Ruler,
    on: (t) => t.ruler,
    set: { ruler: true, draw: 'none' },
    tool: 'tool.ruler',
  },
  ...(
    [
      ['hline', Minus],
      ['trend', Slash],
      ['rect', RectangleHorizontal],
    ] as const
  ).map(([kind, icon]) => ({
    id: kind,
    label: DRAW_LABEL[kind],
    icon,
    on: (t: ChartTools) => !t.ruler && t.draw === kind,
    set: { ruler: false, draw: kind },
    tool: 'tool.drawing',
  })),
  {
    id: 'candles',
    label: 'Velas',
    icon: CandlestickChart,
    on: (t) => t.mode === 'Velas',
    set: { mode: 'Velas' },
  },
  {
    id: 'line',
    label: 'Línea',
    icon: LineChart,
    on: (t) => t.mode === 'Línea',
    set: { mode: 'Línea' },
  },
]

/**
 * Vertical desde `lg` (al costado del gráfico) y horizontal en pantallas chicas. `instrument` es
 * el del gráfico activo: el botón de la papelera borra sus dibujos.
 */
export function ChartDrawBar({
  label,
  tools,
  onChange,
  instrument,
  className,
}: {
  label: string
  tools: ChartTools
  onChange: (patch: Partial<ChartTools>) => void
  instrument: string
  className?: string
}) {
  const drawings = useDrawings(instrument)
  return (
    <div
      role="toolbar"
      aria-label={label}
      aria-orientation="vertical"
      className={cn(
        'flex flex-wrap gap-1 self-start rounded-section border border-border bg-card p-1.5 lg:sticky lg:top-4 lg:flex-col',
        className,
      )}
    >
      {DRAW.map((d, i) => {
        const on = d.on(tools)
        const Icon = d.icon
        const shape = cn(
          'grid size-11 place-items-center rounded-xl text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring',
          (i === 2 || i === 5) && 'max-lg:ml-2 lg:mt-2',
        )
        const button = (
          <button
            key={d.id}
            type="button"
            aria-pressed={on}
            aria-label={d.label}
            title={d.label}
            onClick={() => onChange(d.set)}
            className={cn(
              shape,
              on && 'bg-primary text-primary-foreground hover:text-primary-foreground',
            )}
          >
            <Icon className="size-4" aria-hidden />
          </button>
        )
        if (!d.tool) return button
        return (
          <ToolLockButton
            key={d.id}
            tool={d.tool}
            className={cn(shape, 'flex items-center justify-center')}
            face={<Icon className="size-4" aria-hidden />}
          >
            {button}
          </ToolLockButton>
        )
      })}
      <button
        type="button"
        aria-label={`Borrar todos los dibujos de ${instrument}`}
        title={`Borrar todos los dibujos de ${instrument}`}
        disabled={drawings.length === 0}
        onClick={() => drawingStore.clear(instrument)}
        className="grid size-11 place-items-center rounded-xl text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring disabled:opacity-40 disabled:hover:text-muted-foreground max-lg:ml-2 lg:mt-2"
      >
        <Trash2 className="size-4" aria-hidden />
      </button>
    </div>
  )
}
