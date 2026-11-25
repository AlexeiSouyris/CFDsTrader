/**
 * Noticias y Contexto de mercado (#581, etapa 7 de docs/design/2026-11-rediseno-ui.md): un lector.
 * A la izquierda la lista (fuente, titular y hora de publicación real en UTC), al centro el
 * artículo con tipografía de lectura y a la derecha la columna "Contexto": el índice de sentimiento
 * y los análogos tal como los calculó Python y los sirve el gateway, y el calendario.
 *
 * Honestidad de los datos (#577): las noticias, las carpetas y el calendario todavía no tienen ruta
 * en el gateway, así que son de ejemplo (`mocks/`) y llevan el sello "EJEMPLO". Las carpetas de
 * cada miembro son filtros: solo eligen qué se ve. La UI no calcula nada (CLAUDE.md §1).
 */
import { useRef, useState, type ReactNode } from 'react'

import { ExampleBadge, ExampleNotice } from '@/components/ui/data-honesty'
import { AnalogDistribution, SentimentGauge } from '@/components/ui/market-context'
import { Segmented } from '@/components/ui/segmented'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, dateUtc } from '@/lib/format'
import { useMarketContext } from '@/lib/use-market-context'
import { cn } from '@/lib/utils'
import { CALENDAR_EXAMPLE, CURRENT_MEMBER, FOLDERS, NEWS_EXAMPLE, type NewsExample } from '@/mocks'

const SOURCE: Record<NewsExample['source'], string> = { fed: 'Fed', ecb: 'BCE' }
const ALL = 'todas'

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

/** La columna "Contexto": lo que mandó el motor de Python y el calendario (de ejemplo). */
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
        <ContextBlock
          title={
            <>
              Calendario <ExampleBadge />
            </>
          }
        >
          <ol className="grid gap-2.5">
            {CALENDAR_EXAMPLE.map((e) => (
              <li key={e.event} className="grid gap-0.5">
                <When at={e.scheduled_at} className="text-[11.5px] text-muted-foreground" />
                <span className="text-[13.5px] font-semibold">{e.event}</span>
                <span className="text-[11.5px] text-muted-foreground">
                  {e.country} · impacto {e.impact} · en el calendario desde el{' '}
                  <time dateTime={e.published_at}>{dateUtc(e.published_at)}</time>
                </span>
              </li>
            ))}
          </ol>
        </ContextBlock>
      </div>
    </aside>
  )
}

export function NewsView() {
  const [folder, setFolder] = useState(ALL)
  const items =
    folder === ALL ? NEWS_EXAMPLE : NEWS_EXAMPLE.filter((n) => n.folders.includes(folder))
  const [openId, setOpenId] = useState(NEWS_EXAMPLE[0]?.item_id)
  const open = items.find((n) => n.item_id === openId) ?? items[0]
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
      <ExampleNotice>
        Las noticias, las carpetas y el calendario son de ejemplo: la UI todavía no lee las noticias
        del motor. El sentimiento y los análogos sí vienen del motor.
      </ExampleNotice>

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
          Cada miembro guarda en su carpeta las noticias que le importan. Todos ven todas; cada uno
          modifica solo la suya.
        </p>
      </div>

      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-6 lg:grid-cols-[16rem_minmax(0,1fr)] xl:grid-cols-[16rem_minmax(0,1fr)_21rem]">
        <nav aria-label="Noticias" className="min-w-0">
          {items.length === 0 ? (
            quiet('Esta carpeta está vacía.')
          ) : (
            <ol className="grid border-t border-border">
              {items.map((n) => {
                const active = n.item_id === open?.item_id
                return (
                  <li key={n.item_id} className="border-b border-border">
                    <button
                      type="button"
                      aria-current={active ? 'true' : undefined}
                      onClick={() => read(n.item_id)}
                      className={cn(
                        'grid w-full gap-1 border-l-2 py-3 pr-2 pl-3 text-left transition-colors outline-accent focus-visible:outline-2 focus-visible:-outline-offset-2',
                        active
                          ? 'border-accent bg-accent/10'
                          : 'border-transparent hover:bg-foreground/5',
                      )}
                    >
                      <span className="text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
                        {SOURCE[n.source]} · {n.feed}
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
                  {SOURCE[open.source]} · {open.feed} <ExampleBadge />
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
              <div className="grid gap-4 border-t border-border pt-5 font-serif text-[18px] leading-[1.6]">
                {open.body.map((p) => (
                  <p key={p}>{p}</p>
                ))}
              </div>
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
