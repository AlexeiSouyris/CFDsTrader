/**
 * Detalle de una corrida de backtest (#299): curva de capital y de drawdown, métricas, kill
 * switch, operaciones y comprar y mantener. Lee `backtest_run_detail.v1` tal cual.
 *
 * **No calcula nada** (CLAUDE.md §1): el drawdown de cada día, las métricas, el P/L y comprar y
 * mantener los calcula el motor. `Number` se usa solo para ubicar los puntos en el dibujo.
 * Es simulación con datos del pasado: no anticipa resultados.
 */
import { X } from 'lucide-react'
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'

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

/**
 * Cuántos decimales mostrar de un precio: los de la entrada (hasta 5). Las salidas con
 * slippage llegan con muchos decimales; se cortan solo en pantalla y el valor completo queda en
 * el `title`. Es formato, no un cálculo.
 */
function priceDigits(reference: string): number {
  const frac = reference.split('.')[1]?.length ?? 0
  return Math.min(frac, 5)
}

/** Un precio con los decimales de su referencia y el valor exacto al pasar el mouse. */
export function Price({ value, reference }: { value: string; reference?: string }) {
  return <span title={value}>{decimal(value, priceDigits(reference ?? value))}</span>
}

/** Cuántas operaciones se ven antes de "Mostrar más" (una corrida puede tener cientos). */
export const TRADES_PAGE = 25

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
        Esta corrida no guardó su curva: es de una versión anterior del motor.
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
        className="block w-full select-none tabular-nums"
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
        {Math.abs(yE(hi) - yE(initial)) > 12 && (
          <text
            x={width - 4}
            y={yE(hi) + 3}
            textAnchor="end"
            fontSize={10}
            fill="var(--muted-foreground)"
          >
            {decimal(hi.toFixed(0))}
          </text>
        )}
        {Math.abs(yE(lo) - yE(initial)) > 12 && (
          <text
            x={width - 4}
            y={yE(lo) + 3}
            textAnchor="end"
            fontSize={10}
            fill="var(--muted-foreground)"
          >
            {decimal(lo.toFixed(0))}
          </text>
        )}
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
  const [shown, setShown] = useState(TRADES_PAGE)
  const total = detail.trades.length
  if (total === 0)
    return (
      <p className="text-[12.5px] text-muted-foreground">
        La corrida no tiene operaciones guardadas.
      </p>
    )
  const th = 'px-2 py-2 font-bold'
  // En el celular y la tableta angosta cada fila es una tarjeta: el título de la columna va en `data-label`.
  const td =
    'px-2 py-2.5 max-md:p-0 max-md:before:block max-md:before:text-[10px] max-md:before:font-bold max-md:before:tracking-[0.1em] max-md:before:text-muted-foreground max-md:before:uppercase max-md:before:content-[attr(data-label)]'
  const rest = total - shown
  return (
    <>
      <div className="overflow-x-auto max-md:overflow-visible">
        <table
          aria-label="Operaciones de la corrida"
          className="w-full text-[12.5px] md:min-w-[600px] max-md:block"
        >
          <thead className="max-md:sr-only">
            <tr className="border-b border-border text-left text-[10px] tracking-[0.1em] text-muted-foreground uppercase">
              <th className={th}>Instrumento</th>
              <th className={th}>Entrada</th>
              <th className={th}>Salida</th>
              <th className={cn(th, 'text-right')}>P/L neto</th>
              <th className={th}>Motivo</th>
            </tr>
          </thead>
          <tbody className="max-md:grid max-md:gap-2 sm:max-md:grid-cols-2">
            {detail.trades.slice(0, shown).map(({ proposal: p, outcome: o }) => {
              const exit = o?.exits[o.exits.length - 1]
              return (
                <tr
                  key={p.id}
                  className="border-b border-border last:border-b-0 max-md:grid max-md:grid-cols-2 max-md:gap-x-3 max-md:gap-y-2 max-md:rounded-xl max-md:border max-md:p-3 max-md:last:border-b"
                >
                  <td className="px-2 py-2.5 whitespace-nowrap max-md:order-first max-md:self-center max-md:p-0">
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
                  <td data-label="Entrada" className={cn(td, 'tabular-nums')}>
                    {o ? (
                      <>
                        <Price value={o.entry_price} />
                        <span className="block text-[11px] text-muted-foreground">
                          {dayYear(o.entry_at)}
                        </span>
                      </>
                    ) : (
                      <span className="text-muted-foreground">sin resultado</span>
                    )}
                  </td>
                  <td data-label="Salida" className={cn(td, 'tabular-nums')}>
                    {exit && o ? (
                      <>
                        <Price value={exit.price} reference={o.entry_price} />
                        <span className="block text-[11px] text-muted-foreground">
                          {dayYear(exit.at)}
                        </span>
                      </>
                    ) : (
                      '—'
                    )}
                  </td>
                  <td
                    data-label="P/L neto"
                    className={cn(
                      td,
                      'text-right font-extrabold tabular-nums max-md:-order-1',
                      o && tone(o.net_pnl),
                    )}
                  >
                    {o ? signed(o.net_pnl, 2) : '—'}
                  </td>
                  <td data-label="Motivo" className={td}>
                    {o ? EXIT_LABEL[o.exit_reason] : '—'}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
      <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
        <p className="text-[11px] text-muted-foreground">
          P/L neto en la moneda de cada instrumento, con spread, slippage, comisión y financiación.
        </p>
        {rest > 0 && (
          <div className="flex flex-wrap gap-2">
            <span className="self-center text-[11px] text-muted-foreground tabular-nums">
              {shown} de {total}
            </span>
            <button
              type="button"
              onClick={() => setShown((n) => n + TRADES_PAGE)}
              className="rounded-full border border-border px-3 py-1.5 text-[12px] font-bold hover:bg-foreground/5"
            >
              Mostrar {Math.min(rest, TRADES_PAGE)} más
            </button>
            {rest > TRADES_PAGE && (
              <button
                type="button"
                onClick={() => setShown(total)}
                className="rounded-full px-3 py-1.5 text-[12px] font-bold text-muted-foreground hover:text-foreground"
              >
                Mostrar todas
              </button>
            )}
          </div>
        )}
      </div>
    </>
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
  // Al abrir, el foco va al título del detalle y la página lo trae a la vista: sin esto, con
  // teclado o lector de pantalla el detalle aparece más abajo sin aviso.
  // En `useLayoutEffect`: el foco queda puesto en el mismo commit que muestra el detalle (antes de
  // pintar), no en un efecto que corre después y que un test podía no esperar (#415).
  const title = useRef<HTMLHeadingElement>(null)
  useLayoutEffect(() => {
    const el = title.current
    if (!el) return
    el.focus({ preventScroll: true })
    const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
    el.scrollIntoView?.({ block: 'start', behavior: reduce ? 'auto' : 'smooth' })
  }, [])
  return (
    <section aria-label={`Detalle de la corrida ${r.run_id}`} className="grid scroll-mt-4 gap-3">
      <header className="flex flex-wrap items-start justify-between gap-3 px-1">
        <div className="min-w-0">
          <p className="text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase">
            Simulación · backtest{example && ' · datos de ejemplo'}
          </p>
          <h2
            ref={title}
            tabIndex={-1}
            className="mt-1 scroll-mt-24 text-[clamp(18px,2vw,24px)] font-bold tracking-[-0.03em] [overflow-wrap:anywhere] outline-none"
          >
            {r.run_id}
          </h2>
          <p className="mt-1 text-[12px] text-muted-foreground tabular-nums">
            {r.instruments.join(', ')} · {r.timeframe} · {dayYear(r.from)} – {dayYear(r.to)} ·
            capital inicial {decimal(r.initial_equity)}
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="flex shrink-0 items-center gap-1.5 rounded-full border border-border px-3 py-1.5 text-[12px] font-bold hover:bg-foreground/5"
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
                  <span className="min-w-0">
                    <strong>{b.instrument_id}</strong>{' '}
                    <span className="text-muted-foreground tabular-nums">
                      <Price value={b.entry_price} /> →{' '}
                      <Price value={b.exit_price} reference={b.entry_price} />
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

      <Block
        title="Kill switch"
        aside={
          events.length
            ? `${events.length} ${events.length === 1 ? 'evento' : 'eventos'}`
            : undefined
        }
      >
        {events.length === 0 ? (
          <p className="text-[12.5px] text-muted-foreground">
            {r.metrics.kill_switch_at
              ? `Se activó el ${dateTimeUtc(r.metrics.kill_switch_at)}.`
              : 'No se activó en esta corrida.'}
          </p>
        ) : (
          <ol aria-label="Eventos del kill switch" className="grid gap-1.5 text-[12.5px]">
            {events.map((e) => (
              <li key={`${e.event}-${e.at}`} className="flex flex-wrap gap-x-3 tabular-nums">
                <strong className={e.event === 'activated' ? 'text-down' : undefined}>
                  {e.event === 'activated' ? 'Activado' : 'Reactivado (solo backtest)'}
                </strong>
                <span>{dayYear(e.at)}</span>
                <span className="text-muted-foreground tabular-nums">
                  capital {decimal(e.equity, 2)}
                </span>
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
      <Block title="Walk-forward / fuera de muestra">
        {example ? (
          <p data-walk-forward="pending" className="text-[12.5px] text-muted-foreground">
            Los datos de ejemplo no tienen walk-forward: solo las corridas del motor lo traen.
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
