/**
 * Multigráfico (#540): hasta cuatro gráficos a la vez, con una barra de dibujo a la izquierda, una
 * barra superior (instrumento, timeframe, indicadores y distribución), los gráficos al centro y un
 * panel de información a la derecha. Diseño propio, con el estilo de la UI.
 *
 * Reúsa el gráfico de la app (`LiquidityChart` vía `chart-slot.tsx`): las velas, niveles, FVG y
 * las herramientas de análisis llegan del motor (`liquidity_view.v1`). La barra de dibujo maneja
 * las herramientas que el gráfico ya tiene (navegar, medir, velas o línea) en el gráfico activo:
 * son solo visuales. Acá no se calcula nada de trading ni hay botones de órdenes.
 *
 * Distribuciones: 1, 2 lado a lado (la de 2 por defecto), 2 apilados (solo si se elige) y 4 en
 * una cuadrícula de 2 × 2. En el celular se ve un gráfico por vez, con un selector. La distribución,
 * los instrumentos y el gráfico activo se recuerdan en el navegador.
 *
 * Modo compacto (#545): con más de un gráfico, cada panel muestra solo el gráfico con un
 * encabezado mínimo (instrumento, timeframe y último cierre tal cual llega del motor), y los
 * controles del gráfico activo (niveles, rango y herramientas) pasan a la barra de arriba. Con un
 * solo gráfico hay lugar de sobra: se ve completo, con su cabecera, controles y leyenda.
 *
 * Dibujos (#543): la barra de dibujo suma línea horizontal, línea de tendencia y rectángulo sobre
 * el gráfico activo, y un botón para borrar todos los dibujos de su instrumento. Se guardan en el
 * navegador por instrumento y son solo visuales: no generan señales ni órdenes.
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
import { useEffect, useState } from 'react'

import type { ChartMode } from '@/components/ui/chart-measure'
import {
  DEFAULT_SLOTS,
  parseSlots,
  useChartCatalog,
  useChartSlot,
  type ChartSel,
  type SlotData,
} from '@/lib/chart-slots'
import { SlotChart } from '@/components/ui/chart-slot'
import { ChartToolsMenu } from '@/components/ui/chart-tools'
import { Segmented } from '@/components/ui/segmented'
import { chartBars } from '@/lib/liquidity-bars'
import { DRAW_LABEL, drawingStore, useDrawings, type DrawTool } from '@/lib/chart-drawings'
import type { ViewTimeframe } from '@/lib/liquidity-contracts'
import { dateTimeUtc, decimal } from '@/lib/format'
import { useChartTools } from '@/lib/use-chart-tools'
import { cn } from '@/lib/utils'

/** Distribuciones: la etiqueta del selector, cuántos gráficos y cómo se ubican en escritorio. */
/** `compact`: los paneles sin cabecera ni controles propios (#545), por eso son más altos. */
const LAYOUTS = {
  '1': { label: '1', count: 1, grid: '', height: 520, compact: false },
  '2': { label: '2 lado a lado', count: 2, grid: 'lg:grid-cols-2', height: 500, compact: true },
  '2v': { label: '2 apilados', count: 2, grid: '', height: 340, compact: true },
  '4': { label: '4', count: 4, grid: 'lg:grid-cols-2', height: 340, compact: true },
} as const
type Layout = keyof typeof LAYOUTS
/** orden del selector: la de 2 apilados queda al final, como opción */
const LAYOUT_IDS: Layout[] = ['1', '2', '4', '2v']
const MAX = 4

const STORAGE_KEY = 'cfdstrader.multichart'
type Saved = { layout: Layout; slots: ChartSel[]; active: number }

function loadSaved(): Saved {
  const fallback: Saved = { layout: '2', slots: DEFAULT_SLOTS, active: 0 }
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<Saved> | null
    if (!raw || !LAYOUT_IDS.includes(raw.layout as Layout)) return fallback
    const layout = raw.layout as Layout
    const active = Number.isInteger(raw.active) ? Number(raw.active) : 0
    return {
      layout,
      slots: parseSlots(raw.slots),
      active: Math.max(0, Math.min(active, LAYOUTS[layout].count - 1)),
    }
  } catch {
    return fallback
  }
}

function save(saved: Saved) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(saved))
  } catch {
    // modo privado o almacenamiento bloqueado: anda igual, solo no se recuerda
  }
}

type Tools = { ruler: boolean; draw: DrawTool; mode: ChartMode }
const NO_TOOLS: Tools = { ruler: false, draw: 'none', mode: 'Velas' }

const DRAW: {
  id: string
  label: string
  icon: LucideIcon
  on: (t: Tools) => boolean
  set: Partial<Tools>
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
    on: (t: Tools) => !t.ruler && t.draw === kind,
    set: { ruler: false, draw: kind },
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

const SOURCE: Record<NonNullable<SlotData['source']>, string> = {
  motor: 'velas del motor',
  ejemplo: 'datos de ejemplo',
}

export function MultichartView() {
  const [saved, setSaved] = useState(loadSaved)
  useEffect(() => save(saved), [saved])
  const [tools, setTools] = useState<Tools[]>(() => Array.from({ length: MAX }, () => NO_TOOLS))
  const [chartTools, toggleChartTool] = useChartTools()
  const { instruments, timeframes } = useChartCatalog()

  const layout = LAYOUTS[saved.layout]
  const count = layout.count
  const active = Math.min(saved.active, count - 1)
  // una llamada por lugar (siempre cuatro): los que la distribución no muestra no piden nada
  const s0 = useChartSlot(saved.slots[0])
  const s1 = useChartSlot(saved.slots[1], count > 1)
  const s2 = useChartSlot(saved.slots[2], count > 2)
  const s3 = useChartSlot(saved.slots[3], count > 3)
  const slots = [s0, s1, s2, s3]
  const current = slots[active]
  const sel = saved.slots[active]
  const drawingsOf = current.data?.instrument_id ?? sel.instrument
  const activeDrawings = useDrawings(drawingsOf)
  // en modo compacto, el gráfico activo dibuja sus controles acá, en la barra de arriba (#545)
  const [controlsEl, setControlsEl] = useState<HTMLElement | null>(null)

  const setActive = (i: number) => setSaved((p) => (p.active === i ? p : { ...p, active: i }))
  const patchSel = (patch: Partial<ChartSel>) =>
    setSaved((p) => ({
      ...p,
      slots: p.slots.map((s, k) => (k === active ? { ...s, ...patch } : s)),
    }))
  const patchTools = (i: number, patch: Partial<Tools>) =>
    setTools((all) => all.map((t, k) => (k === i ? { ...t, ...patch } : t)))

  return (
    <div className="grid min-w-0 gap-3" data-multichart={saved.layout}>
      <div
        role="toolbar"
        aria-label="Gráfico activo"
        className="glass flex min-w-0 flex-wrap items-center gap-2 rounded-[20px] p-3"
      >
        <span className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
          Gráfico {active + 1}
        </span>
        <label className="flex items-center gap-1.5 text-[11px] font-bold text-muted-foreground">
          <span className="sr-only">Instrumento del gráfico activo</span>
          <select
            value={sel.instrument}
            onChange={(e) => patchSel({ instrument: e.target.value })}
            className="h-8 rounded-full border border-border bg-background/60 px-3 text-[12px] font-bold text-foreground focus-visible:outline-2 focus-visible:outline-ring"
          >
            {(instruments.includes(sel.instrument)
              ? instruments
              : [sel.instrument, ...instruments]
            ).map((id) => (
              <option key={id} value={id}>
                {id}
              </option>
            ))}
          </select>
        </label>
        <Segmented
          label="Timeframe del gráfico activo"
          options={timeframes}
          value={sel.timeframe}
          onChange={(tf) => patchSel({ timeframe: tf as ViewTimeframe })}
        />
        {layout.compact ? (
          <div ref={setControlsEl} data-active-controls className="flex min-w-0" />
        ) : (
          <ChartToolsMenu
            tools={chartTools}
            onToggle={toggleChartTool}
            available={current.data?.analysis != null}
          />
        )}
        <span className="ml-auto" />
        <Segmented
          label="Distribución"
          options={LAYOUT_IDS.map((id) => LAYOUTS[id].label)}
          value={layout.label}
          onChange={(v) => {
            const id = LAYOUT_IDS.find((k) => LAYOUTS[k].label === v) ?? '1'
            setSaved((p) => ({ ...p, layout: id }))
          }}
        />
      </div>

      {count > 1 && (
        <div className="flex items-center gap-2 lg:hidden" data-chart-switcher>
          <span className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
            Gráfico
          </span>
          <Segmented
            label="Gráfico a la vista"
            options={Array.from({ length: count }, (_, i) => String(i + 1))}
            value={String(active + 1)}
            onChange={(v) => setActive(Number(v) - 1)}
          />
        </div>
      )}

      <div className="grid min-w-0 gap-3 lg:grid-cols-[auto_minmax(0,1fr)] xl:grid-cols-[auto_minmax(0,1fr)_260px]">
        <div
          role="toolbar"
          aria-label={`Herramientas de dibujo del gráfico ${active + 1}`}
          aria-orientation="vertical"
          className="glass flex flex-wrap gap-1 self-start rounded-[20px] p-1.5 lg:sticky lg:top-4 lg:flex-col"
        >
          {DRAW.map((d, i) => {
            const on = d.on(tools[active])
            const Icon = d.icon
            return (
              <button
                key={d.id}
                type="button"
                aria-pressed={on}
                aria-label={d.label}
                title={d.label}
                onClick={() => patchTools(active, d.set)}
                className={cn(
                  'grid size-11 place-items-center rounded-xl text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring',
                  on && 'bg-primary text-primary-foreground hover:text-primary-foreground',
                  (i === 2 || i === 5) && 'max-lg:ml-2 lg:mt-2',
                )}
              >
                <Icon className="size-4" aria-hidden />
              </button>
            )
          })}
          <button
            type="button"
            aria-label={`Borrar todos los dibujos de ${drawingsOf}`}
            title={`Borrar todos los dibujos de ${drawingsOf}`}
            disabled={activeDrawings.length === 0}
            onClick={() => drawingStore.clear(drawingsOf)}
            className="grid size-11 place-items-center rounded-xl text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring disabled:opacity-40 disabled:hover:text-muted-foreground max-lg:ml-2 lg:mt-2"
          >
            <Trash2 className="size-4" aria-hidden />
          </button>
        </div>

        <div data-chart-layout={saved.layout} className={cn('grid min-w-0 gap-3', layout.grid)}>
          {slots.slice(0, count).map((slot, i) => (
            <section
              key={i}
              data-chart-slot={i}
              data-active={i === active || undefined}
              aria-label={`Gráfico ${i + 1}: ${saved.slots[i].instrument} ${saved.slots[i].timeframe}`}
              onPointerDownCapture={() => setActive(i)}
              onFocusCapture={() => setActive(i)}
              className={cn(
                'glass relative min-w-0 rounded-[20px] p-3 ring-offset-0 transition-shadow has-[[role=dialog]]:z-30',
                i === active ? 'ring-2 ring-primary' : 'ring-1 ring-transparent',
                i !== active && 'max-lg:hidden',
              )}
            >
              <header
                className={cn(
                  'flex min-w-0 items-baseline justify-between gap-2',
                  layout.compact ? 'mb-1' : 'mb-2',
                )}
              >
                <button
                  type="button"
                  aria-pressed={i === active}
                  onClick={() => setActive(i)}
                  className="shrink-0 rounded-md text-[13px] font-extrabold tracking-tight focus-visible:outline-2 focus-visible:outline-ring"
                >
                  Gráfico {i + 1}
                  <span className="sr-only">{i === active ? ' (activo)' : ', activar'}</span>
                </button>
                {layout.compact && <CompactHeading slot={slot} />}
                {slot.source && (
                  // en compacto y en el celular, la fuente queda en el panel de información
                  <span
                    className={cn(
                      'shrink-0 text-[11px] text-muted-foreground',
                      layout.compact && 'max-sm:hidden',
                    )}
                  >
                    {SOURCE[slot.source]}
                  </span>
                )}
              </header>
              <SlotChart
                slot={slot}
                height={layout.height}
                ruler={tools[i].ruler}
                onRuler={(ruler) => patchTools(i, ruler ? { ruler, draw: 'none' } : { ruler })}
                draw={tools[i].draw}
                onDraw={(draw) => patchTools(i, { draw })}
                mode={tools[i].mode}
                onMode={(mode) => patchTools(i, { mode })}
                compact={layout.compact}
                controlsTarget={i === active ? controlsEl : null}
              />
            </section>
          ))}
        </div>

        <SlotInfo index={active} slot={current} />
      </div>
    </div>
  )
}

/**
 * El encabezado mínimo de un panel en modo compacto (#545): instrumento, timeframe y el cierre de
 * la última vela, tal cual llega del motor (solo con formato de pantalla).
 */
function CompactHeading({ slot }: { slot: SlotData }) {
  const { sel, data } = slot
  const last = data ? chartBars(data).at(-1) : undefined
  return (
    <p
      data-compact-heading
      className="flex min-w-0 flex-1 items-baseline gap-2 overflow-hidden text-[12px] whitespace-nowrap"
    >
      <span className="truncate font-bold tracking-[0.08em] text-muted-foreground uppercase">
        {data?.instrument_id ?? sel.instrument} · {data?.timeframe ?? sel.timeframe}
      </span>
      {last && (
        <span
          className="font-bold tabular-nums"
          title={`Última vela cerrada: ${dateTimeUtc(last.end)}`}
        >
          <span className="sr-only">Último cierre: </span>
          {decimal(last.close)}
        </span>
      )}
    </p>
  )
}

/** Panel lateral: lo que el motor manda del gráfico activo, tal cual llega. */
function SlotInfo({ index, slot }: { index: number; slot: SlotData }) {
  const { sel, data } = slot
  const last = data ? chartBars(data).at(-1) : undefined
  const rows: [string, string][] = [
    ['Instrumento', sel.instrument],
    ['Timeframe', sel.timeframe],
    ['Fuente', slot.source ? SOURCE[slot.source] : slot.loading ? 'pidiendo…' : 'sin velas'],
  ]
  if (last) {
    rows.push(
      ['Última vela cerrada', dateTimeUtc(last.end)],
      ['Apertura', decimal(last.open)],
      ['Máximo', decimal(last.high)],
      ['Mínimo', decimal(last.low)],
      ['Cierre', decimal(last.close)],
    )
  }
  if (data)
    rows.push(
      ['Niveles de liquidez', String(data.levels.length)],
      ['FVG', String(data.fvgs.length)],
    )
  return (
    <aside
      aria-label={`Información del gráfico ${index + 1}`}
      className="glass min-w-0 self-start rounded-[20px] p-4 lg:col-span-2 xl:col-span-1"
    >
      <h2 className="mb-3 text-[14px] font-bold">Información · gráfico {index + 1}</h2>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-[12.5px]">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-muted-foreground">{k}</dt>
            <dd className="text-right font-semibold tabular-nums">{v}</dd>
          </div>
        ))}
      </dl>
      <p className="mt-4 text-[11.5px] leading-snug text-muted-foreground">
        Solo para mirar el precio: desde acá no se envían órdenes. Las líneas y mediciones son
        visuales. Lo que muestra el motor es lo que pasó, no una promesa de lo que va a pasar.
      </p>
    </aside>
  )
}
