/**
 * Diario (#581, etapa 8 de docs/design/2026-11-rediseno-ui.md): un libro de registro en línea de
 * tiempo, con el lenguaje editorial de Investigación (papel, fechas en serif, acento violeta). Cada
 * operación simulada es una entrada con su hora UTC, el instrumento, el motivo del motor, el
 * resultado que calculó el motor (ya con costos) y la nota del miembro.
 *
 * Honestidad de los datos (#577): el diario todavía no lee las operaciones del broker simulado ni
 * las de las corridas, así que son de ejemplo y llevan el sello "EJEMPLO". La UI no calcula nada:
 * agrupar por día y mostrar en el orden en que llegan es presentación.
 */
import { ArrowDownRight, ArrowUpRight } from 'lucide-react'

import { ExampleNotice } from '@/components/ui/data-honesty'
import { dateUtc, decimal, isNegative, signed } from '@/lib/format'
import { cn } from '@/lib/utils'
import { TRADES, type TradeRow } from '@/mocks'

const TIME = new Intl.DateTimeFormat('es-ES', {
  hour: '2-digit',
  minute: '2-digit',
  timeZone: 'UTC',
})

/** Las operaciones agrupadas por día UTC, en el orden en que llegan. */
function byDay(trades: TradeRow[]) {
  const days: { day: string; trades: TradeRow[] }[] = []
  for (const t of trades) {
    const day = t.at.slice(0, 10)
    const last = days[days.length - 1]
    if (last?.day === day) last.trades.push(t)
    else days.push({ day, trades: [t] })
  }
  return days
}

function Entry({ t }: { t: TradeRow }) {
  const Arrow = t.side === 'Largo' ? ArrowUpRight : ArrowDownRight
  return (
    <li className="relative grid gap-2 pb-7 pl-6 last:pb-0">
      {/* el punto sobre la línea del tiempo */}
      <span
        aria-hidden="true"
        className="absolute top-1.5 -left-[5px] size-2.5 rounded-full border-2 border-section bg-accent"
      />
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <time dateTime={t.at} className="font-mono text-[12px] text-muted-foreground">
          {TIME.format(new Date(t.at))} UTC
        </time>
        <h3 className="flex items-center gap-1.5 text-[15px] font-bold">
          {t.instrument}
          <span className="inline-flex items-center gap-0.5 text-[12.5px] font-semibold text-muted-foreground">
            <Arrow aria-hidden="true" className="size-3.5" />
            {t.side.toLowerCase()}
          </span>
        </h3>
        <span className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
          {t.mode}
        </span>
      </div>
      <dl className="flex flex-wrap gap-x-5 gap-y-1 text-[12.5px]">
        <div className="flex gap-1.5">
          <dt className="text-muted-foreground">Tamaño</dt>
          <dd className="font-mono">{decimal(t.size)}</dd>
        </div>
        <div className="flex gap-1.5">
          <dt className="text-muted-foreground">Entrada</dt>
          <dd className="font-mono">{decimal(t.entry)}</dd>
        </div>
        <div className="flex gap-1.5">
          <dt className="text-muted-foreground">Resultado del motor</dt>
          <dd
            className={cn(
              'font-mono font-bold',
              t.netPnl && (isNegative(t.netPnl) ? 'text-down' : 'text-up'),
            )}
          >
            {t.netPnl === null ? 'abierta' : `${signed(t.netPnl)} € · ${t.status.toLowerCase()}`}
          </dd>
        </div>
      </dl>
      {t.reason && (
        <p className="max-w-[68ch] text-[13.5px] leading-relaxed">
          <span className="font-semibold text-accent">Motivo · </span>
          {t.reason}
        </p>
      )}
      {t.note ? (
        <blockquote className="max-w-[68ch] border-l-2 border-accent/50 pl-3 font-serif text-[16px] leading-[1.55] italic">
          {t.note}
        </blockquote>
      ) : (
        <p className="text-[12px] text-muted-foreground">Sin nota todavía.</p>
      )}
    </li>
  )
}

export function JournalView() {
  return (
    <div
      data-section="journal"
      className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-6 rounded-section bg-section p-4 text-foreground sm:p-8"
    >
      <ExampleNotice>
        Operaciones de ejemplo: el diario todavía no lee las del broker simulado ni las de las
        corridas (están en Performance).
      </ExampleNotice>
      <header className="grid max-w-[68ch] gap-2">
        <h2 className="font-serif text-[clamp(26px,3vw,36px)] leading-tight font-semibold">
          Libro de operaciones
        </h2>
        <p className="text-[13px] text-muted-foreground">
          Cada operación en simulación, con el motivo de la estrategia, el resultado que calculó el
          motor (ya con costos) y la nota de quien la siguió. Resultados del pasado, no promesas.
        </p>
      </header>
      <section aria-label="Diario de operaciones" className="grid gap-8">
        {byDay(TRADES).map(({ day, trades }) => (
          <div
            key={day}
            className="grid gap-3 border-t border-border pt-5 sm:grid-cols-[8rem_minmax(0,1fr)] sm:gap-6"
          >
            <h2 className="font-serif text-[22px] leading-none font-semibold sm:sticky sm:top-24 sm:self-start">
              <time dateTime={day}>{dateUtc(`${day}T00:00:00Z`)}</time>
            </h2>
            <ol className="ml-1 border-l border-accent/40">
              {trades.map((t) => (
                <Entry key={t.id} t={t} />
              ))}
            </ol>
          </div>
        ))}
      </section>
    </div>
  )
}
