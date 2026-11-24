/**
 * Pestaña Performance (#286): resumen general de la simulación. Las corridas de backtest con la
 * forma de `backtest_run.v1`, el fondo y las estadísticas de la última corrida.
 *
 * **No calcula nada** (CLAUDE.md §1): las métricas llegan del motor y se muestran tal cual; las
 * fracciones se pasan a % corriendo la coma en el texto (`percent`).
 *
 * Las corridas llegan del gateway (`GET /api/v1/backtest_runs`, `backtest_run.v1[]`, las más
 * nuevas primero, #292). Si no responde, se ven las corridas de ejemplo de `mocks/`, marcadas así.
 * El fondo llega del motor (`GET /api/v1/fund`, `fund_summary.v1`, #312); sin gateway, el de
 * ejemplo, marcado así. No se inventan contratos: los tipos son los de `contracts/`.
 */
import { useState, type ReactNode } from 'react'

import { AnimatedNumber } from '@/components/ui/animated-number'
import { RunDetailView } from '@/components/ui/run-detail'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal } from '@/lib/format'
import { currencySign, useFund } from '@/lib/fund-api'
import { dayYear, numOrDash, pctOrDash, signedPct, tone } from '@/lib/run-format'
import { useBacktestRuns, useRunDetail } from '@/lib/paper-api'
import { LearningSection, PaperVsBacktestSection } from '@/views/phase8'
import { cn } from '@/lib/utils'
import { BACKTEST_RUN_DETAIL, BACKTEST_RUNS, type BacktestRun } from '@/mocks'

const members = (n: number) => `${n} ${n === 1 ? 'miembro' : 'miembros'}`

function Card({
  title,
  aside,
  children,
  className,
}: {
  title?: ReactNode
  aside?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <section
      className={cn(
        'glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30',
        className,
      )}
    >
      {title && (
        <header className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
          <h2 className="text-[14px] font-bold">{title}</h2>
          {aside && <span className="text-[11px] text-muted-foreground">{aside}</span>}
        </header>
      )}
      {children}
    </section>
  )
}

function Kpi({ label, value, detail }: { label: ReactNode; value: ReactNode; detail: string }) {
  return (
    <Card className="flex min-h-28 flex-col justify-between gap-2">
      <span className="text-[11px] leading-tight font-bold text-muted-foreground">{label}</span>
      <strong className="text-[clamp(22px,2.4vw,30px)] leading-none font-extrabold tracking-[-0.04em] whitespace-nowrap tabular-nums">
        {value}
      </strong>
      <span className="text-[10.5px] leading-snug text-muted-foreground">{detail}</span>
    </Card>
  )
}

/** Celda que, más angosto que `lg`, muestra el título de su columna arriba (la fila es una tarjeta). */
const cell =
  'py-2.5 max-lg:p-0 max-lg:before:block max-lg:before:text-[10px] max-lg:before:font-bold max-lg:before:tracking-[0.1em] max-lg:before:text-muted-foreground max-lg:before:uppercase max-lg:before:content-[attr(data-label)]'

function RunsTable({
  runs,
  open,
  onOpen,
}: {
  runs: BacktestRun[]
  open: string | null
  onOpen: (runId: string) => void
}) {
  const th = 'py-2 pr-3 font-bold whitespace-nowrap last:pr-0'
  return (
    <div className="overflow-x-auto max-lg:overflow-visible">
      <table
        aria-label="Corridas de backtest"
        className="w-full text-[12.5px] lg:min-w-[760px] max-lg:block"
      >
        <thead className="max-lg:sr-only">
          <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
            <th className={th}>Corrida</th>
            <th className={th}>Instrumentos</th>
            <th className={th}>TF</th>
            <th className={th}>Período</th>
            <th className={cn(th, 'text-right')}>Operaciones</th>
            <th className={cn(th, 'text-right')}>Retorno</th>
            <th className={cn(th, 'text-right')}>DD máx.</th>
            <th className={cn(th, 'text-right')}>Sharpe</th>
            <th className={cn(th, 'text-right')}>Acierto</th>
            <th className={cn(th, 'text-right')}>Kill switch</th>
          </tr>
        </thead>
        <tbody className="max-lg:grid max-lg:gap-2 sm:max-lg:grid-cols-2">
          {runs.map((r) => {
            const isOpen = open === r.run_id
            return (
              <tr
                key={r.run_id}
                data-open={isOpen || undefined}
                className={cn(
                  'border-b border-border transition-colors last:border-b-0 hover:bg-foreground/[0.03] data-open:bg-foreground/5',
                  'max-lg:grid max-lg:grid-cols-3 max-lg:gap-x-3 max-lg:gap-y-2.5 max-lg:rounded-xl max-lg:border max-lg:p-3 max-lg:last:border-b',
                )}
              >
                <td className="py-2.5 pr-3 max-lg:col-span-3 max-lg:p-0">
                  <button
                    type="button"
                    aria-expanded={isOpen}
                    aria-label={`Ver el detalle de ${r.run_id}`}
                    onClick={() => onOpen(r.run_id)}
                    className="block rounded-sm text-left font-semibold [overflow-wrap:anywhere] underline-offset-4 hover:underline lg:whitespace-nowrap"
                  >
                    {r.run_id}
                  </button>
                  <span className="text-[10.5px] text-muted-foreground">
                    {dateTimeUtc(r.finished_at)} ·{' '}
                    {r.costs.spread_source === 'config' ? 'spread de config' : 'spread medido'}
                  </span>
                </td>
                <td data-label="Instrumentos" className={cn(cell, 'pr-3 max-lg:col-span-2')}>
                  {r.instruments.join(', ')}
                </td>
                <td data-label="TF" className={cn(cell, 'pr-3')}>
                  {r.timeframe}
                </td>
                <td
                  data-label="Período"
                  className={cn(cell, 'pr-3 tabular-nums max-lg:col-span-3')}
                >
                  <span className="whitespace-nowrap">{dayYear(r.from)} –</span>{' '}
                  <span className="whitespace-nowrap">{dayYear(r.to)}</span>
                </td>
                <td
                  data-label="Operaciones"
                  className={cn(
                    cell,
                    'pr-3 text-right whitespace-nowrap tabular-nums max-lg:text-left',
                  )}
                >
                  {r.metrics.trades}
                </td>
                <td
                  data-label="Retorno"
                  className={cn(
                    cell,
                    'pr-3 text-right font-semibold whitespace-nowrap tabular-nums max-lg:text-left',
                    tone(r.metrics.total_return),
                  )}
                >
                  {signedPct(r.metrics.total_return)}
                </td>
                <td
                  data-label="DD máx."
                  className={cn(
                    cell,
                    'pr-3 text-right whitespace-nowrap tabular-nums max-lg:text-left',
                  )}
                >
                  {pctOrDash(r.metrics.max_drawdown, 2)}
                </td>
                <td
                  data-label="Sharpe"
                  className={cn(
                    cell,
                    'pr-3 text-right whitespace-nowrap tabular-nums max-lg:text-left',
                  )}
                >
                  {numOrDash(r.metrics.sharpe)}
                </td>
                <td
                  data-label="Acierto"
                  className={cn(
                    cell,
                    'pr-3 text-right whitespace-nowrap tabular-nums max-lg:text-left',
                  )}
                >
                  {pctOrDash(r.metrics.win_rate)}
                </td>
                <td
                  data-label="Kill switch"
                  className={cn(cell, 'text-right whitespace-nowrap tabular-nums max-lg:text-left')}
                >
                  {r.metrics.kill_switch_at ? dayYear(r.metrics.kill_switch_at) : 'no'}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

export function PerformanceView({ onOpen }: { onOpen: (view: string) => void }) {
  const remote = useBacktestRuns()
  const live = remote.status === 'ok'
  const { summary: fund, live: fundLive } = useFund()
  const runs = live ? remote.data : BACKTEST_RUNS
  const last = runs[0]
  const m = last?.metrics
  const [open, setOpen] = useState<string | null>(null)
  // con gateway, el detalle lo pide al motor; sin gateway, solo hay uno de ejemplo
  const remoteDetail = useRunDetail(live ? open : null)
  return (
    <div className="grid gap-3">
      <p className="max-w-3xl text-[13px] text-muted-foreground">
        Cómo le fue a la <strong className="text-foreground">simulación (paper)</strong>, con costos
        incluidos (spread, slippage y financiación). Son resultados del pasado
        {live ? (
          ' que guardó el motor'
        ) : (
          <>
            {' '}
            con <strong className="text-foreground">datos de ejemplo</strong> (el gateway no
            respondió)
          </>
        )}
        : no anticipan lo que va a pasar.
      </p>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Kpi
          label={<Term id="nav">Valor de la unidad</Term>}
          value={<AnimatedNumber value={fund.nav} format={(v) => decimal(v, 4)} />}
          detail={
            fund.valued_at
              ? `liquidación del ${dateTimeUtc(fund.valued_at)}${fundLive ? '' : ' (ejemplo)'}`
              : 'sin liquidaciones todavía'
          }
        />
        <Kpi
          label="Valor del fondo"
          value={
            <>
              <AnimatedNumber value={fund.total_value} format={decimal} />{' '}
              {currencySign(fund.currency)}
            </>
          }
          detail={`${members(fund.members.length)}, simulación${fundLive ? '' : ' (ejemplo)'}`}
        />
        {m && (
          <>
            <Kpi
              label="Retorno de la última corrida"
              value={<span className={tone(m.total_return)}>{signedPct(m.total_return)}</span>}
              detail={`${m.trades} operaciones, con costos`}
            />
            <Kpi
              label={<Term id="drawdown">Drawdown máximo</Term>}
              value={pctOrDash(m.max_drawdown, 2)}
              detail="de la última corrida"
            />
          </>
        )}
      </div>

      <Card
        title="Corridas de backtest"
        aside={`${runs.length} ${runs.length === 1 ? 'corrida' : 'corridas'} ${live ? 'del motor' : '(ejemplo)'}`}
      >
        {runs.length ? (
          <RunsTable
            runs={runs}
            open={open}
            onOpen={(id) => setOpen((cur) => (cur === id ? null : id))}
          />
        ) : (
          <p className="text-[12.5px]">Todavía no hay corridas guardadas.</p>
        )}
      </Card>

      {open && (
        <RunDetailSlot
          key={open}
          runId={open}
          live={live}
          remote={remoteDetail}
          onClose={() => setOpen(null)}
        />
      )}

      <div className="grid gap-3 lg:grid-cols-[1.4fr_1fr]">
        {m && (
          <Card title="Estadísticas de la última corrida" aside={last.run_id}>
            <dl className="grid grid-cols-2 gap-x-6 gap-y-4 sm:grid-cols-3">
              {(
                [
                  ['Operaciones', String(m.trades)],
                  ['Acierto', pctOrDash(m.win_rate)],
                  ['Profit factor', numOrDash(m.profit_factor)],
                  ['Sharpe', numOrDash(m.sharpe)],
                  ['Sortino', numOrDash(m.sortino)],
                  ['Exposición', pctOrDash(m.exposure)],
                ] as const
              ).map(([label, value]) => (
                <div key={label}>
                  <dt className="text-[10.5px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
                    {label}
                  </dt>
                  <dd className="mt-1 text-[18px] font-extrabold tracking-[-0.02em] tabular-nums">
                    {value}
                  </dd>
                </div>
              ))}
            </dl>
          </Card>
        )}
        <Card title="Fondo" aside={fundLive ? 'simulación, del motor' : 'simulación (ejemplo)'}>
          <p className="text-[12.5px] text-muted-foreground">
            {fund.members.length === 0 ? (
              'Todavía no hay aportes liquidados: el fondo no tiene unidades emitidas.'
            ) : (
              <>
                {decimal(fund.total_units)} unidades a {decimal(fund.nav, 4)}{' '}
                {currencySign(fund.currency)} cada una, repartidas entre{' '}
                {members(fund.members.length)}.
              </>
            )}
          </p>
          <button
            type="button"
            onClick={() => onOpen('fondo')}
            className="mt-3 rounded-sm text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
          >
            Ver el fondo y cada miembro →
          </button>
        </Card>
      </div>

      <PaperVsBacktestSection />
      <LearningSection />
    </div>
  )
}

/** El detalle de la corrida abierta: del motor, de ejemplo o el aviso de que no está. */
function RunDetailSlot({
  runId,
  live,
  remote,
  onClose,
}: {
  runId: string
  live: boolean
  remote: ReturnType<typeof useRunDetail>
  onClose: () => void
}) {
  if (!live) {
    if (BACKTEST_RUN_DETAIL.run.run_id === runId)
      return <RunDetailView detail={BACKTEST_RUN_DETAIL} example onClose={onClose} />
    return (
      <Card title="Detalle de la corrida">
        <p role="status" className="text-[12.5px]">
          Sin gateway solo hay un detalle de ejemplo (la corrida{' '}
          <code className="text-[11px]">{BACKTEST_RUN_DETAIL.run.run_id}</code>).
        </p>
      </Card>
    )
  }
  if (remote.status === 'ok') return <RunDetailView detail={remote.data} onClose={onClose} />
  return (
    <Card title="Detalle de la corrida">
      {remote.status === 'loading' ? (
        <p role="status" className="text-[12.5px]">
          Pidiendo el detalle al motor…
        </p>
      ) : (
        <p role="alert" className="text-[12.5px]">
          No se pudo leer el detalle ({remote.message}).
        </p>
      )}
    </Card>
  )
}
