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
 *
 * Instrumentos de IBKR (#556): el selector de instrumento suma los de IBKR (`GET /api/v1/ibkr/*`),
 * con sus timeframes (1m, 15m, 1h). Se ven sin capas de liquidez y son solo datos.
 *
 * Cursor sincronizado (#544): opción de la barra de arriba, apagada por defecto. Al mover el
 * cursor en un panel, los otros marcan la vela que contiene esa hora UTC. No sincroniza el zoom.
 */
import { StaleNote } from '@/components/ui/data-honesty'
import { Crosshair } from 'lucide-react'
import { useEffect, useState } from 'react'

import { ChartDrawBar } from '@/components/ui/chart-draw-bar'
import { NO_TOOLS, type ChartTools } from '@/lib/chart-drawings'
import {
  DEFAULT_SLOTS,
  parseSelKey,
  parseSlots,
  selKey,
  slotTimeframes,
  withInstrumentSel,
  useChartCatalog,
  useChartSlot,
  type ChartSel,
  type SlotData,
} from '@/lib/chart-slots'
import { SlotChart } from '@/components/ui/chart-slot'
import { ChartToolsMenu } from '@/components/ui/chart-tools'
import { Segmented } from '@/components/ui/segmented'
import { CursorSync } from '@/lib/cursor-sync'
import { useIbkrInstruments } from '@/lib/ibkr-api'
import type { ChartTimeframe } from '@/lib/liquidity-bars'
import { closedAt, price, priceDigits } from '@/lib/format'
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
/** `sync`: cursor sincronizado entre paneles (#544), apagado por defecto */
type Saved = { layout: Layout; slots: ChartSel[]; active: number; sync: boolean }

function loadSaved(): Saved {
  const fallback: Saved = { layout: '2', slots: DEFAULT_SLOTS, active: 0, sync: false }
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<Saved> | null
    if (!raw || !LAYOUT_IDS.includes(raw.layout as Layout)) return fallback
    const layout = raw.layout as Layout
    const active = Number.isInteger(raw.active) ? Number(raw.active) : 0
    return {
      layout,
      slots: parseSlots(raw.slots),
      active: Math.max(0, Math.min(active, LAYOUTS[layout].count - 1)),
      sync: raw.sync === true,
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

const SOURCE: Record<NonNullable<SlotData['source']>, string> = {
  motor: 'velas del motor',
  ibkr: 'IBKR · solo datos',
  ejemplo: 'datos de ejemplo',
}

export function MultichartView() {
  const [saved, setSaved] = useState(loadSaved)
  useEffect(() => save(saved), [saved])
  const [tools, setTools] = useState<ChartTools[]>(() =>
    Array.from({ length: MAX }, () => NO_TOOLS),
  )
  const [chartTools, toggleChartTool] = useChartTools()
  const { instruments, timeframes } = useChartCatalog()
  const ibkrInstruments = useIbkrInstruments()
  const [cursorHub] = useState(() => new CursorSync())

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
  const drawingsOf = current.view?.instrument_id ?? current.ibkr?.instrument_id ?? sel.instrument
  const brokerKeys =
    instruments.includes(sel.instrument) || sel.feed
      ? instruments
      : [sel.instrument, ...instruments]
  const ibkrKeys = (
    ibkrInstruments.includes(sel.instrument) || !sel.feed
      ? ibkrInstruments
      : [sel.instrument, ...ibkrInstruments]
  ).map((id) => selKey({ instrument: id, feed: 'ibkr' }))
  // en modo compacto, el gráfico activo dibuja sus controles acá, en la barra de arriba (#545)
  const [controlsEl, setControlsEl] = useState<HTMLElement | null>(null)

  const setActive = (i: number) => setSaved((p) => (p.active === i ? p : { ...p, active: i }))
  const setSel = (next: ChartSel) =>
    setSaved((p) => ({
      ...p,
      slots: p.slots.map((s, k) => (k === active ? next : s)),
    }))
  const patchTools = (i: number, patch: Partial<ChartTools>) =>
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
            value={selKey(sel)}
            onChange={(e) => {
              const p = parseSelKey(e.target.value)
              setSel(withInstrumentSel(sel, p.instrument, p.feed))
            }}
            className="h-8 rounded-full border border-border bg-background/60 px-3 text-[12px] font-bold text-foreground focus-visible:outline-2 focus-visible:outline-ring"
          >
            <optgroup label="Broker simulado">
              {brokerKeys.map((id) => (
                <option key={id} value={id}>
                  {id}
                </option>
              ))}
            </optgroup>
            {ibkrKeys.length > 0 && (
              <optgroup label="IBKR · solo datos">
                {ibkrKeys.map((k) => (
                  <option key={k} value={k}>
                    {parseSelKey(k).instrument} · IBKR
                  </option>
                ))}
              </optgroup>
            )}
          </select>
        </label>
        <Segmented
          label="Timeframe del gráfico activo"
          options={slotTimeframes(sel, timeframes)}
          value={sel.timeframe}
          onChange={(tf) => setSel({ ...sel, timeframe: tf as ChartTimeframe })}
        />
        {layout.compact ? (
          <div ref={setControlsEl} data-active-controls className="flex min-w-0" />
        ) : (
          <ChartToolsMenu
            tools={chartTools}
            onToggle={toggleChartTool}
            available={current.view?.analysis != null}
          />
        )}
        <span className="ml-auto" />
        {count > 1 && (
          <button
            type="button"
            aria-pressed={saved.sync}
            onClick={() => setSaved((p) => ({ ...p, sync: !p.sync }))}
            title="Al mover el cursor en un gráfico, los otros marcan la misma hora UTC"
            className={cn(
              'inline-flex h-8 items-center gap-1.5 rounded-full border border-border px-3 text-[12px] font-bold text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring',
              saved.sync &&
                'border-primary bg-primary text-primary-foreground hover:text-primary-foreground',
            )}
          >
            <Crosshair className="size-3.5" aria-hidden />
            Sincronizar cursor
          </button>
        )}
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
        <ChartDrawBar
          label={`Herramientas de dibujo del gráfico ${active + 1}`}
          tools={tools[active]}
          onChange={(patch) => patchTools(active, patch)}
          instrument={drawingsOf}
        />

        <div data-chart-layout={saved.layout} className={cn('grid min-w-0 gap-3', layout.grid)}>
          {slots.slice(0, count).map((slot, i) => (
            <section
              key={i}
              data-chart-slot={i}
              data-active={i === active || undefined}
              aria-current={i === active ? 'true' : undefined}
              aria-label={`Gráfico ${i + 1}: ${saved.slots[i].instrument} ${saved.slots[i].timeframe}`}
              onPointerDownCapture={() => setActive(i)}
              onFocusCapture={() => setActive(i)}
              // el activo se marca con el punto del encabezado (#570), sin borde alrededor del panel
              className={cn(
                'glass relative min-w-0 rounded-[20px] p-3',
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
                  className={cn(
                    'inline-flex shrink-0 items-center gap-1.5 rounded-md text-[13px] font-extrabold tracking-tight transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                    i === active
                      ? 'text-foreground'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  <span
                    aria-hidden
                    data-active-dot
                    className={cn(
                      'size-2 rounded-full transition-colors',
                      i === active
                        ? 'bg-[var(--chart-accent)] shadow-[0_0_0_3px_color-mix(in_srgb,var(--chart-accent)_22%,transparent)]'
                        : 'border border-muted-foreground/50',
                    )}
                  />
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
                syncCursor={saved.sync && count > 1 ? cursorHub : undefined}
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

/** La última vela cerrada del lugar: la vela en formación (#558) no cuenta, como en el gráfico. */
function lastClosed(slot: SlotData) {
  const n = slot.bars.length
  const formingLast = n > 0 && slot.forming?.start === slot.bars[n - 1].start
  return slot.bars.at(formingLast ? -2 : -1)
}

/** Los decimales del instrumento, según sus últimos precios (como en el gráfico). */
const slotDigits = (slot: SlotData) => priceDigits(slot.bars.slice(-50).map((b) => b.close))

/**
 * El encabezado mínimo de un panel en modo compacto (#545): instrumento, timeframe y el cierre de
 * la última vela, tal cual llega del motor (solo con formato de pantalla).
 */
function CompactHeading({ slot }: { slot: SlotData }) {
  const { sel, view, ibkr } = slot
  const last = lastClosed(slot)
  return (
    <p
      data-compact-heading
      className="flex min-w-0 flex-1 items-baseline gap-2 overflow-hidden text-[12px] whitespace-nowrap"
    >
      <span className="truncate font-bold tracking-[0.08em] text-muted-foreground uppercase">
        {view?.instrument_id ?? ibkr?.instrument_id ?? sel.instrument} ·{' '}
        {view?.timeframe ?? ibkr?.timeframe ?? sel.timeframe}
      </span>
      {last && (
        <span className="font-bold tabular-nums" title={`Última vela: ${closedAt(last.end)}`}>
          <span className="sr-only">Último cierre: </span>
          {price(last.close, slotDigits(slot))}
        </span>
      )}
    </p>
  )
}

/** Panel lateral: lo que el motor manda del gráfico activo, tal cual llega. */
function SlotInfo({ index, slot }: { index: number; slot: SlotData }) {
  const { sel, view, ibkr } = slot
  const last = lastClosed(slot)
  const digits = slotDigits(slot)
  const rows: [string, string][] = [
    ['Instrumento', sel.feed === 'ibkr' ? `${sel.instrument} (IBKR)` : sel.instrument],
    ['Timeframe', sel.timeframe],
    ['Fuente', slot.source ? SOURCE[slot.source] : slot.loading ? 'pidiendo…' : 'sin velas'],
  ]
  if (last) {
    rows.push(
      ['Última vela', closedAt(last.end)],
      ['Apertura', price(last.open, digits)],
      ['Máximo', price(last.high, digits)],
      ['Mínimo', price(last.low, digits)],
      ['Cierre', price(last.close, digits)],
    )
  }
  if (view)
    rows.push(
      ['Niveles de liquidez', String(view.levels.length)],
      ['FVG', String(view.fvgs.length)],
    )
  if (ibkr?.series) rows.push(['Contrato', ibkr.series.contract], ['Operable', 'no, solo datos'])
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
      {last && <StaleNote at={last.end} className="mt-2" />}
      <p className="mt-4 text-[11.5px] leading-snug text-muted-foreground">
        Solo para mirar el precio: desde acá no se envían órdenes. Las líneas y mediciones son
        visuales. Lo que muestra el motor es lo que pasó, no una promesa de lo que va a pasar.
      </p>
    </aside>
  )
}
