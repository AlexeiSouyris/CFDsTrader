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

import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal, isNegative, signed } from '@/lib/format'
import { cn } from '@/lib/utils'
import type { PaperAccount, PaperPosition, TicketDraft, TicketVerdict } from '@/mocks'

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
          <Term id="paper-trading">Paper</Term>: no hay broker y no se envía ninguna orden real.
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
          {decimal(account.equity, 2)}
        </Stat>
        <Stat label={<Term id="pl">P/L abierto</Term>} hint="posiciones abiertas">
          <span className={tone(account.unrealized_pl)}>{signed(account.unrealized_pl, 2)}</span>
        </Stat>
        <Stat label="P/L del día" hint="realizado">
          <span className={tone(account.realized_pl_today)}>
            {signed(account.realized_pl_today, 2)}
          </span>
        </Stat>
        <Stat
          label={<Term id="margen">Margen</Term>}
          hint={`libre ${decimal(account.margin_free, 2)}`}
        >
          {decimal(account.margin_used, 2)}
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

/**
 * Ticket de orden en simulación. El stop es obligatorio. La validación de riesgo la hace el
 * motor (`verdictFor`): la UI solo muestra su respuesta, o que no la hay.
 */
export function OrderTicket({
  instruments,
  initial,
  examples,
  verdictFor,
  locked,
}: {
  instruments: string[]
  initial: TicketDraft
  examples: { label: string; draft: TicketDraft }[]
  verdictFor: (draft: TicketDraft) => TicketVerdict | null
  /** motivo por el que no se puede operar (por ejemplo, kill switch) */
  locked?: string
}) {
  const id = useId()
  const [draft, setDraft] = useState<TicketDraft>(initial)
  const [missing, setMissing] = useState<string | null>(null)
  const [verdict, setVerdict] = useState<TicketVerdict | null | undefined>(undefined)
  const [sent, setSent] = useState(false)

  const change = (patch: Partial<TicketDraft>) => {
    setDraft((d) => ({ ...d, ...patch }))
    setVerdict(undefined)
    setSent(false)
    setMissing(null)
  }

  const validate = (e: FormEvent) => {
    e.preventDefault()
    if (!draft.size.trim()) return setMissing('Falta el tamaño.')
    if (!draft.stop.trim()) return setMissing('El stop es obligatorio: sin stop no se abre nada.')
    setMissing(null)
    setVerdict(verdictFor({ ...draft, size: asDecimal(draft.size), stop: asDecimal(draft.stop) }))
  }

  const field =
    'mt-1 w-full rounded-xl border border-border bg-transparent px-3 py-2.5 text-[15px] font-semibold tabular-nums outline-none focus-visible:border-ring'

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

      <label className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
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
        <label
          htmlFor={`${id}-size`}
          className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase"
        >
          Tamaño (€/punto)
          <input
            id={`${id}-size`}
            aria-label="Tamaño en € por punto"
            inputMode="decimal"
            className={field}
            value={draft.size}
            onChange={(e) => change({ size: e.target.value })}
          />
        </label>
        <label
          htmlFor={`${id}-stop`}
          className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase"
        >
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
      </div>

      {missing && (
        <p role="alert" className="text-[12.5px] font-semibold text-down">
          {missing}
        </p>
      )}

      <button
        type="submit"
        disabled={Boolean(locked)}
        className="glass-strong flex items-center justify-center gap-2 rounded-full px-4 py-2.5 text-[13px] font-bold disabled:opacity-40"
      >
        <ShieldCheck className="size-4" aria-hidden="true" /> Validar con el motor
      </button>

      {verdict === null && (
        <div role="status" className="rounded-2xl border border-border p-3 text-[12.5px]">
          <p className="font-semibold">Sin respuesta del motor.</p>
          <p className="mt-1 text-muted-foreground">
            La UI no calcula el riesgo. En esta maqueta, el motor solo responde los ejemplos:
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
        (sent ? (
          <p role="status" className="text-[12.5px] font-bold">
            Enviada a la simulación (maqueta: no se envió nada).
          </p>
        ) : (
          <button
            type="button"
            disabled={Boolean(locked)}
            onClick={() => setSent(true)}
            className={cn(
              'rounded-full px-4 py-2.5 text-[14px] font-extrabold disabled:opacity-40',
              draft.side === 'long' ? 'bg-up text-white' : 'bg-down text-white',
            )}
          >
            {SIDE[draft.side]} en simulación
          </button>
        ))}

      {locked && <p className="text-[12px] font-semibold text-down">{locked}</p>}
    </form>
  )
}

function Verdict({ verdict: v }: { verdict: TicketVerdict }) {
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
      <dl className="mt-2 grid grid-cols-3 gap-2 text-[12px]">
        <div>
          <dt className="text-muted-foreground">Riesgo</dt>
          <dd className="font-bold tabular-nums">
            {decimal(v.risk_eur, 2)} € · {decimal(v.risk_pct)} %
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">
            <Term id="apalancamiento">Apalanc.</Term>
          </dt>
          <dd className="font-bold tabular-nums">{decimal(v.leverage)}:1</dd>
        </div>
        <div>
          <dt className="text-muted-foreground">
            <Term id="margen">Margen</Term>
          </dt>
          <dd className="font-bold tabular-nums">{decimal(v.margin_required, 2)} €</dd>
        </div>
      </dl>
      <ul className="mt-3 grid gap-1.5">
        {v.checks.map((c) => (
          <li key={c.rule} className="flex items-start gap-2 text-[12px]">
            {c.ok ? (
              <Check className="mt-0.5 size-3.5 shrink-0 text-up" aria-label="cumple" />
            ) : (
              <X className="mt-0.5 size-3.5 shrink-0 text-down" aria-label="no cumple" />
            )}
            <span>
              <strong className="font-semibold">{c.rule}:</strong>{' '}
              <span className="text-muted-foreground">{c.detail}</span>
            </span>
          </li>
        ))}
      </ul>
    </section>
  )
}

/** Posiciones abiertas de la simulación, con el P/L que manda el motor. */
export function PositionsTable({ positions }: { positions: PaperPosition[] }) {
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
            <tr key={p.id} className="border-b border-border last:border-b-0">
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
                {decimal(p.stop)} · {decimal(p.tp1)}
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
              <td className="px-2 py-3 text-right tabular-nums">
                {decimal(p.margin, 2)} €
                <span className="block text-[11px] text-muted-foreground">
                  {decimal(p.leverage)}:1
                </span>
              </td>
              <td className="px-2 py-3 text-muted-foreground">{dateTimeUtc(p.opened_at)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
