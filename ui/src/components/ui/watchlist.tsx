/**
 * Lista de mercados de la Terminal (#552): un renglón por instrumento con el último precio, la
 * hora, la variación y el atraso, tal como los manda la fuente de `lib/market-summary.ts`. Elegir
 * un renglón lo abre en el gráfico activo (y, si se opera, en el ticket). No calcula nada.
 *
 * Los renglones van en grupos, uno por fuente (#556): los del broker simulado y los de IBKR, que
 * son solo datos y llevan su marca con el atraso que declara el gateway.
 */
import { dateTimeUtc, decimal, isNegative, signed } from '@/lib/format'
import type { MarketSummary } from '@/lib/market-summary'
import { cn } from '@/lib/utils'

const TIME = new Intl.DateTimeFormat('es-AR', {
  hour: '2-digit',
  minute: '2-digit',
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

export function Watchlist({
  groups,
  names,
  selected,
  onSelect,
}: {
  groups: WatchGroup[]
  /** nombre de cada instrumento, del catálogo */
  names: Record<string, string>
  /** el renglón elegido: el id, o `ibkr:<id>` si es de IBKR */
  selected: string
  onSelect: (instrument: string, feed?: 'ibkr') => void
}) {
  return (
    <section
      aria-label="Mercados"
      className="panel flex min-w-0 flex-col gap-2 rounded-section p-2"
      aria-busy={groups.some((g) => g.loading) || undefined}
    >
      <h2 className="px-2 pt-1 text-[13px] font-extrabold tracking-tight">Mercados</h2>
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
    <section aria-labelledby={headingId} data-watch-group={g.id} className="grid gap-0.5">
      <header className="min-w-0 px-2 pb-1">
        <h3
          id={headingId}
          className="text-[10.5px] font-bold tracking-[0.08em] text-muted-foreground uppercase"
        >
          {g.title}
        </h3>
        <p className="truncate text-[10.5px] text-muted-foreground">{g.sourceLabel}</p>
      </header>
      <ul className="grid gap-0.5">
        {g.rows.map((r) => {
          const key = g.feed === 'ibkr' ? `ibkr:${r.instrument_id}` : r.instrument_id
          const on = key === selected
          return (
            <li key={key}>
              <button
                type="button"
                aria-pressed={on}
                data-instrument={r.instrument_id}
                data-feed={g.feed}
                onClick={() => onSelect(r.instrument_id, g.feed)}
                className={cn(
                  'grid min-h-9 w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 rounded-xl px-2 py-1 text-left transition-colors hover:bg-foreground/5 focus-visible:outline-2 focus-visible:outline-ring',
                  on && 'bg-primary/10 ring-1 ring-primary/50 ring-inset',
                )}
              >
                <span className="min-w-0">
                  <span className="block text-[12.5px] font-extrabold tracking-tight">
                    {r.instrument_id}
                  </span>
                  <span className="block truncate text-[10.5px] text-muted-foreground">
                    {names[r.instrument_id] ?? ''}
                  </span>
                </span>
                <span className="text-right">
                  <span className="block text-[12.5px] font-bold tabular-nums">
                    {r.last === null ? '—' : decimal(r.last)}
                  </span>
                  {r.change_pct !== null && (
                    <span
                      className={cn(
                        'block text-[10.5px] font-bold tabular-nums',
                        isNegative(r.change_pct) ? 'text-down' : 'text-up',
                      )}
                    >
                      {signed(r.change_pct)} %
                    </span>
                  )}
                </span>
                <span
                  className="col-span-2 truncate text-[10px] text-muted-foreground tabular-nums"
                  title={r.at ? `Vela cerrada: ${dateTimeUtc(r.at)}` : undefined}
                >
                  {r.at ? when(r.at) : 'sin velas'}
                  {!g.badge && r.delay_s !== null && ` · ${delayMin(r.delay_s)} min de atraso`}
                </span>
                {g.badge && (
                  <span
                    data-delay-badge
                    className="col-span-2 mt-0.5 max-w-full justify-self-start truncate rounded-full border border-border px-1.5 py-px text-[9.5px] font-bold text-muted-foreground"
                  >
                    {g.badge}
                    {r.delay_s !== null && ` · ~${delayMin(r.delay_s)} min de atraso`}
                  </span>
                )}
              </button>
            </li>
          )
        })}
      </ul>
      {g.note && (
        <p className="px-2 pb-1 text-[10.5px] leading-snug text-muted-foreground">{g.note}</p>
      )}
      {!g.note && !anyChange && g.rows.length > 0 && (
        <p className="px-2 pb-1 text-[10.5px] leading-snug text-muted-foreground">
          Sin variación ni atraso: la fuente todavía no los manda. Precios del pasado, no promesas.
        </p>
      )}
    </section>
  )
}
