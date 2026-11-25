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
        'inline-flex min-h-8 items-center gap-1.5 rounded-full border px-3 py-1 text-[11.5px] font-bold whitespace-nowrap',
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

/**
 * Barra compacta: compuerta, kill switch, pérdida del día, apalancamiento y reconciliación.
 *
 * Fluye en bloques que bajan de línea enteros (#356): los chips juntos, la pérdida del día con su
 * barra (se estira, nunca baja de ~15rem), el apalancamiento sin cortar, y la reconciliación en
 * su propia franja abajo. Así ningún texto se encima con otro en ningún ancho.
 */
export function OpsStatusBar({ status }: { status: OpsStatus | null }) {
  if (!status) {
    return (
      <section
        aria-label="Estado de la operación"
        data-ops-status="offline"
        className="glass flex flex-wrap items-center gap-x-3 gap-y-2 rounded-[20px] px-4 py-3 text-[12px] text-muted-foreground"
      >
        <Chip className="border-border">Sin conexión con el gateway</Chip>
        <span className="min-w-0">
          No se sabe el estado del broker: no se muestra uno de ejemplo.
        </span>
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
      className="glass relative grid gap-3 rounded-[20px] px-4 py-3 sm:px-5"
    >
      <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
        <div className="flex flex-wrap items-center gap-2">
          <Chip className={gate.tone}>
            <span className="size-2 shrink-0 rounded-full bg-current" aria-hidden="true" />
            {gate.label}
          </Chip>
          {a && (
            <Chip
              className={a.kill_switch.active ? 'border-down bg-down text-white' : 'border-border'}
            >
              <Term id="kill-switch">Kill switch</Term>{' '}
              {a.kill_switch.active ? 'ACTIVO' : 'apagado'}
            </Chip>
          )}
        </div>
        {a ? (
          <>
            <div className="grid min-w-[min(100%,15rem)] flex-[1_1_15rem] gap-1.5 text-[11.5px]">
              <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
                <span className="font-bold">Pérdida del día</span>
                <span className="text-muted-foreground tabular-nums">
                  <strong
                    className={cn(
                      'font-bold',
                      a.kill_switch.active ? 'text-down' : 'text-foreground',
                    )}
                  >
                    {decimal(a.daily_loss_used_pct)} % del máximo
                  </strong>{' '}
                  ({decimal(a.daily_loss_limit_pct)} % del capital)
                </span>
              </div>
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
                    'block h-full rounded-full transition-[width] duration-500 motion-reduce:transition-none',
                    a.kill_switch.active ? 'bg-down' : 'bg-foreground/60',
                  )}
                  style={{ width: `min(${a.daily_loss_used_pct}%, 100%)` }}
                />
              </span>
            </div>
            <p className="text-[11.5px] text-muted-foreground tabular-nums sm:whitespace-nowrap">
              <Term id="apalancamiento">Apalancamiento</Term>{' '}
              <strong className="font-bold text-foreground">{decimal(a.leverage)}:1</strong> ·{' '}
              {a.open_positions} de {a.max_positions} posiciones
            </p>
          </>
        ) : (
          <Chip className="border-down/40 text-down">Sin datos de la cuenta</Chip>
        )}
      </div>
      {rec && (
        <p
          className={cn(
            'border-t border-border pt-2.5 text-[11.5px] leading-snug',
            rec.clean ? 'text-muted-foreground' : 'font-semibold text-down',
          )}
          data-reconcile={rec.clean ? 'clean' : 'dirty'}
        >
          Reconciliación {rec.kind === 'startup' ? 'al arrancar' : 'periódica'}:{' '}
          {rec.clean
            ? `limpia (${rec.matched} coinciden)`
            : `${rec.mismatches} posiciones distintas y ${rec.drifts} órdenes que no se reconstruyen igual`}{' '}
          <span className="whitespace-nowrap tabular-nums">· {dateTimeUtc(rec.at)}</span>
        </p>
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
  instrument_skipped: 'Instrumento omitido',
  connection_error: 'Error de conexión',
}

/**
 * Marcas de una nota técnica para desarrolladores (#580): pedidos a la API, ADR, secciones de la
 * spec, números de Issue, rutas de docs o contratos. Ese texto va plegado en "Detalle técnico".
 */
const TECHNICAL = /GET \/|POST \/|\bADR\b|§|#\d+|docs\/|\.md\b|\.toml\b|\w+\.v\d\b|\{\w+\}/

/**
 * Lo que se muestra de un evento: un texto para el usuario y, si el motor mandó una nota técnica,
 * el texto original aparte. `instrument_skipped` llega como "XAUUSD: <motivo técnico>".
 */
function describe(e: OpsEvent): { text: string; technical: string | null } {
  const raw = summary(e)
  if (e.event === 'instrument_skipped') {
    const id = /^([\w.]+):/.exec(raw)?.[1]
    return {
      text: id
        ? `Se omite ${id} en el broker simulado: faltan datos del instrumento.`
        : 'Se omite un instrumento en el broker simulado: faltan datos del instrumento.',
      technical: raw,
    }
  }
  if (!TECHNICAL.test(raw)) return { text: raw, technical: null }
  // se quedan las frases sin marcas técnicas; el texto entero, plegado
  const plain = raw
    .split(/(?<=[.;])\s+/)
    .filter((part) => !TECHNICAL.test(part))
    .join(' ')
    .trim()
  return { text: plain || 'Aviso del motor con detalle técnico.', technical: raw }
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

/**
 * Los últimos eventos del broker y alertas, el más nuevo primero. Los repetidos llegan agrupados
 * del motor (#576): se muestra la última vez, cuántas veces y desde cuándo, tal cual.
 */
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
            <span className="text-muted-foreground tabular-nums">
              {dateTimeUtc(e.last_seen ?? e.ts)}
            </span>
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
            {(e.count ?? 1) > 1 && (
              <span
                className="rounded-full border border-border px-2 text-[10px] font-bold tabular-nums"
                title={`Primera vez: ${dateTimeUtc(e.ts)}`}
              >
                ×{e.count} · desde {dateTimeUtc(e.ts)}
              </span>
            )}
            <EventText event={e} />
          </motion.li>
        ))}
      </AnimatePresence>
    </ol>
  )
}

/** El texto de un evento, con el detalle técnico plegado si lo hay (#580). */
function EventText({ event }: { event: OpsEvent }) {
  const { text, technical } = describe(event)
  if (!technical)
    return <span className="min-w-0 basis-full break-words sm:basis-auto">{text}</span>
  return (
    <span className="min-w-0 basis-full break-words sm:basis-auto">
      {text}{' '}
      <details className="inline text-muted-foreground [&[open]]:block">
        <summary className="inline cursor-pointer text-[11px] underline decoration-dotted">
          Detalle técnico
        </summary>
        <span className="mt-1 block text-[11px] [overflow-wrap:anywhere]">{technical}</span>
      </details>
    </span>
  )
}
