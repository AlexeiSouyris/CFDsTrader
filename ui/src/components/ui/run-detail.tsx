/**
 * Detalle de una corrida de backtest (#299): curva de capital y de drawdown, métricas, kill
 * switch, operaciones y comprar y mantener. Lee `backtest_run_detail.v1` tal cual.
 *
 * **No calcula nada** (CLAUDE.md §1): el drawdown de cada día, las métricas, el P/L y comprar y
 * mantener los calcula el motor. `Number` se usa solo para ubicar los puntos en el dibujo.
 * Es simulación con datos del pasado: no anticipa resultados.
 */
import { X } from 'lucide-react'
import type { ReactNode } from 'react'

import type { ExitReason, RunDetail as Detail } from '@/lib/backtest-contracts'
import { dateTimeUtc, decimal, signed } from '@/lib/format'
import { dayYear, numOrDash, pctOrDash, signedPct, tone } from '@/lib/run-format'
import { useElementWidth } from '@/lib/use-element-width'
import { cn } from '@/lib/utils'

import { WalkForwardLive } from './walk-forward'

const EXIT_LABEL: Record<ExitReason, string> = {
  stop: 'Stop',
  tp1: 'TP1',
  break_even: 'Break-even',
  trailing: 'Trailing',
  time: 'Por tiempo',
  forced: 'Cierre forzado',
  end_of_data: 'Fin de los datos',
}

function Block({
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
    <section className={cn('glass relative min-w-0 rounded-[20px] p-4', className)}>
      <header className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
        <h3 className="text-[14px] font-bold">{title}</h3>
        {aside && <span className="text-[11px] text-muted-foreground">{aside}</span>}
      </header>
      {children}
    </section>
  )
}

/**
 * Las dos curvas del motor, una arriba de la otra y con el mismo eje de días: el capital (con
 * el capital inicial como referencia) y el drawdown (hacia abajo, desde 0). Las líneas
 * verticales son los eventos del kill switch.
 */
function Curves({ detail }: { detail: Detail }) {
  const [ref, width] = useElementWidth<HTMLDivElement>(720, 300)
  const points = detail.curve?.points ?? []
  const events = detail.run.metrics.kill_switch_events ?? []
  if (points.length === 0)
    return (
      <p className="text-[12.5px] text-muted-foreground">
        Esta corrida no guardó su curva (es anterior a #299).
      </p>
    )
  const axis = 64
  const plotW = width - axis
  const eqH = 200
  const ddH = 90
  const gap = 26
  const height = eqH + gap + ddH + 20
  const n = points.length
  const x = (i: number) => (n === 1 ? plotW / 2 : (i / (n - 1)) * plotW)
  const eq = points.map((p) => Number(p.equity))
  const initial = Number(detail.run.initial_equity)
  const lo = Math.min(initial, ...eq)
  const hi = Math.max(initial, ...eq)
  const pad = (hi - lo) * 0.08 || 1
  const yE = (v: number) => 6 + (1 - (v - (lo - pad)) / (hi - lo + 2 * pad)) * (eqH - 6)
  const ddMax = Math.max(...points.map((p) => Number(p.drawdown)), 0.0001)
  const top = eqH + gap
  const yD = (v: number) => top + (v / ddMax) * ddH
  const line = (ys: number[]) =>
    ys.map((y, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y.toFixed(1)}`).join('')
  const eqPath = line(eq.map(yE))
  const ddPath = line(points.map((p) => yD(Number(p.drawdown))))
  const ddArea = `${ddPath}L${x(n - 1).toFixed(1)},${top}L${x(0).toFixed(1)},${top}Z`
  const index = (iso: string) => {
    const d = iso.slice(0, 10)
    const k = points.findIndex((p) => p.date >= d)
    return k < 0 ? n - 1 : k
  }
  const last = points[n - 1]
  const worst = detail.run.metrics.max_drawdown
  const labels = [0, Math.floor((n - 1) / 2), n - 1]
  return (
    <div ref={ref} className="chart-enter w-full" data-swipe-ignore>
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={`Curva de capital de ${n} días, de ${decimal(points[0].equity)} a ${decimal(last.equity)}, y drawdown máximo de ${pctOrDash(worst, 2)}.`}
        className="block w-full select-none"
      >
        <text x={0} y={eqH + 16} fontSize={10} fill="var(--muted-foreground)">
          Drawdown
        </text>
        {/* capital inicial */}
        <line
          x1={0}
          x2={plotW}
          y1={yE(initial)}
          y2={yE(initial)}
          stroke="var(--chart-grid)"
          strokeDasharray="4 4"
        />
        <text
          x={width - 4}
          y={yE(initial) + 3}
          textAnchor="end"
          fontSize={10}
          fill="var(--muted-foreground)"
        >
          {decimal(detail.run.initial_equity)}
        </text>
        <text
          x={width - 4}
          y={yE(hi) + 3}
          textAnchor="end"
          fontSize={10}
          fill="var(--muted-foreground)"
        >
          {decimal(hi.toFixed(0))}
        </text>
        <text
          x={width - 4}
          y={yE(lo) + 3}
          textAnchor="end"
          fontSize={10}
          fill="var(--muted-foreground)"
        >
          {decimal(lo.toFixed(0))}
        </text>
        <path
          data-curve="equity"
          d={eqPath}
          fill="none"
          stroke="var(--chart-line)"
          strokeWidth={1.6}
        />

        {/* drawdown */}
        <line x1={0} x2={plotW} y1={top} y2={top} stroke="var(--chart-grid)" />
        <path
          data-curve="drawdown"
          d={ddArea}
          fill="var(--chart-down)"
          fillOpacity={0.18}
          stroke="var(--chart-down)"
          strokeWidth={1.2}
        />
        <text
          x={width - 4}
          y={top + 3}
          textAnchor="end"
          fontSize={10}
          fill="var(--muted-foreground)"
        >
          0 %
        </text>
        <text
          x={width - 4}
          y={top + ddH}
          textAnchor="end"
          fontSize={10}
          fill="var(--muted-foreground)"
        >
          −{pctOrDash(worst, 2)}
        </text>

        {/* kill switch */}
        {events.map((e) => {
          const ex = x(index(e.at))
          const color = e.event === 'activated' ? 'var(--chart-down)' : 'var(--chart-signal)'
          return (
            <g key={`${e.event}-${e.at}`} data-kill-switch={e.event}>
              <line
                x1={ex}
                x2={ex}
                y1={0}
                y2={top + ddH}
                stroke={color}
                strokeDasharray="3 3"
                strokeOpacity={0.8}
              />
              <title>{`Kill switch ${e.event === 'activated' ? 'activado' : 'reactivado'}: ${dateTimeUtc(e.at)}`}</title>
            </g>
          )
        })}

        {labels.map((i) => (
          <text
            key={i}
            x={Math.max(30, Math.min(plotW - 30, x(i)))}
            y={height - 4}
            textAnchor="middle"
            fontSize={10}
            fill="var(--muted-foreground)"
          >
            {dayYear(points[i].date)}
          </text>
        ))}
      </svg>
    </div>
  )
}

function Metrics({ detail }: { detail: Detail }) {
  const m = detail.run.metrics
  const items: [string, ReactNode][] = [
    ['Retorno', <span className={tone(m.total_return)}>{signedPct(m.total_return)}</span>],
    ['Drawdown máx.', pctOrDash(m.max_drawdown, 2)],
    ['Sharpe', numOrDash(m.sharpe)],
    ['Sortino', numOrDash(m.sortino)],
    ['Acierto', pctOrDash(m.win_rate)],
    ['Profit factor', numOrDash(m.profit_factor)],
    ['Operaciones', String(m.trades)],
    ['Exposición', pctOrDash(m.exposure)],
  ]
  return (
    <dl
      aria-label="Métricas de la corrida"
      className="grid grid-cols-2 gap-x-6 gap-y-4 sm:grid-cols-4"
    >
      {items.map(([label, value]) => (
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
  )
}

function Trades({ detail }: { detail: Detail }) {
  if (detail.trades.length === 0)
    return (
      <p className="text-[12.5px] text-muted-foreground">
        La corrida no tiene operaciones guardadas.
      </p>
    )
  const th = 'py-2 px-2 font-bold'
  return (
    <div className="overflow-x-auto">
      <table aria-label="Operaciones de la corrida" className="w-full min-w-[760px] text-[12.5px]">
        <thead>
          <tr className="border-b border-border text-left text-[10px] tracking-[0.1em] text-muted-foreground uppercase">
            <th className={th}>Instrumento</th>
            <th className={th}>Entrada</th>
            <th className={th}>Salida</th>
            <th className={cn(th, 'text-right')}>P/L neto</th>
            <th className={th}>Motivo</th>
          </tr>
        </thead>
        <tbody>
          {detail.trades.map(({ proposal: p, outcome: o }) => {
            const exit = o?.exits[o.exits.length - 1]
            return (
              <tr key={p.id} className="border-b border-border last:border-b-0">
                <td className="px-2 py-2.5">
                  <span className="font-extrabold">{p.instrument_id}</span>{' '}
                  <span
                    className={cn(
                      'ml-1 rounded-md px-1.5 py-0.5 text-[10.5px] font-bold',
                      p.side === 'long' ? 'bg-up/10 text-up' : 'bg-down/10 text-down',
                    )}
                  >
                    {p.side === 'long' ? 'Largo' : 'Corto'}
                  </span>
                </td>
                <td className="px-2 py-2.5 tabular-nums">
                  {o ? (
                    <>
                      {decimal(o.entry_price)}
                      <span className="block text-[11px] text-muted-foreground">
                        {dayYear(o.entry_at)}
                      </span>
                    </>
                  ) : (
                    <span className="text-muted-foreground">sin resultado</span>
                  )}
                </td>
                <td className="px-2 py-2.5 tabular-nums">
                  {exit ? (
                    <>
                      {decimal(exit.price)}
                      <span className="block text-[11px] text-muted-foreground">
                        {dayYear(exit.at)}
                      </span>
                    </>
                  ) : (
                    '—'
                  )}
                </td>
                <td
                  className={cn(
                    'px-2 py-2.5 text-right font-extrabold tabular-nums',
                    o && tone(o.net_pnl),
                  )}
                >
                  {o ? signed(o.net_pnl, 2) : '—'}
                </td>
                <td className="px-2 py-2.5">{o ? EXIT_LABEL[o.exit_reason] : '—'}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
      <p className="mt-2 text-[11px] text-muted-foreground">
        P/L neto en la moneda de cada instrumento, con spread, slippage, comisión y financiación.
      </p>
    </div>
  )
}

export function RunDetailView({
  detail,
  example = false,
  onClose,
}: {
  detail: Detail
  /** son los datos de ejemplo (sin gateway) */
  example?: boolean
  onClose: () => void
}) {
  const r = detail.run
  const events = r.metrics.kill_switch_events ?? []
  const bh = detail.curve?.buy_and_hold ?? []
  return (
    <section aria-label={`Detalle de la corrida ${r.run_id}`} className="grid gap-3">
      <header className="flex flex-wrap items-start justify-between gap-3 px-1">
        <div className="min-w-0">
          <p className="text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase">
            Simulación · backtest{example && ' · datos de ejemplo'}
          </p>
          <h2 className="mt-1 text-[clamp(18px,2vw,24px)] font-bold tracking-[-0.03em] break-all">
            {r.run_id}
          </h2>
          <p className="mt-1 text-[12px] text-muted-foreground">
            {r.instruments.join(', ')} · {r.timeframe} · {dayYear(r.from)} – {dayYear(r.to)} ·
            capital inicial {decimal(r.initial_equity)}
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="flex items-center gap-1.5 rounded-full border border-border px-3 py-1.5 text-[12px] font-bold hover:bg-foreground/5"
        >
          <X className="size-3.5" aria-hidden="true" /> Cerrar el detalle
        </button>
      </header>

      <Block title="Capital y drawdown" aside="calculados por el motor, día por día">
        <Curves detail={detail} />
      </Block>

      <div className="grid gap-3 lg:grid-cols-[1.6fr_1fr]">
        <Block title="Métricas" aside="con costos incluidos">
          <Metrics detail={detail} />
        </Block>
        <Block title="Comprar y mantener" aside="mismo período y costos">
          {bh.length === 0 ? (
            <p className="text-[12.5px] text-muted-foreground">
              Esta corrida no tiene comprar y mantener.
            </p>
          ) : (
            <ul aria-label="Comprar y mantener" className="grid gap-2">
              {bh.map((b) => (
                <li
                  key={b.instrument_id}
                  className="flex items-baseline justify-between gap-3 text-[12.5px]"
                >
                  <span>
                    <strong>{b.instrument_id}</strong>{' '}
                    <span className="text-muted-foreground">
                      {decimal(b.entry_price)} → {decimal(b.exit_price)}
                    </span>
                  </span>
                  <span className={cn('font-extrabold tabular-nums', tone(b.total_return))}>
                    {signedPct(b.total_return)}
                  </span>
                </li>
              ))}
              <li className="pt-1 text-[11px] text-muted-foreground">
                Retorno de la corrida:{' '}
                <strong className={tone(r.metrics.total_return)}>
                  {signedPct(r.metrics.total_return)}
                </strong>
              </li>
            </ul>
          )}
        </Block>
      </div>

      <Block title="Kill switch" aside={events.length ? `${events.length} eventos` : undefined}>
        {events.length === 0 ? (
          <p className="text-[12.5px] text-muted-foreground">
            {r.metrics.kill_switch_at
              ? `Se activó el ${dateTimeUtc(r.metrics.kill_switch_at)}.`
              : 'No se activó en esta corrida.'}
          </p>
        ) : (
          <ol aria-label="Eventos del kill switch" className="grid gap-1.5 text-[12.5px]">
            {events.map((e) => (
              <li key={`${e.event}-${e.at}`} className="flex flex-wrap gap-x-3">
                <strong className={e.event === 'activated' ? 'text-down' : undefined}>
                  {e.event === 'activated' ? 'Activado' : 'Reactivado (solo backtest)'}
                </strong>
                <span>{dayYear(e.at)}</span>
                <span className="text-muted-foreground">capital {decimal(e.equity)}</span>
              </li>
            ))}
          </ol>
        )}
      </Block>

      <Block title="Operaciones" aside={`${detail.trades.length} guardadas`}>
        <Trades detail={detail} />
      </Block>

      {/*
        Walk-forward y fuera de muestra (fase 5, #298 y #316): el último `walk_forward_report.v1`
        de cada instrumento de la corrida, tal cual lo manda el motor. Con los datos de ejemplo no
        se inventa ninguno.
      */}
      <Block title="Walk-forward / fuera de muestra" aside="fase 5">
        {example ? (
          <p data-walk-forward="pending" className="text-[12.5px] text-muted-foreground">
            Los datos de ejemplo no tienen walk-forward: llega del motor con la fase 5, desde su
            propio contrato.
          </p>
        ) : (
          <div className="grid gap-4">
            {r.instruments.map((id) => (
              <WalkForwardLive key={id} instrument={id} />
            ))}
          </div>
        )}
      </Block>
    </section>
  )
}
