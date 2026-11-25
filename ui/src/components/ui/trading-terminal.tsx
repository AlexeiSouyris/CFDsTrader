/**
 * Piezas de la terminal de trading (fase 10, prototipo, #275): la franja de la cuenta con el modo
 * simulación, el aviso de kill switch, el ticket de orden y las posiciones abiertas.
 *
 * Diseño propio del sistema de `ui/` (vidrio, Manrope, pocos colores con sentido: verde y rojo
 * solo para dirección y P/L, ámbar solo para "simulación"). **No calcula nada:** el P/L, el
 * margen, el apalancamiento y la validación de riesgo llegan del motor; la UI los muestra.
 */
import { Check, Clock, OctagonAlert, ShieldCheck, X } from 'lucide-react'
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

/**
 * El indicador de que todo es simulación. En la barra de la Terminal (#552) va en su versión
 * compacta, siempre a la vista.
 */
export function PaperBadge({ compact = false }: { compact?: boolean }) {
  return (
    <div
      role="status"
      aria-label="Modo simulación: no se envían órdenes reales"
      title="Paper: las órdenes van al broker simulado. No se envía ninguna orden real."
      className={cn(
        'flex shrink-0 items-center gap-3 border border-[var(--chart-signal)]/50 bg-[var(--chart-signal)]/10',
        compact ? 'gap-2 rounded-full px-3 py-1.5' : 'rounded-[18px] px-4 py-3',
      )}
    >
      <span className="relative flex size-2.5" aria-hidden="true">
        <span className="absolute inline-flex size-full animate-ping rounded-full bg-[var(--chart-signal)] opacity-60 motion-reduce:animate-none" />
        <span className="relative inline-flex size-2.5 rounded-full bg-[var(--chart-signal)]" />
      </span>
      <span>
        <strong
          className={cn(
            'block leading-none font-extrabold text-[var(--chart-signal)]',
            compact ? 'text-[11.5px] tracking-[0.1em]' : 'text-[15px] tracking-[0.12em]',
          )}
        >
          MODO SIMULACIÓN
        </strong>
        {!compact && (
          <span className="mt-1 block text-[11px] text-muted-foreground">
            <Term id="paper-trading">Paper</Term>: las órdenes van al broker simulado. No se envía
            ninguna orden real.
          </span>
        )}
      </span>
    </div>
  )
}

function Stat({
  label,
  children,
  hint,
  big = false,
  className,
}: {
  label: ReactNode
  children: ReactNode
  hint?: ReactNode
  big?: boolean
  className?: string
}) {
  return (
    <div className={cn('min-w-0', className)} title={typeof hint === 'string' ? hint : undefined}>
      <dt className="text-[10px] font-bold tracking-[0.1em] whitespace-nowrap text-muted-foreground uppercase">
        {label}
      </dt>
      <dd
        className={cn(
          'mt-1 leading-none font-extrabold tracking-[-0.02em] whitespace-nowrap tabular-nums',
          big ? 'text-[22px]' : 'text-[15px]',
        )}
      >
        {children}
      </dd>
      {hint && (
        <p className="mt-1 truncate text-[10.5px] leading-snug text-muted-foreground">{hint}</p>
      )}
    </div>
  )
}

const tone = (v: string) => (isNegative(v) ? 'text-down' : 'text-up')

/**
 * La barra de la cuenta de simulación (#552): el modo simulación, el capital, el P/L, el margen,
 * el apalancamiento y la pérdida del día, tal como llegan del motor, en una sola franja.
 *
 * El capital va en su propia lista (#356): así no se pisa con el P/L abierto. El resto se acomoda
 * solo según el ancho; en el celular, en dos columnas.
 */
export function AccountStrip({ account, aside }: { account: PaperAccount; aside?: ReactNode }) {
  const used = Number(account.daily_loss_used_pct)
  return (
    <section
      aria-label="Cuenta de simulación"
      className="glass relative flex min-w-0 flex-wrap items-center gap-x-6 gap-y-3 rounded-[20px] px-4 py-3"
    >
      <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2">
        <PaperBadge compact />
        <dl>
          <Stat label="Capital" big hint={`${account.currency}, al ${dateTimeUtc(account.as_of)}`}>
            <AnimatedNumber value={account.equity} format={(t) => decimal(t, 2)} />
          </Stat>
        </dl>
      </div>
      <dl className="grid min-w-0 flex-1 grid-cols-2 gap-x-5 gap-y-3 sm:grid-cols-[repeat(auto-fit,minmax(7.5rem,1fr))]">
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
          label={<Term id="apalancamiento">Apalanc.</Term>}
          hint={`${account.open_positions} de ${account.max_positions} posiciones`}
        >
          {decimal(account.leverage)}:1
        </Stat>
        <Stat label="Pérdida del día" hint="usada del máximo diario (§10)">
          {decimal(account.daily_loss_used_pct)} %
          <span
            className="mt-1.5 block h-1 overflow-hidden rounded-full bg-border"
            role="meter"
            aria-label="Pérdida del día usada"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={used}
            aria-valuetext={`${decimal(account.daily_loss_used_pct)} % del máximo diario`}
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
      {aside}
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
/** Pantalla táctil (celular, tablet): ahí enviar pide una confirmación más (#307). */
const coarsePointer = () =>
  typeof window !== 'undefined' && window.matchMedia?.('(pointer: coarse)').matches === true

/**
 * Ticket de orden en **simulación** (`order.v1`, modo paper). El stop es obligatorio y el tamaño
 * no se elige: lo decide el módulo de riesgo del motor. Primero se valida (`preview`,
 * `ticket_verdict.v1`, sin efectos) y, si el riesgo la acepta, se envía (`submit`,
 * `order_report.v1`). La UI solo muestra lo que responde el motor; no calcula riesgo.
 *
 * Validar y enviar usan el mismo `order_id`: el broker es idempotente, así que un doble clic o un
 * reintento nunca abren dos posiciones. Cambiar el ticket arma una orden nueva.
 *
 * En una pantalla táctil (#307), enviar pide un paso de confirmación: un toque sin querer no
 * manda la orden. Siempre es simulación (paper).
 */
export function OrderTicket({
  instruments,
  initial,
  examples = [],
  preview,
  submit,
  onFilled,
  onReport,
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
  /** cada informe del broker simulado, con la orden enviada (la lista de órdenes de la sesión) */
  onReport?: (order: Order, report: OrderReport) => void
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
  const [confirming, setConfirming] = useState(false)

  const change = (patch: Partial<TicketDraft>) => {
    setDraft((d) => ({ ...d, ...patch }))
    setOrder(null)
    setConfirming(false)
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
    setConfirming(false)
    if (busy || !order) return
    if (!submit) return setReport('mock')
    setBusy('submit')
    setFailure(null)
    try {
      // la hora de la orden es la del envío: el gateway rechaza órdenes viejas
      const sent = { ...order, created_at: new Date().toISOString() }
      const r = await submit(sent)
      setReport(r)
      onReport?.(sent, r)
      if (r.status === 'filled') onFilled?.(r)
    } catch (err) {
      setFailure(errorText(err))
    } finally {
      setBusy(null)
    }
  }

  const field =
    'mt-1.5 block min-h-11 w-full rounded-xl border border-border bg-transparent px-3 py-2.5 text-[15px] font-semibold tracking-normal normal-case tabular-nums transition-colors outline-none hover:border-foreground/30 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40 aria-[invalid=true]:border-down/60'
  const label =
    'block min-w-0 text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase'

  return (
    <form
      onSubmit={validate}
      aria-label="Ticket de orden"
      aria-busy={busy !== null}
      className="@container grid gap-4"
    >
      <p
        data-paper-banner
        className="flex items-center gap-2 rounded-xl border border-[var(--chart-signal)]/50 bg-[var(--chart-signal)]/10 px-3 py-2 text-[11.5px] font-bold text-[var(--chart-signal)]"
      >
        <span className="tracking-[0.12em]">SIMULACIÓN</span>
        <span className="font-semibold text-muted-foreground">
          paper: no se envía ninguna orden real
        </span>
      </p>
      <div className="grid grid-cols-2 gap-2" role="group" aria-label="Dirección">
        {(['long', 'short'] as const).map((s) => (
          <button
            key={s}
            type="button"
            aria-pressed={draft.side === s}
            onClick={() => change({ side: s })}
            className={cn(
              'min-h-11 rounded-xl border px-3 py-2.5 text-[14px] font-extrabold transition-colors',
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
            aria-invalid={missing ? true : undefined}
            aria-describedby={missing ? `${id}-missing` : undefined}
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
        <p id={`${id}-missing`} role="alert" className="text-[12.5px] font-semibold text-down">
          {missing}
        </p>
      )}

      <button
        type="submit"
        disabled={Boolean(locked) || busy !== null}
        aria-describedby={locked ? `${id}-locked` : undefined}
        className="glass-strong flex min-h-11 items-center justify-center gap-2 rounded-full px-4 py-2.5 text-[13px] font-bold disabled:cursor-not-allowed disabled:opacity-40"
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
          <Report report={report} since={verdict.as_of} />
        ) : confirming ? (
          <div
            role="group"
            aria-label="Confirmar el envío"
            className="grid gap-2 rounded-2xl border border-[var(--chart-signal)]/50 p-3 text-[12.5px]"
          >
            <p>
              <strong>¿Enviar esta orden al broker simulado?</strong> {SIDE[draft.side]}{' '}
              {draft.instrument_id} con stop {draft.stop}. Es SIMULACIÓN: no se envía nada real.
            </p>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => setConfirming(false)}
                className="rounded-full border border-border px-3 py-2 font-bold"
              >
                Cancelar
              </button>
              <button
                type="button"
                disabled={Boolean(locked) || busy !== null}
                onClick={send}
                className={cn(
                  'rounded-full px-3 py-2 font-extrabold text-white disabled:opacity-40',
                  draft.side === 'long' ? 'bg-up' : 'bg-down',
                )}
              >
                Confirmar
              </button>
            </div>
          </div>
        ) : (
          <button
            type="button"
            disabled={Boolean(locked) || busy !== null}
            onClick={() => (coarsePointer() ? setConfirming(true) : void send())}
            className={cn(
              'min-h-11 rounded-full px-4 py-2.5 text-[14px] font-extrabold disabled:cursor-not-allowed disabled:opacity-40',
              draft.side === 'long' ? 'bg-up text-white' : 'bg-down text-white',
            )}
          >
            {busy === 'submit' ? 'Enviando…' : `${SIDE[draft.side]} en simulación`}
          </button>
        ))}

      {locked && (
        <p id={`${id}-locked`} className="text-[12px] font-semibold text-down">
          {locked}
        </p>
      )}
    </form>
  )
}

/**
 * Aviso de que el motor no tiene velas en vivo (`no_recent_candles`, #384): suele ser el cupo de
 * IG agotado. Es un aviso de espera, no un error: va en tono neutro (ni rojo de error ni el ámbar
 * reservado a "simulación") y con `role="status"`. La fecha es la que manda el motor (`as_of`); la UI no la calcula.
 */
function NoCandlesNotice({ since, className }: { since?: string; className?: string }) {
  return (
    <p
      role="status"
      className={cn(
        'flex items-start gap-2 rounded-2xl border border-border bg-foreground/5 p-3 text-[12.5px]',
        className,
      )}
    >
      <Clock className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span>
        <strong className="font-bold">
          {since ? `Sin velas en vivo desde ${dateTimeUtc(since)}.` : 'Sin velas en vivo.'}
        </strong>{' '}
        Vuelven cuando se renueve el cupo de IG.
      </span>
    </p>
  )
}

function Report({ report: r, since }: { report: OrderReport | 'mock'; since?: string }) {
  if (r === 'mock')
    return (
      <p role="status" className="text-[12.5px] font-bold">
        Maqueta sin gateway: no se envió nada al broker simulado.
      </p>
    )
  if (r.status === 'rejected' && r.rejection === 'no_recent_candles')
    return <NoCandlesNotice since={since} />
  if (r.status === 'pending')
    return (
      <p role="status" className="rounded-2xl border border-border p-3 text-[12.5px]">
        <strong className="block font-extrabold">Pendiente de la próxima vela.</strong>
        El motor la decide y la ejecuta (o la rechaza) en la apertura de la vela siguiente, desde{' '}
        {dateTimeUtc(r.at)}. Mirá las posiciones abiertas.
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
  // sin velas en vivo es una espera (cupo de IG), no un error: se avisa en tono neutro (#384)
  const noCandles = v.rejection === 'no_recent_candles'
  return (
    <section
      aria-label="Validación de riesgo del motor"
      className={cn(
        'rounded-2xl border p-3',
        v.accepted
          ? 'border-up/40 bg-up/5'
          : noCandles
            ? 'border-border'
            : 'border-down/40 bg-down/5',
      )}
    >
      <p
        className={cn(
          'text-[13px] font-extrabold',
          v.accepted ? 'text-up' : noCandles ? 'text-foreground' : 'text-down',
        )}
      >
        {v.accepted
          ? 'El motor la acepta'
          : noCandles
            ? 'En espera de datos'
            : 'El motor la rechaza'}
      </p>
      {noCandles && <NoCandlesNotice since={v.as_of} className="mt-2" />}
      {v.rejection && !noCandles && (
        <p className="mt-1 flex items-start gap-2 text-[12.5px]">
          <X className="mt-0.5 size-3.5 shrink-0 text-down" aria-hidden="true" />
          {REJECTION_LABEL[v.rejection]}
        </p>
      )}
      <dl className="mt-3 grid grid-cols-2 gap-x-3 gap-y-2.5 text-[12px] @md:grid-cols-3">
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
          <dd className="font-bold tabular-nums">{dateTimeUtc(v.as_of)}</dd>
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
    return (
      <p className="rounded-xl border border-dashed border-border px-4 py-6 text-center text-[13px] text-muted-foreground">
        <strong className="block text-[13.5px] font-bold text-foreground">
          No hay posiciones abiertas.
        </strong>
        Las que se abran desde el ticket aparecen acá, con el P/L del motor.
      </p>
    )
  const th = 'px-2 py-2 font-bold'
  // En el celular cada fila se apila como una tarjeta (#356): la cabecera queda para lectores de
  // pantalla y cada celda muestra su nombre con `data-label`. Sigue siendo la misma tabla.
  const cell =
    'px-2 py-3 max-md:p-0 max-md:text-left max-md:before:mb-0.5 max-md:before:block max-md:before:text-[10px] max-md:before:font-bold max-md:before:tracking-[0.08em] max-md:before:text-muted-foreground max-md:before:uppercase max-md:before:content-[attr(data-label)]'
  return (
    <div className="md:overflow-x-auto">
      <table className="w-full text-[13px] max-md:block md:min-w-[720px]">
        <caption className="sr-only">Posiciones abiertas</caption>
        <thead className="max-md:sr-only">
          <tr className="border-b border-border text-left text-[10px] tracking-[0.1em] text-muted-foreground uppercase">
            <th scope="col" className={th}>
              Instrumento
            </th>
            <th scope="col" className={cn(th, 'text-right')}>
              Tamaño
            </th>
            <th scope="col" className={cn(th, 'text-right')}>
              Entrada
            </th>
            <th scope="col" className={cn(th, 'text-right')}>
              Último
            </th>
            <th scope="col" className={cn(th, 'text-right')}>
              Stop · TP1
            </th>
            <th scope="col" className={cn(th, 'text-right')}>
              P/L
            </th>
            <th scope="col" className={cn(th, 'text-right')}>
              Margen
            </th>
            <th scope="col" className={th}>
              Abierta
            </th>
          </tr>
        </thead>
        <tbody className="max-md:grid max-md:gap-3">
          {positions.map((p) => (
            <tr
              key={p.position_id}
              className="border-b border-border last:border-b-0 max-md:grid max-md:grid-cols-2 max-md:gap-x-4 max-md:gap-y-3 max-md:rounded-2xl max-md:border max-md:p-4 max-md:last:border-b"
            >
              <td className="px-2 py-3 whitespace-nowrap max-md:col-span-2 max-md:p-0">
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
              <td data-label="Tamaño" className={cn(cell, 'text-right tabular-nums')}>
                {decimal(p.size)} €/pt
              </td>
              <td data-label="Entrada" className={cn(cell, 'text-right tabular-nums')}>
                {decimal(p.entry)}
              </td>
              <td data-label="Último" className={cn(cell, 'text-right font-semibold tabular-nums')}>
                {decimal(p.last)}
              </td>
              <td
                data-label="Stop · TP1"
                className={cn(
                  cell,
                  'text-right whitespace-nowrap text-muted-foreground tabular-nums',
                )}
              >
                {decimal(p.stop)} · {p.tp1 === null ? '—' : decimal(p.tp1)}
              </td>
              <td
                data-label="P/L"
                className={cn(
                  cell,
                  'text-right font-extrabold whitespace-nowrap tabular-nums',
                  tone(p.unrealized_pl),
                )}
              >
                {signed(p.unrealized_pl, 2)} €
                <span className="block text-[11px] font-semibold">{signed(p.unrealized_r)} R</span>
              </td>
              <td
                data-label="Margen"
                className={cn(cell, 'text-right whitespace-nowrap tabular-nums')}
              >
                {decimal(p.margin, 2)} €
              </td>
              <td
                data-label="Abierta"
                className={cn(cell, 'whitespace-nowrap text-muted-foreground tabular-nums')}
              >
                {dateTimeUtc(p.opened_at)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
