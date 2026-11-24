/**
 * Una propuesta del motor para que la apruebe el responsable (spec §6, fase 10, #171).
 * Muestra lo que trae `proposal.v1` y lo que todavía no trae (vencimiento y probabilidad de §13).
 * No calcula nada: ni el riesgo, ni el tamaño, ni la probabilidad. Aprobar es una maqueta.
 */
import { Check, Clock, X } from 'lucide-react'
import { useEffect, useState } from 'react'

import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal } from '@/lib/format'
import { cn } from '@/lib/utils'
import type { BreakdownComponent, PendingProposal } from '@/mocks'

const COMPONENT: Record<BreakdownComponent, { label: string; term?: string }> = {
  trend: { label: 'Tendencia de 1h', term: 'tendencia' },
  sma_side: { label: 'Lado de la SMA50', term: 'sma' },
  candle_pattern: { label: 'Martillo o envolvente', term: 'martillo' },
  double_break: { label: 'Ruptura de doble suelo o techo', term: 'doble-techo' },
  level_rejection: { label: 'Rechazo en soporte o resistencia', term: 'soporte' },
  sweep: { label: 'Sweep de liquidez externa', term: 'sweep' },
  fvg_or_order_block: { label: 'Toque de FVG u order block', term: 'fvg' },
  recent_choch: { label: 'CHoCH reciente', term: 'choch' },
  news: { label: 'Ajuste por noticias' },
}

type Decision = 'approved' | 'rejected' | null

function useRemaining(until: string) {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(t)
  }, [])
  return Math.max(0, Math.floor((Date.parse(until) - now) / 1000))
}

export function ProposalCard({
  item,
  canApprove,
  locked,
}: {
  item: PendingProposal
  canApprove: boolean
  /** motivo por el que no se puede aprobar (por ejemplo, kill switch activo) */
  locked?: string
}) {
  const { proposal: p, extras } = item
  const [decision, setDecision] = useState<Decision>(null)
  const left = useRemaining(extras.expiresAt)
  const expired = left === 0
  const long = p.side === 'long'
  const prob = extras.probability

  return (
    <article
      aria-label={`Propuesta ${p.instrument_id} ${long ? 'largo' : 'corto'}`}
      className="glass relative @container min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30 sm:p-5"
    >
      <header className="flex flex-wrap items-center gap-2">
        <h3 className="text-[18px] leading-none font-extrabold tracking-tight">
          {p.instrument_id}
        </h3>
        <span
          className={cn(
            'rounded-md px-2 py-0.5 text-[11px] font-bold',
            long ? 'bg-up/10 text-up' : 'bg-down/10 text-down',
          )}
        >
          <Term id="largo-corto">{long ? 'Largo' : 'Corto'}</Term>
        </span>
        <span className="text-[11px] text-muted-foreground tabular-nums">
          señal {dateTimeUtc(p.signal_at)} · {p.source === 'paper' ? 'simulación' : p.source}
        </span>
        <span
          className={cn(
            'ml-auto flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-bold whitespace-nowrap tabular-nums',
            expired ? 'bg-down/10 text-down' : 'glass-strong',
          )}
          aria-live="off"
        >
          <Clock className="size-3.5" aria-hidden="true" />
          {expired
            ? 'Vencida'
            : `vence en ${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}`}
        </span>
      </header>

      <dl className="mt-4 grid grid-cols-2 gap-x-4 gap-y-3 rounded-2xl border border-border p-3 @md:grid-cols-3 @2xl:grid-cols-5">
        {[
          ['Entrada', decimal(p.reference), undefined],
          ['Stop', decimal(p.stop), 'stop-loss'],
          ['TP1', decimal(p.tp1), 'take-profit'],
          ['Tamaño', `${decimal(p.size)} €/punto`, undefined],
          ['Apalancamiento', `${decimal(p.leverage)}:1`, 'apalancamiento'],
        ].map(([label, value, term]) => (
          <div key={label} className="min-w-0">
            <dt className="text-[10px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
              {term ? <Term id={term}>{label}</Term> : label}
            </dt>
            <dd className="mt-1 text-[14px] font-semibold whitespace-nowrap tabular-nums">
              {value}
            </dd>
          </div>
        ))}
      </dl>

      <div className="mt-4 grid gap-4 @xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
        <section aria-label="Desglose del puntaje">
          <h4 className="mb-2 flex items-baseline justify-between text-[12px] font-bold">
            Puntaje <span className="text-[20px] font-extrabold tabular-nums">{p.score}</span>
          </h4>
          <ul className="grid gap-1.5">
            {p.breakdown.map((b) => {
              const c = COMPONENT[b.component]
              return (
                <li key={b.component} className="flex items-center gap-2 text-[12px]">
                  <span className="min-w-0 flex-1">
                    {c.term ? <Term id={c.term}>{c.label}</Term> : c.label}
                  </span>
                  <span className="flex gap-0.5" aria-hidden="true">
                    {Array.from({ length: Math.abs(b.points) }, (_, i) => (
                      <i
                        key={i}
                        className={cn(
                          'h-2.5 w-4 rounded-sm',
                          b.points > 0 ? 'bg-[var(--chart-up)]' : 'bg-[var(--chart-down)]',
                        )}
                      />
                    ))}
                  </span>
                  <span className="w-7 text-right font-bold tabular-nums">
                    {b.points > 0 ? `+${b.points}` : b.points}
                  </span>
                </li>
              )
            })}
          </ul>
        </section>

        <section aria-label="Probabilidad" className="rounded-2xl border border-border p-3">
          <h4 className="text-[12px] font-bold">
            <Term id="probabilidad">Probabilidad</Term> de tocar TP1 antes que el stop
          </h4>
          {prob.status === 'ok' ? (
            <>
              <p className="mt-2 text-[28px] leading-none font-extrabold tabular-nums">
                {prob.pct} <span className="text-[14px]">%</span>
              </p>
              <p className="mt-1.5 text-[11px] text-muted-foreground tabular-nums">
                rango {prob.lowPct}–{prob.highPct} % · {prob.cases} casos del backtest
              </p>
            </>
          ) : (
            <p className="mt-2 text-[13px] font-semibold">
              Datos insuficientes
              <span className="block text-[11px] font-normal text-muted-foreground">
                {prob.cases} casos; hacen falta {prob.needed}.
              </span>
            </p>
          )}
          <p className="mt-2 text-[10.5px] text-muted-foreground">
            Es una frecuencia del pasado, no una promesa.
          </p>
        </section>
      </div>

      <footer className="mt-4 flex flex-wrap items-center gap-2 border-t border-border pt-4">
        {decision ? (
          <p role="status" className="text-[12px] font-bold">
            {decision === 'approved' ? 'Aprobada' : 'Rechazada'} (maqueta: no se envió nada).
          </p>
        ) : locked ? (
          <p className="text-[12px] font-semibold text-down">{locked}</p>
        ) : canApprove ? (
          <>
            <button
              type="button"
              disabled={expired}
              onClick={() => setDecision('approved')}
              className="flex min-h-10 items-center gap-1.5 rounded-full bg-primary px-4 py-2 text-[13px] font-bold text-primary-foreground hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-40"
            >
              <Check className="size-4" aria-hidden="true" /> Aprobar
            </button>
            <button
              type="button"
              disabled={expired}
              onClick={() => setDecision('rejected')}
              className="glass-strong flex min-h-10 items-center gap-1.5 rounded-full px-4 py-2 text-[13px] font-bold disabled:cursor-not-allowed disabled:opacity-40"
            >
              <X className="size-4" aria-hidden="true" /> Rechazar
            </button>
          </>
        ) : (
          <p className="text-[12px] text-muted-foreground">
            Solo el responsable del fondo aprueba o rechaza.
          </p>
        )}
      </footer>
    </article>
  )
}
