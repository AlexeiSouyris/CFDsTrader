/**
 * Estado de la operación (#314): la barra compacta de la Terminal y la lista de eventos del
 * Panel. Muestra `ops_status.v1` y `ops_event.v1` tal como llegan: **no calcula nada** (la
 * barra de pérdida usa el porcentaje del motor como ancho, recortado por CSS) y no tiene ningún
 * control para reiniciar el kill switch.
 */
import { OctagonAlert } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'

import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal } from '@/lib/format'
import type { Gate, OpsEvent, OpsStatus } from '@/lib/ops-api'
import { cn } from '@/lib/utils'

const GATE: Record<Gate, { label: string; tone: string }> = {
  ready: { label: 'Acepta órdenes', tone: 'border-up/40 text-up' },
  starting: {
    label: 'Arrancando: no acepta órdenes',
    tone: 'border-[var(--chart-signal)]/50 text-[var(--chart-signal)]',
  },
  stopping: { label: 'Apagándose: no acepta órdenes', tone: 'border-down/40 text-down' },
  unavailable: { label: 'Broker simulado no disponible', tone: 'border-down/40 text-down' },
}

function Chip({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <span
      className={cn(
        'inline-flex min-h-8 items-center gap-1.5 rounded-full border px-3 py-1 text-[11.5px] font-bold',
        className,
      )}
    >
      {children}
    </span>
  )
}

/** Banner rojo, sólido e inconfundible, mientras el kill switch esté activo. */
export function OpsKillSwitchBanner({ status }: { status: OpsStatus }) {
  const ks = status.account?.kill_switch
  return (
    <AnimatePresence initial={false}>
      {ks?.active && (
        <motion.div
          key="kill-switch"
          role="alert"
          data-kill-switch="active"
          initial={{ opacity: 0, y: -6 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -6 }}
          transition={{ duration: 0.25 }}
          className="flex flex-wrap items-start gap-3 rounded-[20px] bg-down p-4 text-white shadow-lg"
        >
          <OctagonAlert className="mt-0.5 size-6 shrink-0" aria-hidden="true" />
          <div className="min-w-0 flex-1">
            <p className="text-[16px] font-extrabold tracking-[0.02em] uppercase">
              Kill switch activo: no se opera
            </p>
            <p className="mt-1 text-[13px] font-semibold break-words">{ks.reason}</p>
            <p className="mt-1 text-[11.5px] opacity-90">
              Desde el {dateTimeUtc(ks.since)}. Lo libera el motor; desde acá no se puede reiniciar.
            </p>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}

/** Barra compacta: compuerta, kill switch, pérdida del día, apalancamiento y reconciliación. */
export function OpsStatusBar({ status }: { status: OpsStatus | null }) {
  if (!status) {
    return (
      <section
        aria-label="Estado de la operación"
        data-ops-status="offline"
        className="glass flex flex-wrap items-center gap-2 rounded-[20px] px-4 py-3 text-[12px] text-muted-foreground"
      >
        <Chip className="border-border">Sin conexión con el gateway</Chip>
        <span>No se sabe el estado del broker: no se muestra uno de ejemplo.</span>
      </section>
    )
  }
  const a = status.account
  const gate = GATE[status.gate]
  const rec = status.last_reconcile
  return (
    <section
      aria-label="Estado de la operación"
      data-ops-status={status.gate}
      className="glass flex flex-wrap items-center gap-x-4 gap-y-3 rounded-[20px] px-4 py-3"
    >
      <Chip className={gate.tone}>
        <span className="size-2 rounded-full bg-current" aria-hidden="true" />
        {gate.label}
      </Chip>
      {a ? (
        <>
          <Chip
            className={a.kill_switch.active ? 'border-down bg-down text-white' : 'border-border'}
          >
            <Term id="kill-switch">Kill switch</Term> {a.kill_switch.active ? 'ACTIVO' : 'apagado'}
          </Chip>
          <div className="grid min-w-[180px] flex-1 gap-1 text-[11.5px]">
            <span className="flex flex-wrap justify-between gap-x-2">
              <span className="font-bold">Pérdida del día</span>
              <span className="text-muted-foreground tabular-nums">
                {decimal(a.daily_loss_used_pct)} % del máximo ({decimal(a.daily_loss_limit_pct)} %
                del capital)
              </span>
            </span>
            <span
              className="block h-2 overflow-hidden rounded-full bg-border"
              role="meter"
              aria-label="Pérdida del día usada"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Number(a.daily_loss_used_pct)}
              aria-valuetext={`${decimal(a.daily_loss_used_pct)} % del máximo diario`}
            >
              <span
                data-daily-loss-bar
                className={cn(
                  'block h-full rounded-full transition-[width] duration-500',
                  a.kill_switch.active ? 'bg-down' : 'bg-foreground/60',
                )}
                style={{ width: `min(${a.daily_loss_used_pct}%, 100%)` }}
              />
            </span>
          </div>
          <span className="text-[11.5px] tabular-nums">
            <Term id="apalancamiento">Apalancamiento</Term> {decimal(a.leverage)}:1 ·{' '}
            {a.open_positions} de {a.max_positions} posiciones
          </span>
        </>
      ) : (
        <Chip className="border-down/40 text-down">Sin datos de la cuenta</Chip>
      )}
      {rec && (
        <span
          className={cn('text-[11.5px]', rec.clean ? 'text-muted-foreground' : 'text-down')}
          data-reconcile={rec.clean ? 'clean' : 'dirty'}
        >
          Reconciliación {rec.kind === 'startup' ? 'al arrancar' : 'periódica'}:{' '}
          {rec.clean
            ? `limpia (${rec.matched} coinciden)`
            : `${rec.mismatches} posiciones distintas y ${rec.drifts} órdenes que no se reconstruyen igual`}{' '}
          · {dateTimeUtc(rec.at)}
        </span>
      )}
    </section>
  )
}

const EVENT_LABEL: Record<string, string> = {
  order_received: 'Orden recibida',
  order_duplicate: 'Orden repetida',
  risk_decision: 'Decisión del riesgo',
  order_report: 'Informe de orden',
  kill_switch: 'Kill switch',
  alert: 'Alerta',
  startup_reconcile: 'Reconciliación al arrancar',
  shutdown: 'Parada',
}

const str = (v: unknown) => (typeof v === 'string' || typeof v === 'number' ? String(v) : null)

/** Un resumen legible de los campos que manda el motor (sin interpretarlos). */
function summary(e: OpsEvent): string {
  const f = e.fields
  const direct = str(f.message) ?? str(f.reason) ?? str(f.error) ?? str(f.detail)
  if (direct) return direct
  const report = (f.report ?? null) as Record<string, unknown> | null
  if (report) return [str(report.order_id), str(report.status)].filter(Boolean).join(' · ')
  if (e.event === 'risk_decision')
    return [str(f.order_id), f.accepted === true ? 'aceptada' : 'rechazada', str(f.rejection)]
      .filter(Boolean)
      .join(' · ')
  return [str(f.order_id), str(f.instrument_id), str(f.side), str(f.kind)]
    .filter(Boolean)
    .join(' · ')
}

const LEVEL: Record<OpsEvent['level'], string> = {
  info: 'border-border text-muted-foreground',
  warn: 'border-[var(--chart-signal)]/50 text-[var(--chart-signal)]',
  error: 'border-down/50 text-down',
}

/** Los últimos eventos del broker y alertas, el más nuevo primero. */
export function OpsEventFeed({ events }: { events: OpsEvent[] }) {
  if (events.length === 0) return <p className="text-[12.5px]">Todavía no hay eventos guardados.</p>
  return (
    <ol className="grid gap-2" aria-label="Eventos y alertas">
      <AnimatePresence initial={false}>
        {events.map((e, i) => (
          <motion.li
            key={`${e.ts}|${e.target}|${e.event}|${events.length - i}`}
            layout
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            data-level={e.level}
            className={cn(
              'flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 rounded-xl border px-3 py-2 text-[12px]',
              e.target === 'alert' || e.event === 'kill_switch'
                ? 'border-down/40 bg-down/5'
                : 'border-border',
            )}
          >
            <span className="text-muted-foreground tabular-nums">{dateTimeUtc(e.ts)}</span>
            <span
              className={cn(
                'rounded-full border px-2 text-[10px] font-bold uppercase',
                LEVEL[e.level],
              )}
            >
              {e.level}
            </span>
            <span className="font-semibold">{EVENT_LABEL[e.event] ?? e.event}</span>
            <span className="text-muted-foreground">{e.target}</span>
            <span className="min-w-0 basis-full break-words sm:basis-auto">{summary(e)}</span>
          </motion.li>
        ))}
      </AnimatePresence>
    </ol>
  )
}
