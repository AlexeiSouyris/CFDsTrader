/**
 * Terminal de trading (fase 10, prototipo, #275). **Solo simulación (paper)** y datos de ejemplo:
 * no hay broker ni se envía ninguna orden. Solo ubica componentes y les pasa lo que mandaría el
 * motor; no calcula riesgo, P/L, margen ni probabilidades (CLAUDE.md §1).
 *
 * TODO(#275): conectar al motor (gateway) cuando existan los contratos de cuenta, posición y
 * validación de orden. Hoy: `mocks/terminal.ts` y `PROPOSALS` (`proposal.v1`).
 *
 * Gráficos (#286): una grilla de 1, 2 o 4 (2×2) `LiquidityChart`, cada uno con su instrumento y
 * timeframe. Las velas llegan del gateway (`liquidity_view.v1`, como la vista de liquidez); sin
 * gateway, el de US100 en 15 m usa los datos de ejemplo. La disposición queda en `localStorage`.
 */
import { useEffect, useState, type ReactNode } from 'react'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { ProposalCard } from '@/components/ui/proposal-card'
import { Segmented } from '@/components/ui/segmented'
import {
  AccountStrip,
  KillSwitchBanner,
  OrderTicket,
  PositionsTable,
} from '@/components/ui/trading-terminal'
import { useInstruments, useLiquidityView } from '@/lib/liquidity-api'
import { chartBars } from '@/lib/liquidity-bars'
import type { LiquidityViewData, ViewTimeframe } from '@/lib/liquidity-contracts'
import { cn } from '@/lib/utils'
import {
  CHART_SIGNALS,
  LIQUIDITY_US100,
  mockEngineVerdict,
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
}: {
  title: ReactNode
  aside?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="glass relative min-w-0 rounded-[20px] p-5 has-[[role=dialog]]:z-30">
      <header className="mb-4 flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-[15px] font-extrabold tracking-tight">{title}</h2>
        {aside && <span className="text-[11.5px] text-muted-foreground">{aside}</span>}
      </header>
      {children}
    </section>
  )
}

const SCENARIOS = ['Normal', 'Kill switch activo'] as const

const LAYOUTS = ['1', '2', '4'] as const
type Layout = (typeof LAYOUTS)[number]
type ChartSel = { instrument: string; timeframe: ViewTimeframe }
const TIMEFRAMES: ViewTimeframe[] = ['15m', '1h', '1d']
/** Lo que abre cada lugar de la grilla la primera vez. */
const DEFAULT_SLOTS: ChartSel[] = [
  { instrument: 'US100', timeframe: '15m' },
  { instrument: 'DE40', timeframe: '15m' },
  { instrument: 'US500', timeframe: '1h' },
  { instrument: 'US30', timeframe: '1h' },
]
const STORAGE_KEY = 'cfdstrader.terminal.charts'
const CHART_HEIGHT: Record<Layout, number> = { '1': 400, '2': 360, '4': 300 }

type Saved = { layout: Layout; slots: ChartSel[] }

/** Lee la disposición guardada; si no hay, está rota o el navegador no deja leer, la de siempre. */
function loadLayout(): Saved {
  const fallback: Saved = { layout: '1', slots: DEFAULT_SLOTS }
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<Saved> | null
    if (!raw || !LAYOUTS.includes(raw.layout as Layout)) return fallback
    const slots = DEFAULT_SLOTS.map((d, i) => {
      const s = raw.slots?.[i]
      return s && typeof s.instrument === 'string' && TIMEFRAMES.includes(s.timeframe) ? s : d
    })
    return { layout: raw.layout as Layout, slots }
  } catch {
    return fallback
  }
}

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
  timeframes,
  height,
}: {
  index: number
  sel: ChartSel
  onChange: (patch: Partial<ChartSel>) => void
  instruments: string[]
  timeframes: ViewTimeframe[]
  height: number
}) {
  const remote = useLiquidityView(sel)
  const example =
    sel.instrument === LIQUIDITY_US100.instrument_id && sel.timeframe === '15m'
      ? LIQUIDITY_US100
      : null
  const live: LiquidityViewData | null =
    remote.status === 'ok' && remote.data.candles.length > 0 ? remote.data : null
  const data = live ?? example
  return (
    <Panel
      title={`Gráfico ${index + 1}`}
      aside={live ? 'velas del motor' : example ? 'datos de ejemplo' : undefined}
    >
      <div className="mb-3 flex flex-wrap gap-2">
        <Segmented
          label={`Instrumento del gráfico ${index + 1}`}
          options={instruments}
          value={sel.instrument}
          onChange={(instrument) => onChange({ instrument })}
        />
        <Segmented
          label={`Timeframe del gráfico ${index + 1}`}
          options={timeframes}
          value={sel.timeframe}
          onChange={(tf) => onChange({ timeframe: tf as ViewTimeframe })}
        />
      </div>
      {data ? (
        <LiquidityChart
          key={`${sel.instrument}|${sel.timeframe}`}
          instrument={data.instrument_id}
          timeframe={data.timeframe ?? sel.timeframe}
          bars={chartBars(data)}
          levels={data.levels}
          fvgs={data.fvgs}
          imbalance={data.imbalance}
          signals={data === example ? CHART_SIGNALS : []}
          height={height}
        />
      ) : (
        <p
          role="status"
          className="grid place-items-center rounded-xl border border-dashed border-border px-4 text-center text-[12.5px] text-muted-foreground"
          style={{ minHeight: height }}
        >
          {remote.status === 'loading'
            ? `Pidiendo ${sel.instrument} (${sel.timeframe}) al motor…`
            : `Sin velas de ${sel.instrument} en ${sel.timeframe}: el gateway no respondió o todavía no las tiene.`}
        </p>
      )}
    </Panel>
  )
}

export function TerminalView({ member }: { member: Member }) {
  const [scenario, setScenario] = useState<(typeof SCENARIOS)[number]>('Normal')
  const account = scenario === 'Normal' ? PAPER_ACCOUNT : PAPER_ACCOUNT_HALTED
  const locked = account.kill_switch.active
    ? 'Kill switch activo: no se puede operar ni aprobar.'
    : undefined
  const [saved, setSaved] = useState(loadLayout)
  useEffect(() => saveLayout(saved), [saved])
  const catalog = useInstruments()
  const instruments =
    catalog.status === 'ok' ? catalog.data.instruments.map((i) => i.id) : TERMINAL_INSTRUMENTS
  const timeframes = catalog.status === 'ok' ? catalog.data.timeframes : TIMEFRAMES
  const count = Number(saved.layout)
  const charts = (
    <div
      data-chart-layout={saved.layout}
      className={cn('grid min-w-0 gap-4', count > 1 && 'lg:grid-cols-2')}
    >
      {saved.slots.slice(0, count).map((sel, i) => (
        <ChartSlot
          key={i}
          index={i}
          sel={sel}
          instruments={instruments}
          timeframes={timeframes}
          height={CHART_HEIGHT[saved.layout]}
          onChange={(s) =>
            setSaved((prev) => ({
              ...prev,
              slots: prev.slots.map((old, k) => (k === i ? { ...old, ...s } : old)),
            }))
          }
        />
      ))}
    </div>
  )
  const ticket = (
    <Panel title="Ticket" aside="simulación">
      <OrderTicket
        instruments={TERMINAL_INSTRUMENTS}
        initial={TICKET_EXAMPLES[0].draft}
        examples={TICKET_EXAMPLES}
        verdictFor={mockEngineVerdict}
        locked={locked}
      />
    </Panel>
  )
  const positions = (
    <Panel title={`Posiciones abiertas (${PAPER_POSITIONS.length})`} aside="P/L del motor">
      <PositionsTable positions={PAPER_POSITIONS} />
    </Panel>
  )
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="max-w-2xl text-[13px] text-muted-foreground">
          Prototipo con datos de ejemplo. El motor propone y valida; la persona decide. Son
          frecuencias del pasado y reglas de riesgo, no promesas.
        </p>
        <div className="flex flex-wrap gap-2">
          <Segmented
            label="Gráficos en pantalla"
            options={LAYOUTS}
            value={saved.layout}
            onChange={(v) => setSaved((prev) => ({ ...prev, layout: v as Layout }))}
          />
          <Segmented
            label="Escenario de ejemplo"
            options={SCENARIOS}
            value={scenario}
            onChange={(v) => setScenario(v as typeof scenario)}
          />
        </div>
      </div>

      <AccountStrip account={account} />
      <KillSwitchBanner account={account} />

      {count === 1 ? (
        <>
          <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
            {charts}
            {ticket}
          </div>
          {positions}
        </>
      ) : (
        <>
          {charts}
          <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
            {positions}
            {ticket}
          </div>
        </>
      )}

      <section aria-label="Propuestas" className="grid gap-3">
        <h2 className="px-1 text-[15px] font-extrabold tracking-tight">
          Propuestas del motor{' '}
          <span className="text-[12px] font-semibold text-muted-foreground">
            · vencen a los 10 minutos
          </span>
        </h2>
        <div className="grid gap-3 2xl:grid-cols-2">
          {PROPOSALS.map((p) => (
            <ProposalCard
              key={p.proposal.id}
              item={p}
              canApprove={member.id === RESPONSIBLE_ID}
              locked={locked}
            />
          ))}
        </div>
      </section>
    </div>
  )
}
