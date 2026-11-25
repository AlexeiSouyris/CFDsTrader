/**
 * Noticias y Contexto de mercado (#581, etapa 7 de docs/design/2026-11-rediseno-ui.md): un lector.
 * A la izquierda la lista (fuente, titular y hora de publicación real en UTC), al centro el
 * artículo con tipografía de lectura y a la derecha la columna "Contexto": el índice de sentimiento
 * y los análogos tal como los calculó Python y los sirve el gateway, y el calendario.
 *
 * Honestidad de los datos (#577, #600): las noticias y el calendario salen del gateway
 * (`/api/v1/news` y `/api/v1/calendar`, lo que guardó Python). Si el gateway no responde, se
 * muestran los de ejemplo (`mocks/`) con el sello "EJEMPLO". Las carpetas de cada miembro todavía
 * no tienen datos reales: son de ejemplo y solo filtran las noticias de ejemplo. La interpretación
 * automática se muestra tal cual la guardó `intelligence`, como estimación. La UI no calcula nada
 * (CLAUDE.md §1).
 */
import { useRef, useState, type ReactNode } from 'react'

import { ExampleBadge, ExampleNotice } from '@/components/ui/data-honesty'
import { AnalogDistribution, SentimentGauge } from '@/components/ui/market-context'
import { Segmented } from '@/components/ui/segmented'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, dateUtc } from '@/lib/format'
import { useCalendar, useNews } from '@/lib/news-api'
import type {
  EconomicEvent,
  GatewayCalendar,
  NewsInterpretation,
  OfficialRelease,
} from '@/lib/news-contracts'
import { useMarketContext } from '@/lib/use-market-context'
import { cn } from '@/lib/utils'
import { CALENDAR_EXAMPLE, CURRENT_MEMBER, FOLDERS, NEWS_EXAMPLE } from '@/mocks'

const SOURCE: Record<string, string> = { fed: 'Fed', ecb: 'BCE' }
const sourceName = (s: string) => SOURCE[s] ?? s.toUpperCase()
const ALL = 'todas'

/** Una noticia de la lista: del motor (`news_item.v1` con sus interpretaciones) o de ejemplo. */
type Story = {
  key: string
  source: string
  feed: string
  title: string
  published_at: string
  retrieved_at: string
  /** enlace a la fuente (solo las reales) */
  url?: string
  /** texto de relleno (solo las de ejemplo: el texto real no se guarda) */
  body?: string[]
  interpretations: NewsInterpretation[]
  folders: string[]
}

const EXAMPLE_STORIES: Story[] = NEWS_EXAMPLE.map((n) => ({
  key: n.item_id,
  source: n.source,
  feed: n.feed,
  title: n.title,
  published_at: n.published_at,
  retrieved_at: n.retrieved_at,
  body: n.body,
  interpretations: [],
  folders: n.folders,
}))

const IMPACT: Record<EconomicEvent['impact'], string> = {
  low: 'bajo',
  medium: 'medio',
  high: 'alto',
}

const SERIES: Record<OfficialRelease['series'], string> = {
  fomc_statement: 'Comunicado del FOMC',
  cpi: 'IPC de EE. UU. (BLS)',
  employment_situation: 'Informe de empleo de EE. UU. (BLS)',
}

const STATUS: Record<OfficialRelease['status'], string> = {
  scheduled: 'programada',
  rescheduled: 'reprogramada',
  cancelled: 'cancelada',
  unscheduled: 'no programada',
}

/** Una fila del calendario, tal como la mandó el gateway. */
type CalendarRow = { key: string; at: string; title: string; detail: ReactNode; known: string }

/** Junta las dos tablas del calendario en una sola lista por hora (solo presentación). */
function calendarRows(c: GatewayCalendar): CalendarRow[] {
  const events = c.economic_events.map((e, i) => ({
    key: `e${i}`,
    at: e.scheduled_at,
    title: e.event,
    detail: (
      <>
        {[e.country, e.instrument_id].filter(Boolean).join(' · ')} · impacto {IMPACT[e.impact]}
      </>
    ),
    known: e.published_at,
  }))
  const releases = c.official_releases.flatMap((r, i) =>
    r.scheduled_at === null
      ? []
      : [
          {
            key: `r${i}`,
            at: r.scheduled_at,
            title: SERIES[r.series],
            detail: (
              <>
                {r.reference_period} · {STATUS[r.status]}
                {r.with_press_conference && ' · con conferencia de prensa'} ·{' '}
                <a
                  href={r.source_url}
                  target="_blank"
                  rel="noreferrer"
                  className="underline underline-offset-2 hover:text-foreground"
                >
                  fuente
                </a>
              </>
            ),
            known: r.published_at,
          },
        ],
  )
  return [...events, ...releases].sort((a, b) => Date.parse(a.at) - Date.parse(b.at))
}

/** Una hora UTC legible, con la hora exacta en `dateTime` para lectores de pantalla y copiado. */
function When({ at, className }: { at: string; className?: string }) {
  return (
    <time dateTime={at} className={cn('tabular-nums', className)}>
      {dateTimeUtc(at)}
    </time>
  )
}

const quiet = (text: string, role?: 'status' | 'alert') => (
  <p role={role} className="text-[12.5px] text-muted-foreground">
    {text}
  </p>
)

function ContextBlock({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <section className="grid min-w-0 gap-3 border-t border-border pt-4 first-of-type:border-t-0 first-of-type:pt-0">
      <h3 className="flex items-center justify-between gap-2 text-[11px] font-bold tracking-[0.1em] text-accent uppercase">
        {title}
      </h3>
      {children}
    </section>
  )
}

function CalendarList({ rows }: { rows: CalendarRow[] }) {
  return (
    <ol className="grid max-h-[32rem] gap-2.5 overflow-y-auto pr-1 [scrollbar-width:thin]">
      {rows.map((e) => (
        <li key={e.key} className="grid gap-0.5">
          <When at={e.at} className="text-[11.5px] text-muted-foreground" />
          <span className="text-[13.5px] font-semibold">{e.title}</span>
          <span className="text-[11.5px] text-muted-foreground">
            {e.detail} · en el calendario desde el{' '}
            <time dateTime={e.known}>{dateUtc(e.known)}</time>
          </span>
        </li>
      ))}
    </ol>
  )
}

/** El calendario del motor; si el gateway no responde, el de ejemplo con su sello. */
function CalendarBlock() {
  const calendar = useCalendar()
  if (calendar.status !== 'ok') {
    return (
      <ContextBlock
        title={
          <>
            Calendario <ExampleBadge />
          </>
        }
      >
        <CalendarList
          rows={CALENDAR_EXAMPLE.map((e) => ({
            key: e.event,
            at: e.scheduled_at,
            title: e.event,
            detail: (
              <>
                {e.country} · impacto {e.impact}
              </>
            ),
            known: e.published_at,
          }))}
        />
      </ContextBlock>
    )
  }
  const rows = calendarRows(calendar.data)
  return (
    <ContextBlock title="Calendario">
      {rows.length === 0 ? (
        quiet(
          `Sin eventos guardados entre el ${dateUtc(calendar.data.from)} y el ${dateUtc(calendar.data.to)}.`,
        )
      ) : (
        <CalendarList rows={rows} />
      )}
    </ContextBlock>
  )
}

/** La interpretación automática, tal cual la guardó `intelligence`: una estimación, no un hecho. */
function Interpretation({ it }: { it: NewsInterpretation }) {
  return (
    <section className="grid gap-3 rounded-xl border border-border p-4">
      <h3 className="text-[11px] font-bold tracking-[0.1em] text-accent uppercase">
        Interpretación automática
      </h3>
      <p className="text-[15px] leading-[1.55]">{it.summary_es}</p>
      {it.impacts.length > 0 && (
        <div className="min-w-0 overflow-x-auto [scrollbar-width:thin]">
          <table className="w-full text-left text-[12.5px] tabular-nums">
            <caption className="sr-only">Estimación por instrumento</caption>
            <thead className="text-muted-foreground">
              <tr>
                <th scope="col" className="py-1 pr-4 font-semibold">
                  Instrumento
                </th>
                <th scope="col" className="py-1 pr-4 font-semibold">
                  Sentimiento estimado (−1 a 1)
                </th>
                <th scope="col" className="py-1 font-semibold">
                  Relevancia estimada (0 a 1)
                </th>
              </tr>
            </thead>
            <tbody>
              {it.impacts.map((i) => (
                <tr key={i.instrument_id} className="border-t border-border">
                  <th scope="row" className="py-1 pr-4 font-semibold">
                    {i.instrument_id}
                  </th>
                  <td className="py-1 pr-4">{i.sentiment}</td>
                  <td className="py-1">{i.relevance}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="text-[11.5px] text-muted-foreground">
        Estimación de un modelo automático ({it.interpreter.name} {it.interpreter.version}), hecha
        el <When at={it.interpreted_at} />. Puede equivocarse: no es un pronóstico ni una
        recomendación.
      </p>
    </section>
  )
}

/** La columna "Contexto": lo que mandó el motor de Python y el calendario. */
function ContextColumn() {
  const { latest, analogs, ids, selected, setPicked, index, report } = useMarketContext()
  return (
    <aside
      aria-labelledby="noticias-contexto"
      className="grid min-w-0 content-start gap-4 border-t border-border pt-6 lg:col-span-2 xl:col-span-1 xl:border-t-0 xl:border-l xl:pt-0 xl:pl-6"
    >
      <div className="grid gap-2">
        <h2 id="noticias-contexto" className="font-serif text-[22px] font-semibold">
          Contexto
        </h2>
        <p className="text-[12px] text-muted-foreground">
          Solo informativo: no es un pronóstico ni una recomendación.
        </p>
        {ids.length > 1 && selected && (
          <div className="max-w-full min-w-0 overflow-x-auto [scrollbar-width:thin]">
            <Segmented label="Índice" options={ids} value={selected} onChange={setPicked} />
          </div>
        )}
      </div>
      <div className="grid min-w-0 gap-4">
        <ContextBlock title={<Term id="indice-sentimiento">Sentimiento</Term>}>
          {latest.status === 'loading' ? (
            quiet('Pidiendo los datos al motor…', 'status')
          ) : index ? (
            <SentimentGauge index={index} />
          ) : latest.status === 'error' ? (
            quiet('No se pudo leer el índice del motor. Se vuelve a intentar cada minuto.', 'alert')
          ) : (
            quiet('Sin índice guardado para este instrumento.')
          )}
        </ContextBlock>
        <ContextBlock title={<Term id="analogos">Análogos históricos</Term>}>
          {analogs.status === 'loading' ? (
            quiet('Pidiendo los datos al motor…', 'status')
          ) : report ? (
            <div className="min-w-0 overflow-x-auto [scrollbar-width:thin]">
              <AnalogDistribution report={report} />
            </div>
          ) : analogs.status === 'error' ? (
            quiet('No se pudieron leer los análogos. Se vuelve a intentar cada minuto.', 'alert')
          ) : (
            quiet('Sin análogos guardados para este instrumento.')
          )}
        </ContextBlock>
        <CalendarBlock />
      </div>
    </aside>
  )
}

export function NewsView() {
  const news = useNews()
  // Del motor si el gateway respondió; si no, las de ejemplo con su sello.
  const real = news.status === 'ok'
  const stories: Story[] = real
    ? news.data.items.map(({ item, interpretations }) => ({
        key: `${item.source}:${item.item_id}`,
        source: item.source,
        feed: item.feed,
        title: item.title,
        published_at: item.published_at,
        retrieved_at: item.retrieved_at,
        url: item.url,
        interpretations,
        folders: [],
      }))
    : EXAMPLE_STORIES
  const [folder, setFolder] = useState(ALL)
  const items = real || folder === ALL ? stories : stories.filter((n) => n.folders.includes(folder))
  const [openId, setOpenId] = useState<string | undefined>(undefined)
  const open = items.find((n) => n.key === openId) ?? items[0]
  const articleRef = useRef<HTMLElement>(null)

  const read = (id: string) => {
    setOpenId(id)
    // en el celular el artículo queda debajo de la lista: se baja hasta él
    if (window.matchMedia?.('(max-width: 1023px)').matches)
      articleRef.current?.scrollIntoView?.({ block: 'start' })
  }

  const chips = [
    { id: ALL, label: 'Todas' },
    ...FOLDERS.map((f) => ({
      id: f.owner.id,
      label: f.owner.id === CURRENT_MEMBER.id ? 'Mi carpeta' : `Carpeta de ${f.owner.name}`,
    })),
  ]

  return (
    <div
      data-section="news"
      className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-5 rounded-section bg-section p-3 text-foreground sm:p-6"
    >
      {real ? (
        <p className="text-[12px] text-muted-foreground">
          Noticias de las fuentes aprobadas, tal como las guardó el motor. Las carpetas de cada
          miembro todavía no tienen datos reales.
        </p>
      ) : (
        <ExampleNotice>
          {news.status === 'loading'
            ? 'Pidiendo las noticias al motor… Mientras tanto, las de ejemplo.'
            : 'No se pudieron leer las noticias del motor: las noticias, las carpetas y el calendario que se ven son de ejemplo. Se vuelve a intentar cada minuto.'}
        </ExampleNotice>
      )}

      {!real && (
        <div className="grid gap-2">
          <div
            role="group"
            aria-label="Carpetas"
            className="-m-1.5 flex gap-2 overflow-x-auto p-1.5 [scrollbar-width:none] sm:flex-wrap"
          >
            {chips.map((c) => (
              <button
                key={c.id}
                type="button"
                aria-pressed={folder === c.id}
                onClick={() => setFolder(c.id)}
                className={cn(
                  'min-h-9 shrink-0 rounded-full border px-3.5 whitespace-nowrap text-[12.5px] font-semibold transition-colors outline-accent focus-visible:outline-2 focus-visible:outline-offset-3',
                  folder === c.id
                    ? 'border-accent bg-accent/15 text-foreground'
                    : 'border-border text-muted-foreground hover:text-foreground',
                )}
              >
                {c.label}
              </button>
            ))}
          </div>
          <p className="text-[11.5px] text-muted-foreground">
            Cada miembro guarda en su carpeta las noticias que le importan. Todos ven todas; cada
            uno modifica solo la suya.
          </p>
        </div>
      )}

      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-6 lg:grid-cols-[16rem_minmax(0,1fr)] xl:grid-cols-[16rem_minmax(0,1fr)_21rem]">
        <nav aria-label="Noticias" className="min-w-0">
          {items.length === 0 ? (
            quiet(real ? 'Todavía no hay noticias guardadas.' : 'Esta carpeta está vacía.')
          ) : (
            <ol className="grid border-t border-border">
              {items.map((n) => {
                const active = n.key === open?.key
                return (
                  <li key={n.key} className="border-b border-border">
                    <button
                      type="button"
                      aria-current={active ? 'true' : undefined}
                      onClick={() => read(n.key)}
                      className={cn(
                        'grid w-full gap-1 border-l-2 py-3 pr-2 pl-3 text-left transition-colors outline-accent focus-visible:outline-2 focus-visible:-outline-offset-2',
                        active
                          ? 'border-accent bg-accent/10'
                          : 'border-transparent hover:bg-foreground/5',
                      )}
                    >
                      <span className="text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
                        {sourceName(n.source)} · {n.feed}
                      </span>
                      <span className="font-serif text-[15.5px] leading-snug font-semibold">
                        {n.title}
                      </span>
                      <When at={n.published_at} className="text-[11.5px] text-muted-foreground" />
                    </button>
                  </li>
                )
              })}
            </ol>
          )}
        </nav>

        <article
          ref={articleRef}
          aria-labelledby="noticias-titulo"
          className="min-w-0 scroll-mt-24"
        >
          {open && (
            <div className="grid max-w-[68ch] gap-5">
              <header className="grid gap-3">
                <p className="flex flex-wrap items-center gap-2 text-[11px] font-bold tracking-[0.1em] text-accent uppercase">
                  {sourceName(open.source)} · {open.feed} {!real && <ExampleBadge />}
                </p>
                <h2
                  id="noticias-titulo"
                  className="font-serif text-[clamp(26px,2.8vw,36px)] leading-[1.15] font-semibold tracking-[-0.01em]"
                >
                  {open.title}
                </h2>
                <dl className="grid gap-x-6 gap-y-1 text-[12.5px] text-muted-foreground sm:flex sm:flex-wrap">
                  <div className="flex gap-1.5">
                    <dt>Publicada</dt>
                    <dd className="font-semibold text-foreground">
                      <When at={open.published_at} />
                    </dd>
                  </div>
                  <div className="flex gap-1.5">
                    <dt>Leída por el sistema</dt>
                    <dd>
                      <When at={open.retrieved_at} />
                    </dd>
                  </div>
                </dl>
              </header>
              {open.body ? (
                <div className="grid gap-4 border-t border-border pt-5 font-serif text-[18px] leading-[1.6]">
                  {open.body.map((p) => (
                    <p key={p}>{p}</p>
                  ))}
                </div>
              ) : (
                <div className="grid gap-4 border-t border-border pt-5">
                  {open.url && (
                    <p className="text-[14px]">
                      El texto completo no se guarda: está en el sitio de la fuente.{' '}
                      <a
                        href={open.url}
                        target="_blank"
                        rel="noreferrer"
                        className="font-semibold text-accent underline underline-offset-2"
                      >
                        Leer en {sourceName(open.source)}
                      </a>
                    </p>
                  )}
                  {open.interpretations.length === 0
                    ? quiet('Todavía sin interpretación automática.')
                    : open.interpretations.map((it) => (
                        <Interpretation key={it.interpretation_id} it={it} />
                      ))}
                </div>
              )}
              <p className="text-[12px] text-muted-foreground">
                Para el sistema, la noticia existe desde su hora de publicación, nunca antes.
              </p>
            </div>
          )}
        </article>

        <ContextColumn />
      </div>
    </div>
  )
}
