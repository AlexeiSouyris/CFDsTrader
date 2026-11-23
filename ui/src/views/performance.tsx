/**
 * Pestaña Performance (#286): resumen general de la simulación. Las corridas de backtest con la
 * forma de `backtest_run.v1`, el fondo y las estadísticas de la última corrida.
 *
 * **No calcula nada** (CLAUDE.md §1): las métricas llegan del motor y se muestran tal cual; las
 * fracciones se pasan a % corriendo la coma en el texto (`percent`).
 *
 * Las corridas llegan del gateway (`GET /api/v1/backtest_runs`, `backtest_run.v1[]`, las más
 * nuevas primero, #292). Si no responde, se ven las corridas de ejemplo de `mocks/`, marcadas así.
 * TODO(#286): el fondo sigue saliendo de `mocks/` (`FUND`): el gateway todavía no lo expone. No se
 * inventan contratos: los tipos son los de `contracts/`.
 */
import { useState, type ReactNode } from 'react'

import { AnimatedNumber } from '@/components/ui/animated-number'
import { RunDetailView } from '@/components/ui/run-detail'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal } from '@/lib/format'
import { dayYear, numOrDash, pctOrDash, signedPct, tone } from '@/lib/run-format'
import { useBacktestRuns, useRunDetail } from '@/lib/paper-api'
import { cn } from '@/lib/utils'
import { BACKTEST_RUN_DETAIL, BACKTEST_RUNS, FUND, type BacktestRun } from '@/mocks'

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
    <Card className="flex min-h-28 flex-col justify-between">
      <span className="text-[11px] font-bold text-muted-foreground">{label}</span>
      <strong className="text-[clamp(22px,2.4vw,30px)] leading-none font-extrabold tracking-[-0.04em]">
        {value}
      </strong>
      <span className="text-[10.5px] text-muted-foreground">{detail}</span>
    </Card>
  )
}

function RunsTable({
  runs,
  open,
  onOpen,
}: {
  runs: BacktestRun[]
  open: string | null
  onOpen: (runId: string) => void
}) {
  return (
    <div className="overflow-x-auto">
      <table aria-label="Corridas de backtest" className="w-full min-w-[860px] text-[12.5px]">
        <thead>
          <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
            <th className="py-2 font-bold">Corrida</th>
            <th className="py-2 font-bold">Instrumentos</th>
            <th className="py-2 font-bold">TF</th>
            <th className="py-2 font-bold">Período</th>
            <th className="py-2 text-right font-bold">Operaciones</th>
            <th className="py-2 text-right font-bold">Retorno</th>
            <th className="py-2 text-right font-bold">DD máx.</th>
            <th className="py-2 text-right font-bold">Sharpe</th>
            <th className="py-2 text-right font-bold">Acierto</th>
            <th className="py-2 text-right font-bold">Kill switch</th>
          </tr>
        </thead>
        <tbody>
          {runs.map((r) => (
            <tr
              key={r.run_id}
              aria-selected={open === r.run_id}
              className="border-b border-border last:border-b-0 aria-selected:bg-foreground/5"
            >
              <td className="py-2.5">
                <button
                  type="button"
                  aria-expanded={open === r.run_id}
                  aria-label={`Ver el detalle de ${r.run_id}`}
                  onClick={() => onOpen(r.run_id)}
                  className="block text-left font-semibold underline-offset-4 hover:underline"
                >
                  {r.run_id}
                </button>
                <span className="text-[10.5px] text-muted-foreground">
                  {dateTimeUtc(r.finished_at)} ·{' '}
                  {r.costs.spread_source === 'config' ? 'spread de config' : 'spread medido'}
                </span>
              </td>
              <td className="py-2.5">{r.instruments.join(', ')}</td>
              <td className="py-2.5">{r.timeframe}</td>
              <td className="py-2.5 whitespace-nowrap">
                {dayYear(r.from)} – {dayYear(r.to)}
              </td>
              <td className="py-2.5 text-right tabular-nums">{r.metrics.trades}</td>
              <td className={cn('py-2.5 text-right tabular-nums', tone(r.metrics.total_return))}>
                {signedPct(r.metrics.total_return)}
              </td>
              <td className="py-2.5 text-right tabular-nums">
                {pctOrDash(r.metrics.max_drawdown, 2)}
              </td>
              <td className="py-2.5 text-right tabular-nums">{numOrDash(r.metrics.sharpe)}</td>
              <td className="py-2.5 text-right tabular-nums">{pctOrDash(r.metrics.win_rate)}</td>
              <td className="py-2.5 text-right">
                {r.metrics.kill_switch_at ? dayYear(r.metrics.kill_switch_at) : 'no'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

export function PerformanceView({ onOpen }: { onOpen: (view: string) => void }) {
  const remote = useBacktestRuns()
  const live = remote.status === 'ok'
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
          value={<AnimatedNumber value={FUND.nav} format={decimal} />}
          detail={`cierre del ${dateTimeUtc(FUND.navAt)}`}
        />
        <Kpi
          label="Valor del fondo"
          value={
            <>
              <AnimatedNumber value={FUND.total} format={decimal} /> €
            </>
          }
          detail={`${FUND.members.length} miembros, simulación`}
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
        <Card title="Fondo" aside="simulación">
          <p className="text-[12.5px] text-muted-foreground">
            {decimal(FUND.units)} unidades a {decimal(FUND.nav)} € cada una, repartidas entre{' '}
            {FUND.members.length} miembros.
          </p>
          <button
            type="button"
            onClick={() => onOpen('fondo')}
            className="mt-3 text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
          >
            Ver el fondo y cada miembro
          </button>
        </Card>
      </div>
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
