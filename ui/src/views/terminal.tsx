/**
 * Terminal de trading (#552, antes el prototipo de la fase 10, #275). **Solo simulación (paper)**:
 * no se envía ninguna orden real. Solo ubica componentes y les pasa lo que manda el motor; no
 * calcula riesgo, P/L, margen ni probabilidades (CLAUDE.md §1).
 *
 * Disposición, como una terminal de trading:
 * - arriba, la cuenta de simulación (`paper_account.v1`) con el modo simulación siempre a la vista,
 *   y el estado del broker del motor (`ops_status.v1`: conexión, kill switch, pérdida del día);
 * - a la izquierda, la lista de mercados (`lib/market-summary.ts`), en dos grupos: los del broker
 *   simulado (último cierre que sirve el gateway) y los de IBKR (#556: último, variación, hora y
 *   atraso del gateway). Los de IBKR abren el gráfico con sus velas, pero son **solo datos**: el
 *   ticket no los acepta;
 * - al centro, 1, 2 o 4 gráficos (`LiquidityChart`, #286) con la barra de dibujo compartida con el
 *   Multigráfico (#543) sobre el gráfico activo;
 * - a la derecha, el ticket de siempre: validación del riesgo del motor (`ticket_verdict.v1`) y
 *   envío al broker simulado (`order.v1` → `order_report.v1`);
 * - abajo, pestañas: Posiciones (`position.v1`), Órdenes (las de esta pestaña: el gateway no las
 *   lista), Historial (sin ruta todavía: se dice), Actividad (`ops_event.v1`) y las propuestas
 *   de ejemplo (`proposal.v1`).
 * En el celular, Gráfico, Mercados y Ticket se ven de a uno, con un selector.
 *
 * Estilo (#581, `data-section="terminal"`): casi negra y siempre oscura, sin vidrio, paneles
 * separados por líneas de 1 px, números en mono con cifras tabulares y el ámbar solo en los
 * controles. En la pantalla ancha, la lista de mercados y el ticket se agrandan o achican con los
 * divisores (mouse o teclado) y sus anchos quedan en `localStorage`.
 *
 * Si el gateway no responde, se ven los datos de ejemplo de `mocks/terminal.ts` y no se envía
 * nada. La disposición de los gráficos queda en `localStorage`.
 *
 * El ticket (#578) arranca vacío y sigue al gráfico activo cuando su instrumento es operable: lo
 * conoce el motor (catálogo `/api/v1/instruments`) y tiene velas. Ofrece solo esos instrumentos; los
 * del catálogo sin velas se ven deshabilitados. "Simulación" queda fijo arriba en una franja angosta
 * y opaca que no tapa el contenido al quedar quieta.
 */
import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'

import { ChartDrawBar } from '@/components/ui/chart-draw-bar'
import { NO_TOOLS, type ChartTools } from '@/lib/chart-drawings'
import { SlotChart } from '@/components/ui/chart-slot'
import { PaneResizer } from '@/components/ui/pane-resizer'
import { OpsEventFeed, OpsKillSwitchBanner, OpsStatusBar } from '@/components/ui/ops-status'
import { ProposalCard } from '@/components/ui/proposal-card'
import { Segmented } from '@/components/ui/segmented'
import { SessionOrders, type SentOrder } from '@/components/ui/session-orders'
import {
  AccountStrip,
  KillSwitchBanner,
  OrderTicket,
  PositionsTable,
} from '@/components/ui/trading-terminal'
import { Watchlist } from '@/components/ui/watchlist'
import {
  DEFAULT_SLOTS,
  parseSelKey,
  parseSlots,
  selKey,
  slotTimeframes,
  useChartCatalog,
  useChartSlot,
  withInstrumentSel,
  type ChartSel,
} from '@/lib/chart-slots'
import { signed } from '@/lib/format'
import { useInstruments } from '@/lib/liquidity-api'
import type { ChartTimeframe } from '@/lib/liquidity-bars'
import type { ViewTimeframe } from '@/lib/liquidity-contracts'
import { ibkrSummaries, useMarketSummaries, type MarketSummary } from '@/lib/market-summary'
import { useOpsEvents, useOpsStatus } from '@/lib/ops-api'
import { previewOrder, submitOrder, usePaperAccount, usePaperPositions } from '@/lib/paper-api'
import type { PaperAccount } from '@/lib/paper-contracts'
import {
  clearFocusedInstrument,
  clearFocusedTimeframe,
  FOCUS_EVENT,
  focusedInstrument,
  focusedTimeframe,
  TIMEFRAME_EVENT,
} from '@/lib/terminal-focus'
import { cn } from '@/lib/utils'
import {
  mockPreview,
  PAPER_ACCOUNT,
  PAPER_ACCOUNT_HALTED,
  PAPER_POSITIONS,
  PROPOSALS,
  RESPONSIBLE_ID,
  TERMINAL_INSTRUMENTS,
  TICKET_EXAMPLES,
  type Member,
} from '@/mocks'

function Panel({
  title,
  aside,
  children,
  className,
}: {
  title: ReactNode
  aside?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <section className={cn('relative min-w-0 bg-section p-density', className)}>
      <header className="mb-2 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <h2 className="text-[11px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
          {title}
        </h2>
        {aside && <span className="text-[10.5px] text-muted-foreground">{aside}</span>}
      </header>
      {children}
    </section>
  )
}

const SCENARIOS = ['Normal', 'Kill switch activo'] as const

const LAYOUTS = ['1', '2', '4'] as const
type Layout = (typeof LAYOUTS)[number]
const STORAGE_KEY = 'cfdstrader.terminal.charts'
const CHART_HEIGHT: Record<Layout, number> = { '1': 460, '2': 380, '4': 300 }

type Saved = { layout: Layout; slots: ChartSel[] }

/** Lee la disposición guardada; si no hay, está rota o el navegador no deja leer, la de siempre. */
function loadLayout(): Saved {
  const fallback: Saved = { layout: '1', slots: DEFAULT_SLOTS }
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<Saved> | null
    if (!raw || !LAYOUTS.includes(raw.layout as Layout)) return fallback
    return { layout: raw.layout as Layout, slots: parseSlots(raw.slots) }
  } catch {
    return fallback
  }
}

/**
 * El instrumento pedido (buscador #362 o lista de mercados) va al gráfico `index`; `feed: 'ibkr'`
 * si es de IBKR (#556).
 */
const withInstrument = (saved: Saved, id: string | null, index = 0, feed?: 'ibkr'): Saved =>
  id
    ? {
        ...saved,
        slots: saved.slots.map((s, i) => (i === index ? withInstrumentSel(s, id, feed) : s)),
      }
    : saved

/** "~15 min de atraso", tal como lo declara la fuente, o nada. */
const delayText = (row: MarketSummary | undefined) =>
  row?.delay_s != null ? `~${Math.max(1, Math.round(row.delay_s / 60))} min de atraso` : null

function saveLayout(saved: Saved) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(saved))
  } catch {
    // modo privado o almacenamiento bloqueado: la grilla anda igual, solo no se recuerda
  }
}

/** Anchos de la lista de mercados y del ticket (px), con sus límites (#581). */
const PANES_KEY = 'cfdstrader.terminal.panes'
const WIDTH = {
  left: { initial: 200, min: 160, max: 360 },
  right: { initial: 320, min: 272, max: 480 },
} as const
type Widths = { left: number; right: number }

const clampWidth = (side: keyof Widths, px: unknown) => {
  const { initial, min, max } = WIDTH[side]
  return typeof px === 'number' && Number.isFinite(px)
    ? Math.round(Math.min(max, Math.max(min, px)))
    : initial
}

/** Lee los anchos guardados; si no hay, están rotos o el navegador no deja leer, los de siempre. */
function loadWidths(): Widths {
  try {
    const raw = JSON.parse(localStorage.getItem(PANES_KEY) ?? 'null') as Partial<Widths> | null
    return { left: clampWidth('left', raw?.left), right: clampWidth('right', raw?.right) }
  } catch {
    return { left: WIDTH.left.initial, right: WIDTH.right.initial }
  }
}

function saveWidths(w: Widths) {
  try {
    localStorage.setItem(PANES_KEY, JSON.stringify(w))
  } catch {
    // sin almacenamiento: los divisores andan igual, solo no se recuerdan
  }
}

function ChartSlot({
  index,
  sel,
  onChange,
  instruments,
  ibkrInstruments,
  ibkrDelay,
  timeframes,
  height,
  active,
  tools,
  onTools,
}: {
  index: number
  sel: ChartSel
  onChange: (sel: ChartSel) => void
  instruments: string[]
  /** los de IBKR (solo datos), para el mismo selector */
  ibkrInstruments: string[]
  /** el atraso de IBKR de este instrumento, como lo dice la lista de mercados */
  ibkrDelay: string | null
  timeframes: ViewTimeframe[]
  height: number
  active: boolean
  tools: ChartTools
  onTools: (patch: Partial<ChartTools>) => void
}) {
  const slot = useChartSlot(sel)
  const keys = [
    ...instruments,
    ...ibkrInstruments.map((id) => selKey({ instrument: id, feed: 'ibkr' })),
  ]
  const current = selKey(sel)
  return (
    <Panel
      title={`Gráfico ${index + 1}`}
      className={cn(active && 'ring-1 ring-accent ring-inset')}
      aside={
        slot.source === 'motor'
          ? 'velas del motor'
          : slot.source === 'ejemplo'
            ? 'datos de ejemplo'
            : slot.source === 'ibkr'
              ? ['IBKR', slot.ibkr?.series?.contract, ibkrDelay, 'solo datos']
                  .filter(Boolean)
                  .join(' · ')
              : undefined
      }
    >
      <div className="mb-2 flex flex-wrap gap-2">
        <div className="max-w-full overflow-x-auto">
          <Segmented
            label={`Instrumento del gráfico ${index + 1}`}
            options={keys.includes(current) ? keys : [current, ...keys]}
            format={(k) => {
              const p = parseSelKey(k)
              return p.feed === 'ibkr' ? `${p.instrument} · IBKR` : p.instrument
            }}
            value={current}
            onChange={(k) => {
              const p = parseSelKey(k)
              onChange(withInstrumentSel(sel, p.instrument, p.feed))
            }}
          />
        </div>
        <Segmented
          label={`Timeframe del gráfico ${index + 1}`}
          options={slotTimeframes(sel, timeframes)}
          value={sel.timeframe}
          onChange={(tf) => onChange({ ...sel, timeframe: tf as ChartTimeframe })}
        />
      </div>
      <SlotChart
        slot={slot}
        height={height}
        ruler={tools.ruler}
        onRuler={(ruler) => onTools(ruler ? { ruler, draw: 'none' } : { ruler })}
        draw={tools.draw}
        onDraw={(draw) => onTools({ draw })}
        mode={tools.mode}
        onMode={(mode) => onTools({ mode })}
        ohlc
      />
    </Panel>
  )
}

/** Los eventos del broker y alertas (`ops_event.v1`): solo se piden con la pestaña abierta. */
function Activity() {
  const events = useOpsEvents()
  if (events.status === 'ok')
    return (
      <div className="max-h-[28rem] overflow-y-auto pr-1">
        <OpsEventFeed events={events.data} />
      </div>
    )
  return (
    <p role="status" className="py-4 text-center text-[12.5px] text-muted-foreground">
      {events.status === 'loading'
        ? 'Pidiendo los eventos al motor…'
        : 'Sin conexión con el gateway: no hay eventos para mostrar.'}
    </p>
  )
}

/** Sin ruta de historial en el gateway: se dice y se muestra lo único que hay, del motor. */
function History({ account, live }: { account: PaperAccount; live: boolean }) {
  return (
    <div className="grid gap-2 rounded-xl border border-dashed border-border px-3 py-4 text-[13px]">
      <p>
        <strong className="font-bold">P/L realizado hoy</strong>{' '}
        <span className="font-bold tabular-nums">
          {signed(account.realized_pl_today, 2)} {account.currency}
        </span>{' '}
        <span className="text-muted-foreground">({live ? 'del motor' : 'datos de ejemplo'})</span>
      </p>
      <p className="text-[12px] text-muted-foreground">
        El gateway todavía no tiene una ruta para el historial de operaciones cerradas: cuando la
        tenga, se listan acá. Son resultados de la simulación, no promesas.
      </p>
    </div>
  )
}

const PANES = ['Gráfico', 'Mercados', 'Ticket'] as const
type Pane = (typeof PANES)[number]
const TABS = ['Posiciones', 'Órdenes', 'Historial', 'Actividad', 'Propuestas'] as const
type Tab = (typeof TABS)[number]

export function TerminalView({ member }: { member: Member }) {
  const [scenario, setScenario] = useState<(typeof SCENARIOS)[number]>('Normal')
  const [refresh, setRefresh] = useState(0)
  const remoteAccount = usePaperAccount(refresh)
  const remotePositions = usePaperPositions(refresh)
  // Estado de la operación (#314): compuerta, kill switch y pérdida del día, del motor.
  const ops = useOpsStatus(refresh)
  const opsLive = ops.status === 'ok' ? ops.data : null
  // con gateway, todo sale del broker simulado; sin gateway, los datos de ejemplo
  const live = remoteAccount.status === 'ok'
  const account = live
    ? remoteAccount.data
    : scenario === 'Normal'
      ? PAPER_ACCOUNT
      : PAPER_ACCOUNT_HALTED
  const openPositions = live
    ? remotePositions.status === 'ok'
      ? remotePositions.data
      : []
    : PAPER_POSITIONS
  const locked = account.kill_switch.active
    ? 'Kill switch activo: no se puede operar ni aprobar.'
    : undefined

  const [saved, setSaved] = useState(() => withInstrument(loadLayout(), focusedInstrument()))
  useEffect(() => saveLayout(saved), [saved])
  const [widths, setWidths] = useState(loadWidths)
  useEffect(() => saveWidths(widths), [widths])
  const [active, setActive] = useState(0)
  const [tools, setTools] = useState<ChartTools[]>(() => Array.from({ length: 4 }, () => NO_TOOLS))
  useEffect(() => {
    clearFocusedInstrument()
    const onFocus = (e: Event) => {
      clearFocusedInstrument()
      setSaved((prev) => withInstrument(prev, (e as CustomEvent<string>).detail))
      setActive(0)
    }
    window.addEventListener(FOCUS_EVENT, onFocus)
    return () => window.removeEventListener(FOCUS_EVENT, onFocus)
  }, [])
  // el timeframe que pide la paleta de comandos (#581), para el gráfico activo
  const [wantTf, setWantTf] = useState(focusedTimeframe)
  useEffect(() => {
    clearFocusedTimeframe()
    const onTf = (e: Event) => {
      clearFocusedTimeframe()
      setWantTf((e as CustomEvent<string>).detail)
    }
    window.addEventListener(TIMEFRAME_EVENT, onTf)
    return () => window.removeEventListener(TIMEFRAME_EVENT, onTf)
  }, [])
  const { instruments, timeframes } = useChartCatalog()
  const catalog = useInstruments()
  const names: Record<string, string> =
    catalog.status === 'ok'
      ? Object.fromEntries(catalog.data.instruments.map((i) => [i.id, i.name]))
      : {}
  const summaries = useMarketSummaries(instruments)
  const ibkr = useMarketSummaries([], ibkrSummaries)
  const ibkrInstruments = ibkr.rows.map((r) => r.instrument_id)
  const ibkrDelayOf = (id: string) => delayText(ibkr.rows.find((r) => r.instrument_id === id))

  const count = Number(saved.layout)
  const current = Math.min(active, count - 1)
  const activeSel = saved.slots[current]
  // ajuste durante el render (sin efecto): solo si ese gráfico ofrece el timeframe pedido
  if (wantTf !== null) {
    setWantTf(null)
    if ((slotTimeframes(activeSel, timeframes) as readonly string[]).includes(wantTf))
      setSaved((prev) => ({
        ...prev,
        slots: prev.slots.map((s, k) =>
          k === current ? { ...s, timeframe: wantTf as ChartTimeframe } : s,
        ),
      }))
  }
  const [pane, setPane] = useState<Pane>('Gráfico')
  const [tab, setTab] = useState<Tab>('Posiciones')
  const [sent, setSent] = useState<SentOrder[]>([])

  // Los del ticket (#578): con gateway, solo los que conoce el motor; los que no tienen velas se
  // ven pero no se eligen (el broker simulado no tiene precio para ellos).
  const ticketInstruments = live
    ? catalog.status === 'ok'
      ? catalog.data.instruments.map((i) => i.id)
      : []
    : TERMINAL_INSTRUMENTS
  const noCandles =
    live && summaries.status === 'ok'
      ? summaries.rows.filter((r) => r.at === null).map((r) => r.instrument_id)
      : []
  const tradable = ticketInstruments.filter((i) => !noCandles.includes(i))
  // el instrumento del gráfico activo, si es operable; si no (IBKR, sin velas), el ticket sigue
  // con el que tenía o con el primero operable
  const fromChart =
    activeSel.feed !== 'ibkr' && tradable.includes(activeSel.instrument)
      ? activeSel.instrument
      : null
  const [ticketInstrument, setTicketInstrument] = useState(fromChart ?? tradable[0] ?? '')
  const wanted =
    fromChart ?? (tradable.includes(ticketInstrument) ? ticketInstrument : (tradable[0] ?? ''))
  // ajuste durante el render (sin efecto): el ticket sigue al gráfico activo
  if (wanted !== ticketInstrument) setTicketInstrument(wanted)
  // los de IBKR (#556) son solo datos: abren el gráfico, el ticket queda como estaba
  const pickMarket = (id: string, feed?: 'ibkr') => {
    setSaved((prev) => withInstrument(prev, id, current, feed))
    setPane('Gráfico')
  }
  const patchTools = (i: number, patch: Partial<ChartTools>) =>
    setTools((all) => all.map((t, k) => (k === i ? { ...t, ...patch } : t)))

  const charts = (
    <div className="grid min-w-0 gap-2 p-density lg:grid-cols-[auto_minmax(0,1fr)]">
      <ChartDrawBar
        label={`Herramientas de dibujo del gráfico ${current + 1}`}
        tools={tools[current]}
        onChange={(patch) => patchTools(current, patch)}
        instrument={activeSel.instrument}
      />
      <div className="grid min-w-0 gap-2">
        {count > 1 && (
          <div className="flex items-center gap-2 lg:hidden" data-chart-switcher>
            <span className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
              Gráfico
            </span>
            <Segmented
              label="Gráfico a la vista"
              options={Array.from({ length: count }, (_, i) => String(i + 1))}
              value={String(current + 1)}
              onChange={(v) => setActive(Number(v) - 1)}
            />
          </div>
        )}
        <div
          data-chart-layout={saved.layout}
          className={cn(
            'grid min-w-0 overflow-hidden rounded-section',
            count > 1 && 'gap-px border border-border bg-border lg:grid-cols-2',
          )}
        >
          {saved.slots.slice(0, count).map((sel, i) => (
            <div
              key={i}
              data-chart-slot={i}
              data-active={i === current || undefined}
              onPointerDownCapture={() => setActive(i)}
              onFocusCapture={() => setActive(i)}
              className={cn('min-w-0', i !== current && 'max-lg:hidden')}
            >
              <ChartSlot
                index={i}
                sel={sel}
                instruments={instruments}
                ibkrInstruments={ibkrInstruments}
                ibkrDelay={sel.feed === 'ibkr' ? ibkrDelayOf(sel.instrument) : null}
                timeframes={timeframes}
                height={CHART_HEIGHT[saved.layout]}
                active={count > 1 && i === current}
                tools={tools[i]}
                onTools={(patch) => patchTools(i, patch)}
                onChange={(s) =>
                  setSaved((prev) => ({
                    ...prev,
                    slots: prev.slots.map((old, k) => (k === i ? s : old)),
                  }))
                }
              />
            </div>
          ))}
        </div>
      </div>
    </div>
  )

  const ticket = (
    <Panel title="Ticket" aside={live ? 'simulación · broker simulado' : 'simulación · ejemplo'}>
      {activeSel.feed === 'ibkr' && (
        <p
          role="note"
          data-ibkr-not-tradable
          className="mb-2 rounded-xl border border-[var(--chart-signal)]/50 px-3 py-2 text-[12px] leading-snug"
        >
          <strong className="block font-bold">
            {activeSel.instrument} (IBKR): solo datos, no operable.
          </strong>
          <span className="text-muted-foreground">
            El gráfico muestra las velas de IBKR; el ticket sigue con los instrumentos del broker
            simulado.
          </span>
        </p>
      )}
      {ticketInstrument ? (
        // arranca vacío (#578): ni stop ni TP1 de ejemplo
        <OrderTicket
          key={`${live ? 'live' : 'mock'}|${ticketInstrument}`}
          instruments={ticketInstruments}
          names={names}
          unavailable={noCandles}
          initial={{ instrument_id: ticketInstrument, side: 'long', stop: '', tp1: '' }}
          examples={live ? [] : TICKET_EXAMPLES}
          preview={live ? previewOrder : mockPreview}
          submit={live ? submitOrder : undefined}
          onFilled={() => setRefresh((n) => n + 1)}
          onReport={(order, report) => setSent((all) => [{ order, report }, ...all])}
          locked={locked}
        />
      ) : (
        <p role="status" className="py-4 text-center text-[12.5px] text-muted-foreground">
          {live && catalog.status !== 'error'
            ? 'Pidiendo al motor los instrumentos que se pueden operar…'
            : 'El motor no tiene instrumentos con velas para operar ahora.'}
        </p>
      )}
    </Panel>
  )

  const positions =
    live && remotePositions.status === 'error' ? (
      <p role="alert" className="rounded-xl border border-down/40 p-3 text-[13px]">
        <strong className="block font-bold text-down">No se pudieron leer las posiciones.</strong>
        <span className="text-muted-foreground">{remotePositions.message}</span>
      </p>
    ) : live && remotePositions.status === 'loading' ? (
      <p role="status" className="py-6 text-center text-[13px] text-muted-foreground">
        Pidiendo las posiciones al broker simulado…
      </p>
    ) : (
      <PositionsTable positions={openPositions} />
    )

  const tabLabel: Record<Tab, string> = {
    Posiciones: `Posiciones (${openPositions.length})`,
    Órdenes: `Órdenes (${sent.length})`,
    Historial: 'Historial',
    Actividad: 'Actividad',
    Propuestas: 'Propuestas',
  }
  const tabNote: Record<Tab, string> = {
    Posiciones: live ? 'P/L del motor, en simulación' : 'datos de ejemplo',
    Órdenes: 'las enviadas desde esta pestaña',
    Historial: 'sin ruta en el gateway',
    Actividad: 'eventos del broker y alertas',
    Propuestas: 'de ejemplo: el motor todavía no las publica acá',
  }

  return (
    <div
      className="grid min-w-0 gap-2 rounded-section bg-section p-2 text-foreground sm:p-3"
      data-terminal
      data-section="terminal"
    >
      {/*
       * Lo único fijo arriba (#578): una franja angosta y opaca con "simulación". Ocupa su propio
       * lugar (no flota encima de la cuenta ni del gráfico) y, al bajar, el contenido pasa por
       * debajo sin transparencias. La cuenta ya no queda fija: tapaba el gráfico.
       */}
      <div
        aria-hidden="true"
        data-sim-pill
        className="pointer-events-none sticky top-2 z-20 flex justify-center"
      >
        <span className="rounded-full border border-[var(--chart-signal)]/50 bg-background px-3 py-1 text-[10.5px] font-extrabold tracking-[0.1em] text-[var(--chart-signal)] shadow-sm">
          SIMULACIÓN{' '}
          <span className="font-semibold tracking-normal text-muted-foreground">
            · paper: no se envía ninguna orden real
          </span>
        </span>
      </div>
      <div className="grid gap-2">
        <AccountStrip
          account={account}
          aside={
            !live && (
              <Segmented
                label="Escenario de ejemplo"
                options={SCENARIOS}
                value={scenario}
                onChange={(v) => setScenario(v as typeof scenario)}
              />
            )
          }
        />
      </div>
      {opsLive && <OpsKillSwitchBanner status={opsLive} />}
      {!opsLive && <KillSwitchBanner account={account} />}
      <OpsStatusBar status={opsLive} />

      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="max-w-2xl text-[12px] text-muted-foreground">
          {live ? (
            <>
              <strong className="text-foreground">Simulación (paper)</strong> con el broker simulado
              del motor: no se envía ninguna orden real.
            </>
          ) : (
            <>
              <strong className="text-foreground">Datos de ejemplo</strong>: el gateway no respondió
              y no se envía nada.
            </>
          )}{' '}
          El motor valida con su riesgo; la persona decide. Precios y frecuencias del pasado, no
          promesas.
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <div className="lg:hidden">
            <Segmented
              label="Panel a la vista"
              options={PANES}
              value={pane}
              onChange={(v) => setPane(v as Pane)}
            />
          </div>
          {/* 1, 2 o 4 gráficos: en el celular, solo en la pantalla del gráfico (#578) */}
          <div className={cn(pane !== 'Gráfico' && 'max-lg:hidden')} data-layout-picker>
            <Segmented
              label="Gráficos en pantalla"
              options={LAYOUTS}
              value={saved.layout}
              onChange={(v) => setSaved((prev) => ({ ...prev, layout: v as Layout }))}
            />
          </div>
        </div>
      </div>

      {/*
       * Mercados | gráficos | ticket, separados por líneas de 1 px. Los divisores viven en la
       * columna del gráfico (la más alta), sobre esas líneas; el orden de los hijos no cambia.
       */}
      <div
        data-terminal-desk
        style={
          {
            '--w-left': `${widths.left}px`,
            '--w-right': `${widths.right}px`,
          } as CSSProperties
        }
        className="grid min-w-0 rounded-section border border-border lg:grid-cols-[var(--w-left)_minmax(0,1fr)] xl:grid-cols-[var(--w-left)_minmax(0,1fr)_var(--w-right)] [&_.glass]:border-transparent [&_.glass]:bg-transparent"
      >
        <div
          id="terminal-markets"
          className={cn(
            'min-w-0 border-border lg:border-r',
            pane !== 'Mercados' && 'max-lg:hidden',
          )}
        >
          <div className="lg:sticky lg:top-12">
            <Watchlist
              groups={[
                {
                  id: 'broker',
                  title: 'Broker simulado',
                  sourceLabel: summaries.label,
                  rows: summaries.rows,
                  loading: summaries.status === 'loading',
                },
                {
                  id: 'ibkr',
                  title: 'IBKR',
                  sourceLabel: 'solo datos, no operable',
                  rows: ibkr.rows,
                  feed: 'ibkr',
                  badge: 'IBKR',
                  loading: ibkr.status === 'loading',
                  note:
                    ibkr.status === 'error'
                      ? 'Sin conexión con el gateway: no hay datos de IBKR.'
                      : ibkr.status === 'loading'
                        ? 'Pidiendo los datos de IBKR…'
                        : ibkr.rows.length === 0
                          ? 'El gateway no tiene instrumentos de IBKR configurados.'
                          : 'Datos con atraso, tal como los manda el gateway. Precios del pasado, no promesas.',
                },
              ]}
              names={names}
              selected={selKey(activeSel)}
              onSelect={pickMarket}
            />
          </div>
        </div>
        <div className={cn('relative min-w-0', pane !== 'Gráfico' && 'max-lg:hidden')}>
          <PaneResizer
            label="Ancho de la lista de mercados"
            controls="terminal-markets"
            side="left"
            value={widths.left}
            min={WIDTH.left.min}
            max={WIDTH.left.max}
            onChange={(left) => setWidths((w) => ({ ...w, left }))}
            className="-left-[5px] hidden lg:block"
          />
          {charts}
          <PaneResizer
            label="Ancho del ticket"
            controls="terminal-ticket"
            side="right"
            value={widths.right}
            min={WIDTH.right.min}
            max={WIDTH.right.max}
            onChange={(right) => setWidths((w) => ({ ...w, right }))}
            className="-right-[4px] hidden xl:block"
          />
        </div>
        <div
          id="terminal-ticket"
          className={cn(
            'min-w-0 border-border lg:col-span-2 lg:border-t xl:col-span-1 xl:border-t-0 xl:border-l',
            pane !== 'Ticket' && 'max-lg:hidden',
          )}
        >
          <div className="xl:sticky xl:top-12">{ticket}</div>
        </div>
      </div>

      <section
        aria-label="Actividad de la cuenta"
        className="min-w-0 rounded-section border border-border bg-section p-density"
      >
        <div className="mb-2 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-2">
          <div
            role="tablist"
            aria-label="Paneles de la cuenta"
            className="flex max-w-full gap-1 overflow-x-auto border-b border-border"
          >
            {TABS.map((t) => (
              <button
                key={t}
                type="button"
                role="tab"
                id={`terminal-tab-${t}`}
                aria-selected={t === tab}
                aria-controls="terminal-tabpanel"
                onClick={() => setTab(t)}
                className={cn(
                  '-mb-px min-h-8 shrink-0 border-b-2 px-2.5 text-[11px] font-bold tracking-[0.06em] whitespace-nowrap uppercase transition-colors focus-visible:outline-2 focus-visible:outline-ring',
                  t === tab
                    ? 'border-accent text-foreground'
                    : 'border-transparent text-muted-foreground hover:text-foreground',
                )}
              >
                {tabLabel[t]}
              </button>
            ))}
          </div>
          <span className="text-[11px] text-muted-foreground">{tabNote[tab]}</span>
        </div>
        <div role="tabpanel" id="terminal-tabpanel" aria-labelledby={`terminal-tab-${tab}`}>
          {tab === 'Posiciones' && positions}
          {tab === 'Órdenes' && <SessionOrders orders={sent} />}
          {tab === 'Historial' && <History account={account} live={live} />}
          {tab === 'Actividad' && <Activity />}
          {tab === 'Propuestas' && (
            <section aria-label="Propuestas" className="grid gap-3 xl:grid-cols-2">
              {PROPOSALS.map((p) => (
                <ProposalCard
                  key={p.proposal.id}
                  item={p}
                  canApprove={member.id === RESPONSIBLE_ID}
                  locked={locked}
                  example
                />
              ))}
            </section>
          )}
        </div>
      </section>
    </div>
  )
}
