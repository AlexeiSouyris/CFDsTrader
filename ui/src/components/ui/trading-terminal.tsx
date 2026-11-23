/**
 * Piezas de la terminal de trading (fase 10, prototipo, #275): la franja de la cuenta con el modo
 * simulación, el aviso de kill switch, el ticket de orden y las posiciones abiertas.
 *
 * Diseño propio del sistema de `ui/` (vidrio, Manrope, pocos colores con sentido: verde y rojo
 * solo para dirección y P/L, ámbar solo para "simulación"). **No calcula nada:** el P/L, el
 * margen, el apalancamiento y la validación de riesgo llegan del motor; la UI los muestra.
 */
import { Check, OctagonAlert, ShieldCheck, X } from 'lucide-react'
import { useId, useState, type FormEvent, type ReactNode } from 'react'

import { AnimatedNumber } from '@/components/ui/animated-number'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal, isNegative, signed } from '@/lib/format'
import { cn } from '@/lib/utils'
import { newOrder } from '@/lib/paper-api'
import {
  REJECTION_LABEL,
  type Order,
  type OrderReport,
  type PaperAccount,
  type Position,
  type TicketVerdict,
} from '@/lib/paper-contracts'
import type { TicketDraft } from '@/mocks'

/** El indicador grande de que todo es simulación. */
export function PaperBadge() {
  return (
    <div
      role="status"
      aria-label="Modo simulación: no se envían órdenes reales"
      className="flex items-center gap-3 rounded-[18px] border border-[var(--chart-signal)]/50 bg-[var(--chart-signal)]/10 px-4 py-3"
    >
      <span className="relative flex size-2.5" aria-hidden="true">
        <span className="absolute inline-flex size-full animate-ping rounded-full bg-[var(--chart-signal)] opacity-60 motion-reduce:animate-none" />
        <span className="relative inline-flex size-2.5 rounded-full bg-[var(--chart-signal)]" />
      </span>
      <span>
        <strong className="block text-[15px] leading-none font-extrabold tracking-[0.12em] text-[var(--chart-signal)]">
          MODO SIMULACIÓN
        </strong>
        <span className="mt-1 block text-[11px] text-muted-foreground">
          <Term id="paper-trading">Paper</Term>: las órdenes van al broker simulado. No se envía
          ninguna orden real.
        </span>
      </span>
    </div>
  )
}

function Stat({
  label,
  children,
  hint,
  big = false,
}: {
  label: ReactNode
  children: ReactNode
  hint?: ReactNode
  big?: boolean
}) {
  return (
    <div className="min-w-0">
      <dt className="text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
        {label}
      </dt>
      <dd
        className={cn(
          'mt-1 leading-none font-extrabold whitespace-nowrap tracking-[-0.03em] tabular-nums',
          big ? 'text-[clamp(26px,3vw,38px)]' : 'text-[clamp(18px,1.8vw,22px)]',
        )}
      >
        {children}
      </dd>
      {hint && <p className="mt-1.5 text-[11px] text-muted-foreground">{hint}</p>}
    </div>
  )
}

const tone = (v: string) => (isNegative(v) ? 'text-down' : 'text-up')

/** Capital, P/L, margen y apalancamiento de la cuenta de simulación, tal como llegan. */
export function AccountStrip({ account }: { account: PaperAccount }) {
  const used = Number(account.daily_loss_used_pct)
  return (
    <section
      aria-label="Cuenta de simulación"
      className="glass relative grid gap-5 rounded-[20px] p-5 has-[[role=dialog]]:z-30 xl:grid-cols-[auto_minmax(0,1fr)] xl:items-center"
    >
      <PaperBadge />
      <dl className="grid grid-cols-2 gap-x-6 gap-y-5 sm:grid-cols-3 lg:grid-cols-6 xl:grid-cols-3 2xl:grid-cols-6">
        <Stat label="Capital" big hint={`${account.currency}, al ${dateTimeUtc(account.as_of)}`}>
          <AnimatedNumber value={account.equity} format={(t) => decimal(t, 2)} />
        </Stat>
        <Stat label={<Term id="pl">P/L abierto</Term>} hint="posiciones abiertas">
          <span className={tone(account.unrealized_pl)}>
            <AnimatedNumber value={account.unrealized_pl} format={(t) => signed(t, 2)} />
          </span>
        </Stat>
        <Stat label="P/L del día" hint="realizado">
          <span className={tone(account.realized_pl_today)}>
            <AnimatedNumber value={account.realized_pl_today} format={(t) => signed(t, 2)} />
          </span>
        </Stat>
        <Stat
          label={<Term id="margen">Margen</Term>}
          hint={`libre ${decimal(account.margin_free, 2)}`}
        >
          <AnimatedNumber value={account.margin_used} format={(t) => decimal(t, 2)} />
        </Stat>
        <Stat
          label={<Term id="apalancamiento">Apalancamiento</Term>}
          hint={`${account.open_positions} de ${account.max_positions} posiciones`}
        >
          {decimal(account.leverage)}:1
        </Stat>
        <Stat label="Pérdida del día usada" hint="del máximo diario (§10)">
          {decimal(account.daily_loss_used_pct)} %
          <span
            className="mt-2 block h-1.5 overflow-hidden rounded-full bg-border"
            role="meter"
            aria-label="Pérdida del día usada"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={used}
          >
            <span
              className={cn(
                'block h-full rounded-full',
                used >= 100 ? 'bg-down' : 'bg-foreground/60',
              )}
              style={{ width: `${Math.min(100, Math.max(0, used))}%` }}
            />
          </span>
        </Stat>
      </dl>
    </section>
  )
}

/** Aviso grande cuando el motor activó el kill switch: no se abre nada. */
export function KillSwitchBanner({ account }: { account: PaperAccount }) {
  if (!account.kill_switch.active) return null
  return (
    <div
      role="alert"
      className="flex flex-wrap items-start gap-3 rounded-[20px] border border-down/40 bg-down/10 p-4"
    >
      <OctagonAlert className="mt-0.5 size-5 shrink-0 text-down" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <p className="text-[15px] font-extrabold text-down">
          <Term id="kill-switch">Kill switch</Term> activo
        </p>
        <p className="mt-1 text-[13px]">{account.kill_switch.reason}</p>
        <p className="mt-1 text-[11.5px] text-muted-foreground">
          Desde el {dateTimeUtc(account.kill_switch.since)}. No se aprueban propuestas ni se abren
          posiciones hasta que el motor lo libere.
        </p>
      </div>
    </div>
  )
}

const SIDE = { long: 'Comprar', short: 'Vender' } as const
/** Lo que se escribe con coma se manda con punto: el decimal viaja como texto (contratos). */
const asDecimal = (s: string) => s.trim().replace(',', '.')
const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e))

/**
 * Ticket de orden en **simulación** (`order.v1`, modo paper). El stop es obligatorio y el tamaño
 * no se elige: lo decide el módulo de riesgo del motor. Primero se valida (`preview`,
 * `ticket_verdict.v1`, sin efectos) y, si el riesgo la acepta, se envía (`submit`,
 * `order_report.v1`). La UI solo muestra lo que responde el motor; no calcula riesgo.
 *
 * Validar y enviar usan el mismo `order_id`: el broker es idempotente, así que un doble clic o un
 * reintento nunca abren dos posiciones. Cambiar el ticket arma una orden nueva.
 */
export function OrderTicket({
  instruments,
  initial,
  examples = [],
  preview,
  submit,
  onFilled,
  locked,
}: {
  instruments: string[]
  initial: TicketDraft
  /** tickets de ejemplo para la maqueta sin gateway */
  examples?: { label: string; draft: TicketDraft }[]
  /** `null`: el motor no tiene respuesta (maqueta sin gateway) */
  preview: (order: Order) => Promise<TicketVerdict | null>
  /** sin `submit` (maqueta sin gateway) no se envía nada */
  submit?: (order: Order) => Promise<OrderReport>
  /** después de una orden ejecutada, para volver a pedir la cuenta y las posiciones */
  onFilled?: (report: OrderReport) => void
  /** motivo por el que no se puede operar (por ejemplo, kill switch) */
  locked?: string
}) {
  const id = useId()
  const [draft, setDraft] = useState<TicketDraft>(initial)
  const [missing, setMissing] = useState<string | null>(null)
  const [verdict, setVerdict] = useState<TicketVerdict | null | undefined>(undefined)
  const [report, setReport] = useState<OrderReport | 'mock' | null>(null)
  const [busy, setBusy] = useState<'preview' | 'submit' | null>(null)
  const [failure, setFailure] = useState<string | null>(null)
  const [order, setOrder] = useState<Order | null>(null)

  const change = (patch: Partial<TicketDraft>) => {
    setDraft((d) => ({ ...d, ...patch }))
    setOrder(null)
    setVerdict(undefined)
    setReport(null)
    setMissing(null)
    setFailure(null)
  }

  const validate = async (e: FormEvent) => {
    e.preventDefault()
    if (busy) return
    if (!draft.stop.trim()) return setMissing('El stop es obligatorio: sin stop no se abre nada.')
    setMissing(null)
    setFailure(null)
    setReport(null)
    const o = newOrder({
      instrument_id: draft.instrument_id,
      side: draft.side,
      stop: asDecimal(draft.stop),
      tp1: draft.tp1.trim() ? asDecimal(draft.tp1) : null,
    })
    setOrder(o)
    setBusy('preview')
    try {
      setVerdict(await preview(o))
    } catch (err) {
      setVerdict(undefined)
      setFailure(errorText(err))
    } finally {
      setBusy(null)
    }
  }

  const send = async () => {
    if (busy || !order) return
    if (!submit) return setReport('mock')
    setBusy('submit')
    setFailure(null)
    try {
      // la hora de la orden es la del envío: el gateway rechaza órdenes viejas
      const r = await submit({ ...order, created_at: new Date().toISOString() })
      setReport(r)
      if (r.status === 'filled') onFilled?.(r)
    } catch (err) {
      setFailure(errorText(err))
    } finally {
      setBusy(null)
    }
  }

  const field =
    'mt-1 w-full rounded-xl border border-border bg-transparent px-3 py-2.5 text-[15px] font-semibold tabular-nums outline-none focus-visible:border-ring'
  const label = 'text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase'

  return (
    <form onSubmit={validate} aria-label="Ticket de orden" className="grid gap-4">
      <div className="grid grid-cols-2 gap-2" role="group" aria-label="Dirección">
        {(['long', 'short'] as const).map((s) => (
          <button
            key={s}
            type="button"
            aria-pressed={draft.side === s}
            onClick={() => change({ side: s })}
            className={cn(
              'rounded-xl border px-3 py-2.5 text-[14px] font-extrabold transition-colors',
              draft.side === s
                ? s === 'long'
                  ? 'border-up/50 bg-up/15 text-up'
                  : 'border-down/50 bg-down/15 text-down'
                : 'border-border text-muted-foreground hover:text-foreground',
            )}
          >
            {SIDE[s]}
          </button>
        ))}
      </div>

      <label className={label}>
        Instrumento
        <select
          className={field}
          value={draft.instrument_id}
          onChange={(e) => change({ instrument_id: e.target.value })}
        >
          {instruments.map((i) => (
            <option key={i} value={i}>
              {i}
            </option>
          ))}
        </select>
      </label>

      <div className="grid grid-cols-2 gap-3">
        <label htmlFor={`${id}-stop`} className={label}>
          <Term id="stop-loss">Stop</Term> (obligatorio)
          <input
            id={`${id}-stop`}
            aria-label="Stop (obligatorio)"
            inputMode="decimal"
            required
            aria-required="true"
            className={field}
            value={draft.stop}
            onChange={(e) => change({ stop: e.target.value })}
          />
        </label>
        <label htmlFor={`${id}-tp1`} className={label}>
          TP1 (opcional)
          <input
            id={`${id}-tp1`}
            aria-label="TP1 (opcional)"
            inputMode="decimal"
            className={field}
            value={draft.tp1}
            onChange={(e) => change({ tp1: e.target.value })}
          />
        </label>
      </div>
      <p className="-mt-2 text-[11.5px] text-muted-foreground">
        Orden a mercado en simulación. El tamaño no se elige: lo calcula el módulo de riesgo del
        motor.
      </p>

      {missing && (
        <p role="alert" className="text-[12.5px] font-semibold text-down">
          {missing}
        </p>
      )}

      <button
        type="submit"
        disabled={Boolean(locked) || busy !== null}
        className="glass-strong flex items-center justify-center gap-2 rounded-full px-4 py-2.5 text-[13px] font-bold disabled:opacity-40"
      >
        <ShieldCheck className="size-4" aria-hidden="true" />
        {busy === 'preview' ? 'Validando…' : 'Validar con el motor'}
      </button>

      {failure && (
        <p role="alert" className="rounded-2xl border border-down/40 p-3 text-[12.5px]">
          <strong className="block font-bold text-down">No se pudo hablar con el motor.</strong>
          {failure}
        </p>
      )}

      {verdict === null && (
        <div role="status" className="rounded-2xl border border-border p-3 text-[12.5px]">
          <p className="font-semibold">Sin respuesta del motor.</p>
          <p className="mt-1 text-muted-foreground">
            La UI no calcula el riesgo. Sin gateway, la maqueta solo responde los ejemplos:
          </p>
          <div className="mt-2 flex flex-wrap gap-2">
            {examples.map((x) => (
              <button
                key={x.label}
                type="button"
                onClick={() => change(x.draft)}
                className="rounded-full border border-border px-3 py-1 text-[11.5px] font-bold hover:bg-foreground/5"
              >
                {x.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {verdict && <Verdict verdict={verdict} />}

      {verdict?.accepted &&
        (report ? (
          <Report report={report} />
        ) : (
          <button
            type="button"
            disabled={Boolean(locked) || busy !== null}
            onClick={send}
            className={cn(
              'rounded-full px-4 py-2.5 text-[14px] font-extrabold disabled:opacity-40',
              draft.side === 'long' ? 'bg-up text-white' : 'bg-down text-white',
            )}
          >
            {busy === 'submit' ? 'Enviando…' : `${SIDE[draft.side]} en simulación`}
          </button>
        ))}

      {locked && <p className="text-[12px] font-semibold text-down">{locked}</p>}
    </form>
  )
}

function Report({ report: r }: { report: OrderReport | 'mock' }) {
  if (r === 'mock')
    return (
      <p role="status" className="text-[12.5px] font-bold">
        Maqueta sin gateway: no se envió nada al broker simulado.
      </p>
    )
  if (r.status === 'rejected')
    return (
      <p role="status" className="rounded-2xl border border-down/40 p-3 text-[12.5px]">
        <strong className="block font-extrabold text-down">El broker simulado la rechazó.</strong>
        {r.rejection ? REJECTION_LABEL[r.rejection] : 'Sin motivo.'}
      </p>
    )
  return (
    <p role="status" className="rounded-2xl border border-up/40 p-3 text-[12.5px]">
      <strong className="block font-extrabold text-up">Ejecutada en simulación (paper).</strong>
      {r.size && <>Tamaño {decimal(r.size)} €/pt</>}
      {r.fill_price && <> a {decimal(r.fill_price)}</>}
      {r.commission && <>, comisión {decimal(r.commission, 2)} €</>}. Posición{' '}
      <code className="text-[11px]">{r.position_id}</code>, {dateTimeUtc(r.at)}.
    </p>
  )
}

function Verdict({ verdict: v }: { verdict: TicketVerdict }) {
  const or = (x: string | null, f: (t: string) => string) => (x === null ? '—' : f(x))
  return (
    <section
      aria-label="Validación de riesgo del motor"
      className={cn(
        'rounded-2xl border p-3',
        v.accepted ? 'border-up/40 bg-up/5' : 'border-down/40 bg-down/5',
      )}
    >
      <p className={cn('text-[13px] font-extrabold', v.accepted ? 'text-up' : 'text-down')}>
        {v.accepted ? 'El motor la acepta' : 'El motor la rechaza'}
      </p>
      {v.rejection && (
        <p className="mt-1 flex items-start gap-2 text-[12.5px]">
          <X className="mt-0.5 size-3.5 shrink-0 text-down" aria-hidden="true" />
          {REJECTION_LABEL[v.rejection]}
        </p>
      )}
      <dl className="mt-2 grid grid-cols-2 gap-2 text-[12px] sm:grid-cols-3">
        <div>
          <dt className="text-muted-foreground">Tamaño</dt>
          <dd className="font-bold tabular-nums">{or(v.size, (t) => `${decimal(t)} €/pt`)}</dd>
        </div>
        <div>
          <dt className="text-muted-foreground">Riesgo si salta el stop</dt>
          <dd className="font-bold tabular-nums">
            {or(v.risk, (t) => `${decimal(t, 2)} €`)}
            {v.risk_pct !== null && ` · ${decimal(v.risk_pct)} %`}
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">
            <Term id="apalancamiento">Apalanc.</Term>
          </dt>
          <dd className="font-bold tabular-nums">{or(v.leverage, (t) => `${decimal(t)}:1`)}</dd>
        </div>
        <div>
          <dt className="text-muted-foreground">
            <Term id="margen">Margen</Term>
          </dt>
          <dd className="font-bold tabular-nums">{or(v.margin, (t) => `${decimal(t, 2)} €`)}</dd>
        </div>
        <div>
          <dt className="text-muted-foreground">Precio de referencia</dt>
          <dd className="font-bold tabular-nums">{or(v.reference, (t) => decimal(t))}</dd>
        </div>
        <div>
          <dt className="text-muted-foreground">Vela usada</dt>
          <dd className="font-bold">{dateTimeUtc(v.as_of)}</dd>
        </div>
      </dl>
      {v.leverage_capped && (
        <p className="mt-2 flex items-start gap-2 text-[12px]">
          <Check className="mt-0.5 size-3.5 shrink-0 text-up" aria-hidden="true" />
          El tope de apalancamiento del instrumento redujo el tamaño.
        </p>
      )}
    </section>
  )
}

/** Posiciones abiertas de la simulación (`position.v1`), con el P/L que manda el motor. */
export function PositionsTable({ positions }: { positions: Position[] }) {
  if (positions.length === 0)
    return <p className="text-[13px] text-muted-foreground">No hay posiciones abiertas.</p>
  const th = 'py-2 px-2 font-bold'
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[720px] text-[13px]">
        <caption className="sr-only">Posiciones abiertas</caption>
        <thead>
          <tr className="border-b border-border text-left text-[10px] tracking-[0.1em] text-muted-foreground uppercase">
            <th className={th}>Instrumento</th>
            <th className={cn(th, 'text-right')}>Tamaño</th>
            <th className={cn(th, 'text-right')}>Entrada</th>
            <th className={cn(th, 'text-right')}>Último</th>
            <th className={cn(th, 'text-right')}>Stop · TP1</th>
            <th className={cn(th, 'text-right')}>P/L</th>
            <th className={cn(th, 'text-right')}>Margen</th>
            <th className={th}>Abierta</th>
          </tr>
        </thead>
        <tbody>
          {positions.map((p) => (
            <tr key={p.position_id} className="border-b border-border last:border-b-0">
              <td className="px-2 py-3">
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
              <td className="px-2 py-3 text-right tabular-nums">{decimal(p.size)} €/pt</td>
              <td className="px-2 py-3 text-right tabular-nums">{decimal(p.entry)}</td>
              <td className="px-2 py-3 text-right font-semibold tabular-nums">{decimal(p.last)}</td>
              <td className="px-2 py-3 text-right text-muted-foreground tabular-nums">
                {decimal(p.stop)} · {p.tp1 === null ? '—' : decimal(p.tp1)}
              </td>
              <td
                className={cn(
                  'px-2 py-3 text-right font-extrabold tabular-nums',
                  tone(p.unrealized_pl),
                )}
              >
                {signed(p.unrealized_pl, 2)} €
                <span className="block text-[11px] font-semibold">{signed(p.unrealized_r)} R</span>
              </td>
              <td className="px-2 py-3 text-right tabular-nums">{decimal(p.margin, 2)} €</td>
              <td className="px-2 py-3 text-muted-foreground">{dateTimeUtc(p.opened_at)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
