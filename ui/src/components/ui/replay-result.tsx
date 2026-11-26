/**
 * Resultado de una sesión de replay (#790, diseño §5.1 y §5.4), tal cual lo calificó el motor
 * (`exam_result.v1`) y en este orden a propósito: primero el **proceso** regla por regla, después
 * los **niveles**, al final el **R** (que se muestra y no aprueba ni reprueba) y recién ahí la
 * **fecha real**. Una operación ganadora sin stop reprueba; una perdedora bien hecha aprueba.
 */
import { Check, X } from 'lucide-react'
import type { ReactNode } from 'react'

import { dateTimeUtc, decimal, signed } from '@/lib/format'
import type { ExamResult, ExamRule, Outcome } from '@/lib/replay-contracts'

const RULE: Record<ExamRule['rule'], string> = {
  stop_present: 'Tiene stop',
  stop_side: 'Stop del lado correcto',
  stop_in_range: 'Stop a una distancia permitida (en ATR)',
  risk_within_limit: 'Riesgo dentro del límite',
  plan_written: 'Plan escrito antes de entrar',
  one_position: 'Una sola posición abierta a la vez',
  enough_trades: 'Operaciones suficientes para el examen',
  level_precision: 'Precisión de niveles suficiente',
}

const EXIT: Record<Outcome['exit'], string> = {
  stop: 'stop',
  stop_gap: 'stop (la apertura lo saltó)',
  target: 'objetivo',
  end: 'final del examen',
}

const VERDICT = (passed: boolean | null) =>
  passed === null ? 'Práctica: sin nota' : passed ? 'Proceso aprobado' : 'Proceso no aprobado'

function Rules({ rules }: { rules: ExamRule[] }) {
  return (
    <ul className="grid gap-1 text-[12.5px]">
      {rules.map((r) => {
        const Icon = r.passed ? Check : X
        return (
          <li key={r.rule} className="flex items-center gap-2">
            <Icon aria-hidden="true" className="size-3.5 shrink-0" />
            <span>{RULE[r.rule]}</span>
            <span className="text-muted-foreground">{r.passed ? 'cumplida' : 'no cumplida'}</span>
          </li>
        )
      })}
    </ul>
  )
}

function Part({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="grid gap-1.5 border-t border-border pt-2.5">
      <h3 className="text-[11px] font-extrabold tracking-[0.12em] uppercase">{title}</h3>
      {children}
    </section>
  )
}

export function ReplayResult({
  result: r,
  digits,
  barAt,
}: {
  result: ExamResult
  digits: number
  /** la vela por su número, si se llegó a ver (si no, «vela N») */
  barAt: (index: number) => string
}) {
  const pct = (p: string | null) => (p === null ? '—' : `${decimal(p)} %`)
  const rr = (v: string | null) => (v === null ? '—' : `${signed(v, 2)} R`)
  return (
    <article aria-labelledby="replay-resultado" className="panel grid gap-3 p-3">
      <h2 id="replay-resultado" className="text-[17px] font-bold">
        {VERDICT(r.passed)}
      </h2>

      <Part title="1. Proceso">
        {r.rules.length > 0 && <Rules rules={r.rules} />}
        {r.trades.map((t) => (
          <div key={t.order_id} className="grid gap-1">
            <p className="font-mono text-[12px] font-semibold">Entrada E{t.order_id}</p>
            <Rules rules={t.rules} />
          </div>
        ))}
        {r.rules.length + r.trades.length === 0 && (
          <p className="text-[12.5px] text-muted-foreground">Sin entradas ni reglas de sesión.</p>
        )}
      </Part>

      {r.levels && (
        <Part title="2. Niveles">
          <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-[12.5px] sm:grid-cols-4">
            {[
              ['Marcados', String(r.levels.marked)],
              ['Del motor', String(r.levels.engine)],
              ['Precisión', pct(r.levels.precision_pct)],
              ['Cobertura', pct(r.levels.coverage_pct)],
            ].map(([k, v]) => (
              <div key={k}>
                <dt className="text-muted-foreground">{k}</dt>
                <dd className="font-mono font-semibold">{v}</dd>
              </div>
            ))}
          </dl>
          <p className="text-[12px] text-muted-foreground">
            Tolerancia: {decimal(r.levels.tolerance, digits)} de precio. Los niveles del motor
            quedan dibujados en el gráfico.
          </p>
        </Part>
      )}

      <Part title={`${r.levels ? 3 : 2}. Resultado en R (no aprueba ni reprueba)`}>
        {r.trades.length === 0 ? (
          <p className="text-[12.5px] text-muted-foreground">No hubo entradas.</p>
        ) : (
          <ul className="grid gap-1 font-mono text-[12px]">
            {r.trades.map(({ order_id, outcome: o }) => (
              <li key={order_id}>
                E{order_id} ·{' '}
                {o
                  ? `entró en ${barAt(o.entry_index)} a ${decimal(o.entry_price, digits)}, salió por ${EXIT[o.exit]} a ${decimal(o.exit_price, digits)} · bruto ${rr(o.r_gross)} · costos ${rr(o.costs_r)} · neto ${rr(o.r_net)}${o.r_net === null ? ' (sin stop no hay R)' : ''}`
                  : 'sin vela siguiente para entrar'}
              </li>
            ))}
          </ul>
        )}
      </Part>

      <Part title={`${r.levels ? 4 : 3}. La fecha real`}>
        <p className="text-[12.5px]">
          {r.revealed.instrument_id} · {r.revealed.timeframe} ·{' '}
          {r.revealed.source === 'futures' ? 'velas de futuros' : 'velas del CFD de IG'}: del{' '}
          <time dateTime={r.revealed.first_bar_at}>{dateTimeUtc(r.revealed.first_bar_at, 0)}</time>{' '}
          al <time dateTime={r.revealed.last_bar_at}>{dateTimeUtc(r.revealed.last_bar_at, 0)}</time>
          .
        </p>
      </Part>
      <p className="text-[11.5px] text-muted-foreground">
        Calificó el motor el {dateTimeUtc(r.graded_at)}. Simulación con datos del pasado: no es una
        promesa de nada.
      </p>
    </article>
  )
}
