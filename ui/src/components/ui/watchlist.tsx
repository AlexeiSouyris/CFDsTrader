/**
 * Lista de mercados de la Terminal (#552): un renglón por instrumento con el último precio, la
 * hora, la variación y el atraso, tal como los manda la fuente de `lib/market-summary.ts`. Elegir
 * un renglón lo abre en el gráfico activo (y, si se opera, en el ticket). No calcula nada.
 *
 * Los renglones van en grupos, uno por fuente (#556): los del broker simulado y los de IBKR, que
 * son solo datos y llevan su marca con el atraso que declara el gateway.
 *
 * Aspecto (#635, patrón *Market Watchlist* de 21st.dev, https://21st.dev/@ssychui/components/market-watchlist,
 * reescrito): una sola tabla densa, renglones separados por líneas de 1 px, cifras en mono
 * alineadas por columna y el elegido marcado con una barra ámbar a la izquierda. Sin tarjetas.
 */
import { ToolLock } from '@/components/ui/tool-lock'
import { dateTimeUtc, decimal, isNegative, signed } from '@/lib/format'
import type { MarketSummary } from '@/lib/market-summary'
import { cn } from '@/lib/utils'

// 24 h como el resto de la app: es-AR daba "04:00 a. m." (QA terminal #19)
const TIME = new Intl.DateTimeFormat('es-AR', {
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
  timeZone: 'UTC',
})
const DAY = new Intl.DateTimeFormat('es-AR', { day: '2-digit', month: '2-digit', timeZone: 'UTC' })

/** La hora de la vela en UTC; si no es de hoy, también el día (para que se note lo viejo). */
function when(iso: string): string {
  const d = new Date(iso)
  const today = DAY.format(new Date())
  const day = DAY.format(d)
  return day === today ? `${TIME.format(d)} UTC` : `${day} ${TIME.format(d)} UTC`
}

/** Un grupo de la lista: los renglones de una fuente. */
export type WatchGroup = {
  id: string
  title: string
  /** cómo se nombra la fuente (por ejemplo, "último cierre de 15 m del motor") */
  sourceLabel: string
  rows: MarketSummary[]
  /** `ibkr`: los renglones abren el gráfico con las velas de IBKR */
  feed?: 'ibkr'
  /** marca de cada renglón con el atraso (por ejemplo, "IBKR"); sin esto, el atraso va en texto */
  badge?: string
  loading?: boolean
  /** aviso del grupo: sin conexión, solo datos… */
  note?: string
}

/** El atraso como lo declara la fuente, en minutos enteros para la pantalla. */
const delayMin = (s: number) => Math.max(1, Math.round(s / 60))

/**
 * Un precio con al menos 2 decimales (QA terminal #19, charts #13): "25.590" solo se leía como 25
 * con decimales; "25.590,00" no. Si el precio trae más, se muestran todos. Solo formato.
 */
const fixed = (text: string) => decimal(text, Math.max(2, text.split('.')[1]?.length ?? 0))

type WatchlistProps = {
  groups: WatchGroup[]
  /** nombre de cada instrumento, del catálogo */
  names: Record<string, string>
  /** el renglón elegido: el id, o `ibkr:<id>` si es de IBKR */
  selected: string
  onSelect: (instrument: string, feed?: 'ibkr') => void
}

/** La lista de mercados, con su candado de la Academia (#771): la abre la lección 0.1. */
export function Watchlist(props: WatchlistProps) {
  return (
    <ToolLock tool="tool.watchlist">
      <MarketList {...props} />
    </ToolLock>
  )
}

function MarketList({ groups, names, selected, onSelect }: WatchlistProps) {
  return (
    <section
      aria-label="Mercados"
      className="panel flex min-w-0 flex-col rounded-section"
      aria-busy={groups.some((g) => g.loading) || undefined}
    >
      <h2 className="border-b border-border px-2.5 py-2 text-[11px] font-extrabold tracking-[0.12em] uppercase">
        Mercados
      </h2>
      {groups.map((g) => (
        <WatchGroupList
          key={g.id}
          group={g}
          names={names}
          selected={selected}
          onSelect={onSelect}
        />
      ))}
    </section>
  )
}

function WatchGroupList({
  group: g,
  names,
  selected,
  onSelect,
}: {
  group: WatchGroup
  names: Record<string, string>
  selected: string
  onSelect: (instrument: string, feed?: 'ibkr') => void
}) {
  const anyChange = g.rows.some((r) => r.change_pct !== null)
  const headingId = `watch-group-${g.id}`
  return (
    <section aria-labelledby={headingId} data-watch-group={g.id} className="grid">
      <header className="min-w-0 border-b border-border bg-surface px-2.5 py-1.5">
        <h3
          id={headingId}
          className="text-[10.5px] font-bold tracking-[0.08em] text-muted-foreground uppercase"
        >
          {g.title}
        </h3>
        <p className="truncate font-mono text-[10px] text-muted-foreground">{g.sourceLabel}</p>
      </header>
      <ul className="grid">
        {g.rows.map((r) => {
          const key = g.feed === 'ibkr' ? `ibkr:${r.instrument_id}` : r.instrument_id
          const on = key === selected
          return (
            <li key={key} className="border-b border-border/60">
              <button
                type="button"
                aria-pressed={on}
                data-instrument={r.instrument_id}
                data-feed={g.feed}
                onClick={() => onSelect(r.instrument_id, g.feed)}
                className={cn(
                  'relative grid min-h-9 w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 px-2.5 py-1.5 text-left transition-colors hover:bg-foreground/5 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring',
                  // el elegido: barra ámbar a la izquierda y fondo apenas tintado
                  on &&
                    "bg-primary/10 before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-primary before:content-['']",
                )}
              >
                <span className="min-w-0">
                  {/* el nombre conocido arriba y el id chico abajo (#861) */}
                  <span className="block truncate text-[12.5px] font-extrabold tracking-tight">
                    {names[r.instrument_id] ?? r.instrument_id}
                  </span>
                  <span className="block truncate text-[10.5px] text-muted-foreground">
                    {names[r.instrument_id] ? r.instrument_id : ''}
                  </span>
                </span>
                <span className="text-right">
                  <span className="block font-mono text-[12.5px] font-bold tabular-nums">
                    {r.last === null ? '—' : fixed(r.last)}
                  </span>
                  {r.change_pct !== null && (
                    <span
                      className={cn(
                        'block font-mono text-[10.5px] font-bold tabular-nums',
                        isNegative(r.change_pct) ? 'text-down' : 'text-up',
                      )}
                    >
                      {signed(r.change_pct, 2)} %
                      <span className="sr-only"> contra el cierre de la sesión anterior</span>
                    </span>
                  )}
                </span>
                <span
                  className="col-span-2 truncate font-mono text-[10px] text-muted-foreground tabular-nums"
                  title={r.at ? `Vela cerrada: ${dateTimeUtc(r.at)}` : undefined}
                >
                  {r.at ? when(r.at) : 'sin velas'}
                  {!g.badge && r.delay_s !== null && ` · ${delayMin(r.delay_s)} min de atraso`}
                </span>
                {g.badge && (
                  // `delay_s` es cuánto hace que cerró la última vela (`generated_at − bar_end`):
                  // se dice así, no como "atraso" de la fuente, que es otro número (#275)
                  <span
                    data-delay-badge
                    title={
                      r.delay_s !== null
                        ? `La última vela cerrada terminó hace unos ${delayMin(r.delay_s)} min`
                        : undefined
                    }
                    className="col-span-2 max-w-full justify-self-start truncate font-mono text-[10px] font-bold tracking-[0.04em] text-muted-foreground"
                  >
                    {g.badge}
                    {r.delay_s !== null && ` · cerró hace ~${delayMin(r.delay_s)} min`}
                  </span>
                )}
              </button>
            </li>
          )
        })}
      </ul>
      {g.note && (
        <p className="px-2.5 py-1.5 text-[10.5px] leading-snug text-muted-foreground">{g.note}</p>
      )}
      {!g.note && !anyChange && g.rows.length > 0 && (
        <p className="px-2.5 py-1.5 text-[10.5px] leading-snug text-muted-foreground">
          Sin variación ni atraso: la fuente todavía no los manda. Precios del pasado, no promesas.
        </p>
      )}
    </section>
  )
}
