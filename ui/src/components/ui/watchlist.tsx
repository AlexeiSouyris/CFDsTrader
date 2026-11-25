/**
 * Lista de mercados de la Terminal (#552): un renglón por instrumento con el último precio, la
 * hora, la variación y el atraso, tal como los manda la fuente de `lib/market-summary.ts`. Elegir
 * un renglón lo abre en el gráfico activo y en el ticket. No calcula nada.
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

export function Watchlist({
  rows,
  names,
  selected,
  onSelect,
  sourceLabel,
  loading,
}: {
  rows: MarketSummary[]
  /** nombre de cada instrumento, del catálogo */
  names: Record<string, string>
  selected: string
  onSelect: (instrument: string) => void
  sourceLabel: string
  loading?: boolean
}) {
  const anyChange = rows.some((r) => r.change_pct !== null)
  return (
    <section
      aria-label="Mercados"
      className="glass flex min-w-0 flex-col rounded-[20px] p-2"
      aria-busy={loading || undefined}
    >
      <header className="flex items-baseline justify-between gap-2 px-2 pt-1 pb-2">
        <h2 className="text-[13px] font-extrabold tracking-tight">Mercados</h2>
        <span className="truncate text-[10.5px] text-muted-foreground">{sourceLabel}</span>
      </header>
      <ul className="grid gap-0.5">
        {rows.map((r) => {
          const on = r.instrument_id === selected
          return (
            <li key={r.instrument_id}>
              <button
                type="button"
                aria-pressed={on}
                data-instrument={r.instrument_id}
                onClick={() => onSelect(r.instrument_id)}
                className={cn(
                  'grid min-h-11 w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 rounded-xl px-2.5 py-1.5 text-left transition-colors hover:bg-foreground/5 focus-visible:outline-2 focus-visible:outline-ring',
                  on && 'bg-primary/10 ring-1 ring-primary/40',
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
                  {r.delay_s !== null &&
                    ` · ${Math.max(1, Math.round(r.delay_s / 60))} min de atraso`}
                </span>
              </button>
            </li>
          )
        })}
      </ul>
      {!anyChange && (
        <p className="mt-2 px-2 pb-1 text-[10.5px] leading-snug text-muted-foreground">
          Sin variación ni atraso: la fuente todavía no los manda. Precios del pasado, no promesas.
        </p>
      )}
    </section>
  )
}
