/**
 * Terminal de trading (fase 10, prototipo, #275). **Solo simulación (paper)** y datos de ejemplo:
 * no hay broker ni se envía ninguna orden. Solo ubica componentes y les pasa lo que mandaría el
 * motor; no calcula riesgo, P/L, margen ni probabilidades (CLAUDE.md §1).
 *
 * TODO(#275): conectar al motor (gateway) cuando existan los contratos de cuenta, posición y
 * validación de orden. Hoy: `mocks/terminal.ts` y `PROPOSALS` (`proposal.v1`).
 */
import { useState, type ReactNode } from 'react'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { ProposalCard } from '@/components/ui/proposal-card'
import { Segmented } from '@/components/ui/segmented'
import {
  AccountStrip,
  KillSwitchBanner,
  OrderTicket,
  PositionsTable,
} from '@/components/ui/trading-terminal'
import { chartBars } from '@/lib/liquidity-bars'
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

export function TerminalView({ member }: { member: Member }) {
  const [scenario, setScenario] = useState<(typeof SCENARIOS)[number]>('Normal')
  const account = scenario === 'Normal' ? PAPER_ACCOUNT : PAPER_ACCOUNT_HALTED
  const locked = account.kill_switch.active
    ? 'Kill switch activo: no se puede operar ni aprobar.'
    : undefined
  const data = LIQUIDITY_US100
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="max-w-2xl text-[13px] text-muted-foreground">
          Prototipo con datos de ejemplo. El motor propone y valida; la persona decide. Son
          frecuencias del pasado y reglas de riesgo, no promesas.
        </p>
        <Segmented
          label="Escenario de ejemplo"
          options={SCENARIOS}
          value={scenario}
          onChange={(v) => setScenario(v as typeof scenario)}
        />
      </div>

      <AccountStrip account={account} />
      <KillSwitchBanner account={account} />

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
        <Panel title="Gráfico" aside="velas de 15 m, niveles, FVG y señales del motor">
          <LiquidityChart
            instrument={data.instrument_id}
            timeframe="15m"
            bars={chartBars(data)}
            levels={data.levels}
            fvgs={data.fvgs}
            imbalance={data.imbalance}
            signals={CHART_SIGNALS}
            height={400}
          />
        </Panel>
        <Panel title="Ticket" aside="simulación">
          <OrderTicket
            instruments={TERMINAL_INSTRUMENTS}
            initial={TICKET_EXAMPLES[0].draft}
            examples={TICKET_EXAMPLES}
            verdictFor={mockEngineVerdict}
            locked={locked}
          />
        </Panel>
      </div>

      <Panel title={`Posiciones abiertas (${PAPER_POSITIONS.length})`} aside="P/L del motor">
        <PositionsTable positions={PAPER_POSITIONS} />
      </Panel>

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
