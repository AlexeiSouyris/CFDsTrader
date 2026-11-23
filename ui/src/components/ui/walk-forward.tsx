/**
 * Walk-forward y fuera de muestra de la Fase 5 en el detalle de una corrida (#316). Lee
 * `walk_forward_report.v1` tal cual: el último reporte del instrumento de la corrida.
 *
 * **No calcula nada** (CLAUDE.md §1): métricas, comprar y mantener, estabilidad y el cambio de
 * signo los calcula el motor. La sensibilidad **no elige ningún valor**: solo muestra qué tan
 * estables son las métricas. Es simulación con datos del pasado: no anticipa resultados.
 */
import type { WalkForwardReport, WfPeriod } from '@/lib/walk-forward-api'
import { useWalkForward } from '@/lib/walk-forward-api'
import { dateTimeUtc } from '@/lib/format'
import { dayYear, numOrDash, pctOrDash, signedPct, tone } from '@/lib/run-format'
import { cn } from '@/lib/utils'

const signedOrDash = (f: string | null | undefined) => (f == null ? '—' : signedPct(f))

/** Las celdas de un tramo: retorno, drawdown, Sharpe, operaciones y comprar y mantener. */
function PeriodCells({ p }: { p: WfPeriod }) {
  const m = p.metrics
  const bh = p.buy_and_hold
  if (!m)
    return (
      <td colSpan={5} className="px-2 py-2 text-muted-foreground">
        {dayYear(p.from)} – {dayYear(p.to)}: sin velas
      </td>
    )
  return (
    <>
      <td className={cn('px-2 py-2 text-right font-extrabold tabular-nums', tone(m.total_return))}>
        {signedPct(m.total_return)}
      </td>
      <td className="px-2 py-2 text-right tabular-nums">{pctOrDash(m.max_drawdown)}</td>
      <td className="px-2 py-2 text-right tabular-nums">{numOrDash(m.sharpe)}</td>
      <td className="px-2 py-2 text-right tabular-nums">{m.trades}</td>
      <td className={cn('px-2 py-2 text-right tabular-nums', bh && tone(bh.return))}>
        {bh ? signedPct(bh.return) : '—'}
      </td>
    </>
  )
}

const SUB = ['Retorno', 'Drawdown', 'Sharpe', 'Op.', 'Comprar y mantener']

function Folds({ r }: { r: WalkForwardReport }) {
  const a = r.test_aggregate
  return (
    <div className="overflow-x-auto">
      <table aria-label="Ventanas walk-forward" className="w-full min-w-[760px] text-[12px]">
        <thead className="text-[10.5px] text-muted-foreground uppercase">
          <tr>
            <th rowSpan={2} className="px-2 py-1 text-left">
              Ventana
            </th>
            <th colSpan={5} className="px-2 py-1 text-center">
              Entrenamiento
            </th>
            <th colSpan={5} className="border-l border-border px-2 py-1 text-center">
              Prueba
            </th>
          </tr>
          <tr>
            {[...SUB, ...SUB].map((s, i) => (
              <th
                key={`${s}-${i}`}
                className={cn('px-2 py-1 text-right', i === SUB.length && 'border-l border-border')}
              >
                {s}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {r.folds.map((f) => (
            <tr key={f.index} className="border-t border-border/60">
              <th scope="row" className="px-2 py-2 text-left font-normal">
                <strong>{f.index + 1}</strong>{' '}
                <span className="block text-[11px] text-muted-foreground">
                  {dayYear(f.train.from)} · {dayYear(f.test.from)} – {dayYear(f.test.to)}
                </span>
              </th>
              <PeriodCells p={f.train} />
              <PeriodCells p={f.test} />
            </tr>
          ))}
        </tbody>
      </table>
      <p aria-label="Resumen de las pruebas" className="mt-2 text-[11.5px] text-muted-foreground">
        Pruebas: {a.with_data} de {a.periods} con datos, {a.trades} operaciones, retorno medio{' '}
        <strong className={a.mean_return ? tone(a.mean_return) : undefined}>
          {signedOrDash(a.mean_return)}
        </strong>{' '}
        (mediana {signedOrDash(a.median_return)}), {a.positive} positivas, peor drawdown{' '}
        {pctOrDash(a.worst_drawdown)}, Sharpe medio {numOrDash(a.mean_sharpe)}. Superan a comprar y
        mantener {a.beat_buy_and_hold} de {a.with_buy_and_hold}.
      </p>
    </div>
  )
}

function Stability({ r }: { r: WalkForwardReport }) {
  if (r.stability.length === 0)
    return (
      <p className="text-[12.5px] text-muted-foreground">
        Este reporte no tiene sensibilidad (se corrió con --sin-sensibilidad).
      </p>
    )
  return (
    <div className="overflow-x-auto">
      <table
        aria-label="Estabilidad de los parámetros"
        className="w-full min-w-[620px] text-[12px]"
      >
        <thead className="text-[10.5px] text-muted-foreground uppercase">
          <tr>
            <th className="px-2 py-1 text-left">Parámetro</th>
            <th className="px-2 py-1 text-left">Por valor: retorno medio · Sharpe · op.</th>
            <th className="px-2 py-1 text-right">Diferencia</th>
            <th className="px-2 py-1 text-left">Signo</th>
          </tr>
        </thead>
        <tbody>
          {r.stability.map((s) => (
            <tr
              key={`${s.file}-${s.key}`}
              data-sign-change={s.same_sign ? undefined : 'true'}
              className={cn('border-t border-border/60', !s.same_sign && 'bg-down/10')}
            >
              <th scope="row" className="px-2 py-2 text-left font-normal">
                <code>{s.key}</code>
                <span className="block text-[11px] text-muted-foreground">{s.file}</span>
              </th>
              <td className="px-2 py-2">
                <ul className="flex flex-wrap gap-x-4 gap-y-1">
                  {s.by_value.map((v) => (
                    <li key={v.value}>
                      <strong>{v.value}</strong>:{' '}
                      <span className={v.mean_return ? tone(v.mean_return) : undefined}>
                        {signedOrDash(v.mean_return)}
                      </span>{' '}
                      · {numOrDash(v.mean_sharpe)} · {v.trades}
                    </li>
                  ))}
                </ul>
              </td>
              <td className="px-2 py-2 text-right tabular-nums">{pctOrDash(s.return_spread)}</td>
              <td className="px-2 py-2">
                {s.same_sign ? (
                  'mismo signo'
                ) : (
                  <strong className="text-down">cambia de signo</strong>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="mt-2 text-[11px] text-muted-foreground">
        Solo con el entrenamiento de cada ventana. Mide la estabilidad: no elige ningún valor ni
        cambia la estrategia.
      </p>
    </div>
  )
}

function Holdout({ r }: { r: WalkForwardReport }) {
  const h = r.holdout
  const m = h.metrics
  return (
    <section
      aria-label="Fuera de muestra"
      className="rounded-[14px] border-2 border-dashed border-border p-3"
    >
      <h4 className="text-[12.5px] font-bold">
        Fuera de muestra · {dayYear(h.from)} – {dayYear(h.to)}
      </h4>
      <p className="text-[11px] text-muted-foreground">
        Evaluado una sola vez, aparte y sin retocar, con los parámetros de config.
      </p>
      {m ? (
        <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 text-[12.5px] sm:grid-cols-5">
          <div>
            <dt className="text-[10.5px] text-muted-foreground uppercase">Retorno</dt>
            <dd className={cn('font-extrabold', tone(m.total_return))}>
              {signedPct(m.total_return)}
            </dd>
          </div>
          <div>
            <dt className="text-[10.5px] text-muted-foreground uppercase">Drawdown</dt>
            <dd>{pctOrDash(m.max_drawdown)}</dd>
          </div>
          <div>
            <dt className="text-[10.5px] text-muted-foreground uppercase">Sharpe</dt>
            <dd>{numOrDash(m.sharpe)}</dd>
          </div>
          <div>
            <dt className="text-[10.5px] text-muted-foreground uppercase">Operaciones</dt>
            <dd>{m.trades}</dd>
          </div>
          <div>
            <dt className="text-[10.5px] text-muted-foreground uppercase">Comprar y mantener</dt>
            <dd className={h.buy_and_hold ? tone(h.buy_and_hold.return) : undefined}>
              {h.buy_and_hold ? signedPct(h.buy_and_hold.return) : '—'}
            </dd>
          </div>
        </dl>
      ) : (
        <p className="mt-2 text-[12.5px] text-muted-foreground">Sin velas en el tramo.</p>
      )}
    </section>
  )
}

export function WalkForwardReportView({ report: r }: { report: WalkForwardReport }) {
  return (
    <div data-walk-forward="ready" className="grid gap-4">
      <p className="text-[11.5px] text-muted-foreground">
        {r.instrument_id} · velas {r.candle_source === 'futures' ? 'de futuros' : 'de IG'} ·{' '}
        {r.split.train_months} meses de entrenamiento, {r.split.validation_months} de prueba, paso{' '}
        {r.split.step_months} · calculado el {dateTimeUtc(r.computed_at)} · {r.report_id}
      </p>
      <Folds r={r} />
      <div>
        <h4 className="mb-2 text-[12.5px] font-bold">Sensibilidad de parámetros</h4>
        <Stability r={r} />
      </div>
      <Holdout r={r} />
    </div>
  )
}

/** El bloque en vivo: pide el último reporte del instrumento y lo muestra tal cual llega. */
export function WalkForwardLive({ instrument }: { instrument: string }) {
  const wf = useWalkForward(instrument)
  if (wf.status === 'loading')
    return (
      <p data-walk-forward="loading" className="text-[12.5px] text-muted-foreground">
        Cargando el walk-forward de {instrument}…
      </p>
    )
  if (wf.status === 'error')
    return (
      <p data-walk-forward="pending" className="text-[12.5px] text-muted-foreground">
        No se pudo leer el walk-forward de {instrument} ({wf.message}).
      </p>
    )
  if (wf.data === null)
    return (
      <p data-walk-forward="none" className="text-[12.5px] text-muted-foreground">
        Todavía no hay un reporte de walk-forward para {instrument}. Lo genera el ejemplo{' '}
        <code>walk_forward</code> del motor con un <code>report_id</code>.
      </p>
    )
  return <WalkForwardReportView report={wf.data} />
}
