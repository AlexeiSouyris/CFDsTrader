/**
 * Diario de operaciones (simulación): fecha, instrumento, dirección, tamaño, entrada,
 * P/L neto y estado, con filtro de rango.
 *
 * Inspirado en "Trade Journal Table" (21st.dev), reescrito (#164). El original calculaba las
 * comisiones en la UI; acá el P/L neto llega **ya con los costos** que calcula el motor
 * (spec §11), y la UI solo lo muestra.
 */
import { Check, X } from 'lucide-react'
import { useState } from 'react'

import { Segmented } from '@/components/ui/segmented'
import { dateTimeUtc, decimal, isNegative, signed } from '@/lib/format'
import { cn } from '@/lib/utils'
import type { TradeRow } from '@/mocks'

const RANGES = { '7 días': 7, '30 días': 30, Todo: Infinity } as const
type Range = keyof typeof RANGES
const DAY = 86_400_000

export function TradeJournal({
  trades,
  title = 'Diario de operaciones',
}: {
  trades: TradeRow[]
  title?: string
}) {
  const [range, setRange] = useState<Range>('Todo')
  const newest = Math.max(...trades.map((t) => Date.parse(t.at)))
  const shown = trades
    .filter((t) => newest - Date.parse(t.at) <= RANGES[range] * DAY)
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at))

  return (
    <section aria-label={title} className="w-full">
      <header className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
          <h2 className="text-[15px] font-bold">{title}</h2>
          <span className="text-[11px] text-muted-foreground tabular-nums">
            {shown.length} {shown.length === 1 ? 'operación' : 'operaciones'} · simulación
          </span>
        </div>
        <Segmented
          label="Rango"
          options={Object.keys(RANGES)}
          value={range}
          onChange={(v) => setRange(v as Range)}
        />
      </header>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[560px] border-separate border-spacing-0 text-[12px] tabular-nums">
          <caption className="sr-only">{title}, de la más nueva a la más vieja</caption>
          <thead>
            <tr className="text-left text-[10px] tracking-[0.08em] text-muted-foreground uppercase">
              {[
                'Fecha',
                'Instrumento',
                'Dirección',
                'Tamaño',
                'Entrada',
                'P/L neto (EUR)',
                'Estado',
              ].map((h, i) => (
                <th
                  key={h}
                  scope="col"
                  className={cn(
                    'border-b border-border pb-2 font-bold whitespace-nowrap',
                    i >= 3 && 'text-right',
                  )}
                >
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shown.map((t) => {
              const long = t.side === 'Largo'
              return (
                <tr
                  key={t.id}
                  className="hover:bg-[color-mix(in_srgb,var(--foreground)_4%,transparent)]"
                >
                  <td className="border-b border-border py-2.5 whitespace-nowrap text-muted-foreground">
                    {dateTimeUtc(t.at)}
                  </td>
                  <td className="border-b border-border py-2.5 font-bold whitespace-nowrap">
                    {t.instrument}
                  </td>
                  <td className="border-b border-border py-2.5">
                    <span
                      className={cn(
                        'rounded-md px-1.5 py-0.5 text-[11px] font-bold',
                        long ? 'bg-up/10 text-up' : 'bg-down/10 text-down',
                      )}
                    >
                      {t.side}
                    </span>
                  </td>
                  <td className="border-b border-border py-2.5 text-right">{decimal(t.size)}</td>
                  <td className="border-b border-border py-2.5 text-right">{decimal(t.entry)}</td>
                  <td
                    className={cn(
                      'border-b border-border py-2.5 text-right font-bold whitespace-nowrap',
                      t.netPnl === null
                        ? 'text-muted-foreground'
                        : isNegative(t.netPnl)
                          ? 'text-down'
                          : 'text-up',
                    )}
                  >
                    {t.netPnl === null ? '—' : signed(t.netPnl, 2)}
                  </td>
                  <td className="border-b border-border py-2.5 text-right whitespace-nowrap">
                    <Status status={t.status} />
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
      {shown.length === 0 && (
        <p className="py-6 text-center text-[12px] text-muted-foreground">
          No hay operaciones en este rango.
        </p>
      )}
    </section>
  )
}

function Status({ status }: { status: TradeRow['status'] }) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10.5px] font-bold',
        status === 'Ganada' && 'border-up/30 bg-up/10 text-up',
        status === 'Perdida' && 'border-down/30 bg-down/10 text-down',
        status === 'Abierta' && 'border-border text-muted-foreground',
      )}
    >
      {status === 'Ganada' && <Check className="size-3" strokeWidth={3} aria-hidden="true" />}
      {status === 'Perdida' && <X className="size-3" strokeWidth={3} aria-hidden="true" />}
      {status === 'Abierta' && <i aria-hidden="true" className="size-1.5 rounded-full bg-up" />}
      {status}
    </span>
  )
}
