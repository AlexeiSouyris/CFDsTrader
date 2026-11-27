/**
 * Diario (#581, etapa 8 de docs/design/2026-11-rediseno-ui.md): un libro de registro en línea de
 * tiempo, con el lenguaje editorial de Investigación (papel, fechas en serif, acento violeta). Cada
 * operación del runner de paper es una entrada con la hora UTC de la señal, el instrumento, los
 * gatillos del motor y el resultado que calculó el motor (ya con costos).
 *
 * Datos reales (#616): lee `GET /api/v1/journal` (`gateway_journal.v1`), el diario que guarda el
 * runner de paper. Sin operaciones, un estado vacío; sin gateway, el aviso del error. Nunca filas
 * de ejemplo. La UI no calcula nada: agrupar por día y mostrar en el orden en que llegan es
 * presentación.
 */
import { ArrowDownRight, ArrowUpRight } from 'lucide-react'

import { InstrumentName } from '@/components/ui/instrument-name'
import type { ExitReason } from '@/lib/backtest-contracts'
import { dateUtc, decimal, isNegative, signed } from '@/lib/format'
import { useJournal, type JournalEntry } from '@/lib/journal-api'
import { useInstrumentNames, type InstrumentNames } from '@/lib/liquidity-api'
import { cn } from '@/lib/utils'
import type { Proposal } from '@/mocks'

const TIME = new Intl.DateTimeFormat('es-ES', {
  hour: '2-digit',
  minute: '2-digit',
  timeZone: 'UTC',
})

const TRIGGER_LABEL: Record<Proposal['triggers'][number], string> = {
  candle_pattern: 'patrón de velas',
  double_break: 'ruptura de doble techo o piso',
  level_rejection: 'rechazo de un nivel de liquidez',
  sweep: 'sweep de liquidez',
}

const EXIT_LABEL: Record<ExitReason, string> = {
  stop: 'salió por el stop',
  tp1: 'salió en TP1',
  break_even: 'salió en break-even',
  trailing: 'salió por el trailing',
  time: 'salió por tiempo',
  forced: 'cierre forzado',
  end_of_data: 'cerrada al final de los datos',
}

/** Las entradas agrupadas por día UTC de la señal, en el orden en que llegan. */
function byDay(entries: JournalEntry[]) {
  const days: { day: string; entries: JournalEntry[] }[] = []
  for (const e of entries) {
    const day = e.proposal.signal_at.slice(0, 10)
    const last = days[days.length - 1]
    if (last?.day === day) last.entries.push(e)
    else days.push({ day, entries: [e] })
  }
  return days
}

function Entry({
  entry: { proposal: p, outcome: o },
  names,
}: {
  entry: JournalEntry
  names: InstrumentNames
}) {
  const long = p.side === 'long'
  const Arrow = long ? ArrowUpRight : ArrowDownRight
  return (
    <li className="relative grid gap-2 pb-7 pl-6 last:pb-0">
      {/* el punto sobre la línea del tiempo */}
      <span
        aria-hidden="true"
        className="absolute top-1.5 -left-[5px] size-2.5 rounded-full border-2 border-section bg-accent"
      />
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <time dateTime={p.signal_at} className="font-mono text-[12px] text-muted-foreground">
          {TIME.format(new Date(p.signal_at))} UTC
        </time>
        <h3 className="flex items-center gap-1.5 text-[15px] font-bold">
          <InstrumentName id={p.instrument_id} names={names} />
          <span className="inline-flex items-center gap-0.5 text-[12.5px] font-semibold text-muted-foreground">
            <Arrow aria-hidden="true" className="size-3.5" />
            {long ? 'largo' : 'corto'}
          </span>
        </h3>
        <span className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
          simulación
        </span>
      </div>
      <dl className="flex flex-wrap gap-x-5 gap-y-1 text-[12.5px]">
        <div className="flex gap-1.5">
          <dt className="text-muted-foreground">Tamaño</dt>
          <dd className="font-mono">{decimal(p.size)}</dd>
        </div>
        <div className="flex gap-1.5">
          <dt className="text-muted-foreground">Entrada</dt>
          <dd className="font-mono">{decimal(o ? o.entry_price : p.reference)}</dd>
        </div>
        <div className="flex gap-1.5">
          <dt className="text-muted-foreground">Stop</dt>
          <dd className="font-mono">{decimal(p.stop)}</dd>
        </div>
        <div className="flex gap-1.5">
          <dt className="text-muted-foreground">Resultado del motor</dt>
          <dd
            className={cn(
              'font-mono',
              o ? cn('font-bold', isNegative(o.net_pnl) ? 'text-down' : 'text-up') : '',
            )}
          >
            {o ? `${signed(o.net_pnl)} · ${EXIT_LABEL[o.exit_reason]}` : 'todavía sin resultado'}
          </dd>
        </div>
      </dl>
      <p className="max-w-[68ch] text-[13.5px] leading-relaxed">
        <span className="font-semibold text-accent">Motivo · </span>
        {p.triggers.map((t) => TRIGGER_LABEL[t]).join(', ')} (puntaje {p.score})
      </p>
    </li>
  )
}

function Quiet({ children, role }: { children: string; role: 'status' | 'alert' }) {
  return (
    <p
      role={role}
      className="max-w-[68ch] rounded-xl border border-dashed border-border px-4 py-6 text-[14px] leading-relaxed text-muted-foreground"
    >
      {children}
    </p>
  )
}

export function JournalView() {
  const journal = useJournal()
  const names = useInstrumentNames()
  return (
    <div
      data-section="journal"
      className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-6 rounded-section bg-section p-4 text-foreground sm:p-8"
    >
      <header className="grid max-w-[68ch] gap-2">
        <h2 className="font-serif text-[clamp(26px,3vw,36px)] leading-tight font-semibold">
          Libro de operaciones
        </h2>
        <p className="text-[13px] text-muted-foreground">
          Cada operación del runner de simulación, con los gatillos de la estrategia y el resultado
          que calculó el motor (ya con costos, en la moneda del instrumento). Resultados del pasado,
          no promesas.
        </p>
      </header>
      <section aria-label="Diario de operaciones" className="grid gap-8">
        {journal.status === 'loading' ? (
          <Quiet role="status">Pidiendo el diario al motor…</Quiet>
        ) : journal.status === 'error' ? (
          <Quiet role="alert">
            {`No se pudo leer el diario del motor (${journal.message}). No se muestra nada inventado: probá de nuevo en un rato.`}
          </Quiet>
        ) : journal.data.entries.length === 0 ? (
          <Quiet role="status">
            Todavía no hay operaciones en simulación: el runner arranca el 27-11. Cuando proponga y
            cierre operaciones, van a aparecer acá con su resultado.
          </Quiet>
        ) : (
          byDay(journal.data.entries).map(({ day, entries }) => (
            <div
              key={day}
              className="grid gap-3 border-t border-border pt-5 sm:grid-cols-[8rem_minmax(0,1fr)] sm:gap-6"
            >
              <h2 className="font-serif text-[22px] leading-none font-semibold sm:sticky sm:top-24 sm:self-start">
                <time dateTime={day}>{dateUtc(`${day}T00:00:00Z`)}</time>
              </h2>
              <ol className="ml-1 border-l border-accent/40">
                {entries.map((e) => (
                  <Entry key={e.proposal.id} entry={e} names={names} />
                ))}
              </ol>
            </div>
          ))
        )}
        {journal.status === 'ok' && journal.data.entries.length === journal.data.limit && (
          <p className="text-[12px] text-muted-foreground">
            Se muestran las últimas {journal.data.limit} operaciones.
          </p>
        )}
      </section>
    </div>
  )
}
