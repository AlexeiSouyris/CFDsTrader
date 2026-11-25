/**
 * Pestaña "Noticias" de la Terminal (#275): las últimas noticias y lo que viene en el calendario
 * económico, tal como los sirve el gateway (`gateway_news.v1`, `gateway_calendar.v1`, #600). Solo
 * se piden con la pestaña abierta. Sin gateway no hay ejemplo: se dice. Sin cálculos: ordena por
 * hora lo que llega y nada más (CLAUDE.md §1). Cada noticia muestra su hora de publicación real.
 */
import { dateTimeUtc } from '@/lib/format'
import { useCalendar, useNews } from '@/lib/news-api'
import type { EconomicEvent } from '@/lib/news-contracts'
import { cn } from '@/lib/utils'

const MAX_ROWS = 6
const IMPACT: Record<EconomicEvent['impact'], { label: string; tone: string }> = {
  high: { label: 'alto', tone: 'border-down/60 text-down' },
  medium: { label: 'medio', tone: 'border-[var(--chart-signal)]/60 text-[var(--chart-signal)]' },
  low: { label: 'bajo', tone: 'border-border text-muted-foreground' },
}

function Note({ children }: { children: string }) {
  return (
    <p role="status" className="py-3 text-[12.5px] text-muted-foreground">
      {children}
    </p>
  )
}

export function TerminalNews() {
  const news = useNews()
  const calendar = useCalendar()
  // lo que todavía no pasó a la hora del gateway (`as_of`), de lo más cercano a lo más lejano
  const upcoming =
    calendar.status === 'ok'
      ? calendar.data.economic_events
          .filter((e) => Date.parse(e.scheduled_at) >= Date.parse(calendar.data.as_of))
          .sort((a, b) => Date.parse(a.scheduled_at) - Date.parse(b.scheduled_at))
          .slice(0, MAX_ROWS)
      : []
  return (
    <div className="grid gap-4 lg:grid-cols-2" data-terminal-news>
      <section aria-labelledby="terminal-news-title" className="min-w-0">
        <h3
          id="terminal-news-title"
          className="mb-1 text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase"
        >
          Últimas noticias
        </h3>
        {news.status === 'ok' ? (
          news.data.items.length === 0 ? (
            <Note>El gateway todavía no tiene noticias guardadas.</Note>
          ) : (
            <ul className="divide-y divide-border">
              {news.data.items.slice(0, MAX_ROWS).map(({ item }) => (
                <li key={`${item.source}|${item.item_id}`} className="grid gap-0.5 py-2">
                  <span className="text-[11px] text-muted-foreground tabular-nums">
                    {dateTimeUtc(item.published_at)} ·{' '}
                    <span className="font-bold uppercase">{item.source}</span>
                  </span>
                  <a
                    href={item.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-[13px] leading-snug font-semibold hover:underline focus-visible:outline-2 focus-visible:outline-ring"
                  >
                    {item.title}
                    <span className="sr-only"> (abre en otra pestaña)</span>
                  </a>
                </li>
              ))}
            </ul>
          )
        ) : (
          <Note>
            {news.status === 'loading'
              ? 'Pidiendo las noticias al gateway…'
              : 'Sin conexión con el gateway: no hay noticias para mostrar.'}
          </Note>
        )}
      </section>
      <section aria-labelledby="terminal-agenda-title" className="min-w-0">
        <h3
          id="terminal-agenda-title"
          className="mb-1 text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase"
        >
          Próximos en el calendario
        </h3>
        {calendar.status === 'ok' ? (
          upcoming.length === 0 ? (
            <Note>No hay eventos próximos en el calendario guardado.</Note>
          ) : (
            <ul className="divide-y divide-border">
              {upcoming.map((e) => (
                <li
                  key={`${e.source}|${e.event}|${e.scheduled_at}`}
                  className="grid grid-cols-[auto_minmax(0,1fr)] items-baseline gap-x-2 py-2"
                >
                  <span
                    className={cn(
                      'min-w-12 rounded-full border px-1.5 text-center text-[10.5px] font-bold',
                      IMPACT[e.impact].tone,
                    )}
                  >
                    <span className="sr-only">impacto </span>
                    {IMPACT[e.impact].label}
                  </span>
                  <span className="min-w-0">
                    <span className="block truncate text-[13px] font-semibold">{e.event}</span>
                    <span className="text-[11px] text-muted-foreground tabular-nums">
                      {dateTimeUtc(e.scheduled_at)}
                      {e.country && ` · ${e.country}`}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          )
        ) : (
          <Note>
            {calendar.status === 'loading'
              ? 'Pidiendo el calendario al gateway…'
              : 'Sin conexión con el gateway: no hay calendario para mostrar.'}
          </Note>
        )}
      </section>
    </div>
  )
}
