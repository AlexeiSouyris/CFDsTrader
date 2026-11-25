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
 * Si el gateway no responde, se ven los datos de ejemplo de `mocks/terminal.ts` y no se envía
 * nada. La disposición de los gráficos queda en `localStorage`.
 */
import { useEffect, useState, type ReactNode } from 'react'

import { ChartDrawBar } from '@/components/ui/chart-draw-bar'
import { NO_TOOLS, type ChartTools } from '@/lib/chart-drawings'
import { SlotChart } from '@/components/ui/chart-slot'
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
import { clearFocusedInstrument, FOCUS_EVENT, focusedInstrument } from '@/lib/terminal-focus'
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
  type TicketDraft,
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
    <section className={cn('glass relative min-w-0 rounded-[20px] p-3 sm:p-4', className)}>
      <header className="mb-3 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <h2 className="text-[14px] font-extrabold tracking-tight">{title}</h2>
        {aside && <span className="text-[11px] text-muted-foreground">{aside}</span>}
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
      className={cn('ring-offset-0', active ? 'ring-2 ring-primary/70' : 'ring-1 ring-transparent')}
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
      <div className="mb-3 flex flex-wrap gap-2">
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
    <div className="grid gap-2 rounded-xl border border-dashed border-border px-4 py-5 text-[13px]">
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
  const [pane, setPane] = useState<Pane>('Gráfico')
  const [tab, setTab] = useState<Tab>('Posiciones')
  const [sent, setSent] = useState<SentOrder[]>([])

  // el ticket arranca con el ejemplo; elegir un mercado lo abre en ese instrumento, sin stop
  const [ticketDraft, setTicketDraft] = useState<TicketDraft>(TICKET_EXAMPLES[0].draft)
  // los de IBKR (#556) son solo datos: abren el gráfico, el ticket queda como estaba
  const pickMarket = (id: string, feed?: 'ibkr') => {
    setSaved((prev) => withInstrument(prev, id, current, feed))
    if (!feed && id !== ticketDraft.instrument_id)
      setTicketDraft({ instrument_id: id, side: 'long', stop: '', tp1: '' })
    setPane('Gráfico')
  }
  const patchTools = (i: number, patch: Partial<ChartTools>) =>
    setTools((all) => all.map((t, k) => (k === i ? { ...t, ...patch } : t)))

  const charts = (
    <div className="grid min-w-0 gap-3 lg:grid-cols-[auto_minmax(0,1fr)]">
      <ChartDrawBar
        label={`Herramientas de dibujo del gráfico ${current + 1}`}
        tools={tools[current]}
        onChange={(patch) => patchTools(current, patch)}
        instrument={activeSel.instrument}
      />
      <div className="grid min-w-0 gap-3">
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
          className={cn('grid min-w-0 gap-3', count > 1 && 'lg:grid-cols-2')}
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

  const ticketInstruments = live ? instruments : TERMINAL_INSTRUMENTS
  const ticket = (
    <Panel title="Ticket" aside={live ? 'simulación · broker simulado' : 'simulación · ejemplo'}>
      {activeSel.feed === 'ibkr' && (
        <p
          role="note"
          data-ibkr-not-tradable
          className="mb-3 rounded-xl border border-[var(--chart-signal)]/50 px-3 py-2 text-[12px] leading-snug"
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
      <OrderTicket
        key={`${live ? 'live' : 'mock'}|${ticketDraft.instrument_id}`}
        instruments={
          ticketInstruments.includes(ticketDraft.instrument_id)
            ? ticketInstruments
            : [ticketDraft.instrument_id, ...ticketInstruments]
        }
        initial={ticketDraft}
        examples={live ? [] : TICKET_EXAMPLES}
        preview={live ? previewOrder : mockPreview}
        submit={live ? submitOrder : undefined}
        onFilled={() => setRefresh((n) => n + 1)}
        onReport={(order, report) => setSent((all) => [{ order, report }, ...all])}
        locked={locked}
      />
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
    Propuestas: 'datos de ejemplo · vencen a los 10 minutos',
  }

  return (
    <div className="grid min-w-0 gap-3" data-terminal>
      {/* en el celular la cuenta no queda fija (ocupa media pantalla): queda fijo este aviso */}
      <div
        aria-hidden="true"
        data-sim-pill
        className="pointer-events-none sticky top-2 z-20 -mb-3 flex h-0 justify-end lg:hidden"
      >
        <span className="mt-1 mr-1 h-fit rounded-full border border-[var(--chart-signal)]/50 bg-background/90 px-2.5 py-1 text-[10px] font-extrabold tracking-[0.1em] text-[var(--chart-signal)] shadow-sm backdrop-blur">
          SIMULACIÓN
        </span>
      </div>
      <div className="grid gap-2 lg:sticky lg:top-2 lg:z-20">
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
          <Segmented
            label="Gráficos en pantalla"
            options={LAYOUTS}
            value={saved.layout}
            onChange={(v) => setSaved((prev) => ({ ...prev, layout: v as Layout }))}
          />
        </div>
      </div>

      <div
        data-terminal-desk
        className="grid min-w-0 items-start gap-3 lg:grid-cols-[13rem_minmax(0,1fr)] xl:grid-cols-[12.5rem_minmax(0,1fr)_20rem]"
      >
        <div className={cn('min-w-0 lg:sticky lg:top-28', pane !== 'Mercados' && 'max-lg:hidden')}>
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
        <div className={cn('min-w-0', pane !== 'Gráfico' && 'max-lg:hidden')}>{charts}</div>
        <div
          className={cn(
            'min-w-0 lg:col-span-2 xl:sticky xl:top-28 xl:col-span-1',
            pane !== 'Ticket' && 'max-lg:hidden',
          )}
        >
          {ticket}
        </div>
      </div>

      <section
        aria-label="Actividad de la cuenta"
        className="glass min-w-0 rounded-[20px] p-3 sm:p-4"
      >
        <div className="mb-3 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-2">
          <div
            role="tablist"
            aria-label="Paneles de la cuenta"
            className="flex max-w-full gap-1 overflow-x-auto"
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
                  'min-h-9 shrink-0 rounded-full px-3 text-[12px] font-bold whitespace-nowrap transition-colors focus-visible:outline-2 focus-visible:outline-ring',
                  t === tab
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground hover:text-foreground',
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
                />
              ))}
            </section>
          )}
        </div>
      </section>
    </div>
  )
}
