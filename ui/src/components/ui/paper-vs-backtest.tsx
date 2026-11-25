/**
 * "Paper vs backtest" (fase 8, #331): la última `paper_comparison.v1` de cada instrumento, tal
 * como la calculó el motor. Operaciones emparejadas, faltantes y sobrantes, y la diferencia de
 * P/L repartida por causa.
 *
 * **No calcula nada de trading:** los montos y el reparto llegan del motor, y la suma de las
 * causas la garantiza él. Lo único que se hace acá es dibujar: el largo de cada barra es su monto
 * relativo al mayor, solo para verlo.
 */
import { ScrollRegion } from '@/components/ui/scroll-region'
import { useState } from 'react'

import { Price } from '@/components/ui/run-detail'
import { Segmented } from '@/components/ui/segmented'
import { dateTimeUtc, isNegative, signed } from '@/lib/format'
import { CAUSES, type Cause, type PaperComparison } from '@/lib/learning-api'
import { cn } from '@/lib/utils'

const CAUSE_LABEL: Record<Cause, string> = {
  fill_price: 'Precio de ejecución',
  size: 'Tamaño',
  commission: 'Comisión',
  swap: 'Financiación (swap)',
  weekend_close: 'Cierre de fin de semana',
  risk_rejection: 'Rechazo del riesgo',
  missing_in_paper: 'Faltan en paper',
  extra_in_paper: 'Sobran en paper',
}

const SIDE = { long: 'largo', short: 'corto' } as const

const tone = (v: string) => (isNegative(v) ? 'text-down' : 'text-up')

/** Lo que dice el motor de una operación sin pareja, en palabras. */
function reason(r: string): string {
  if (r === 'no_order') return 'paper no mandó orden'
  if (r === 'no_trade') return 'el backtest no operó esa vela'
  if (r.startsWith('rejected:')) return `el backtest la rechazó (${r.slice('rejected:'.length)})`
  return `paper: rechazada (${r})`
}

/** Barras de la diferencia por causa, divergentes desde el centro. */
export function CauseBars({ c }: { c: PaperComparison }) {
  // Solo para dibujar: el ancho de cada barra relativo a la causa más grande.
  const scale = Math.max(...CAUSES.map((k) => Math.abs(Number(c.causes[k]))), 0)
  return (
    <ul className="grid gap-1.5" aria-label="Diferencia de P/L por causa">
      {CAUSES.map((k) => {
        const v = c.causes[k]
        const share = scale > 0 ? (Math.abs(Number(v)) / scale) * 50 : 0
        return (
          <li
            key={k}
            data-cause={k}
            className="grid grid-cols-[minmax(0,9rem)_1fr_auto] items-center gap-2 text-[12px]"
          >
            <span className="truncate">{CAUSE_LABEL[k]}</span>
            <span className="relative h-2.5 rounded-full bg-border/60" aria-hidden="true">
              <span className="absolute inset-y-0 left-1/2 w-px bg-foreground/30" />
              <span
                className={cn(
                  'absolute inset-y-0 rounded-full transition-[width] duration-500',
                  isNegative(v) ? 'right-1/2 bg-down' : 'left-1/2 bg-up',
                )}
                style={{ width: `${share}%` }}
              />
            </span>
            <span className={cn('min-w-16 text-right font-bold tabular-nums', tone(v))}>
              {signed(v)}
            </span>
          </li>
        )
      })}
    </ul>
  )
}

const th =
  'py-2 pr-3 text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase'
const td = 'py-2 pr-3 tabular-nums whitespace-nowrap'

function Comparison({ c }: { c: PaperComparison }) {
  return (
    <div className="grid gap-4" data-comparison={c.comparison_id}>
      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {(
          [
            ['Período', `${dateTimeUtc(c.from)} → ${dateTimeUtc(c.to)}`],
            ['P/L paper', `${signed(c.paper.net_pnl)} · ${c.paper.trades} op.`],
            ['P/L backtest', `${signed(c.backtest.net_pnl)} · ${c.backtest.trades} op.`],
            ['Diferencia', signed(c.difference)],
          ] as const
        ).map(([label, value]) => (
          <div key={label} className="min-w-0">
            <dt className="text-[10.5px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
              {label}
            </dt>
            <dd
              className={cn(
                'mt-1 text-[13px] font-bold break-words tabular-nums',
                label === 'Diferencia' && tone(c.difference),
              )}
            >
              {value}
            </dd>
          </div>
        ))}
      </dl>

      <section>
        <h4 className="mb-2 text-[12.5px] font-bold">Diferencia (paper − backtest) por causa</h4>
        <CauseBars c={c} />
      </section>

      <section>
        <h4 className="mb-2 text-[12.5px] font-bold">Emparejadas ({c.matched.length})</h4>
        {c.matched.length === 0 ? (
          <p className="text-[12px] text-muted-foreground">Ninguna.</p>
        ) : (
          <ScrollRegion label="Operaciones emparejadas">
            <table
              className="w-full min-w-[560px] text-[12px]"
              aria-label="Operaciones emparejadas"
            >
              <thead>
                <tr className="border-b border-border">
                  <th className={th}>Decisión</th>
                  <th className={th}>Lado</th>
                  <th className={th}>Paper (entrada → salida, P/L)</th>
                  <th className={th}>Backtest</th>
                  <th className={th}>Diferencia</th>
                </tr>
              </thead>
              <tbody>
                {c.matched.map((m) => (
                  <tr key={m.order_id} className="border-b border-border last:border-b-0">
                    <td className={td}>{dateTimeUtc(m.decision_at)}</td>
                    <td className={td}>{SIDE[m.side]}</td>
                    <td className={td}>
                      <Price value={m.paper.entry_price} /> →{' '}
                      <Price value={m.paper.exit_price} reference={m.paper.entry_price} />,{' '}
                      {signed(m.paper.net_pnl)}
                    </td>
                    <td className={td}>
                      <Price value={m.backtest.entry_price} /> →{' '}
                      <Price value={m.backtest.exit_price} reference={m.backtest.entry_price} />,{' '}
                      {signed(m.backtest.net_pnl)}
                    </td>
                    <td className={cn(td, 'font-bold', tone(m.difference.net_pnl))}>
                      {signed(m.difference.net_pnl)}
                      {m.weekend_close && (
                        <span className="ml-1 font-normal text-muted-foreground">
                          (cierre de fin de semana)
                        </span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </ScrollRegion>
        )}
      </section>

      <div className="grid gap-4 md:grid-cols-2">
        <section>
          <h4 className="mb-2 text-[12.5px] font-bold">Faltan en paper ({c.missing.length})</h4>
          {c.missing.length === 0 ? (
            <p className="text-[12px] text-muted-foreground">Ninguna.</p>
          ) : (
            <ul
              className="grid border-t border-border text-[12px]"
              aria-label="Operaciones que faltan en paper"
            >
              {c.missing.map((m) => (
                <li key={m.decision_at + m.side} className="border-b border-border px-1 py-2">
                  {dateTimeUtc(m.decision_at)} · {SIDE[m.side]} · backtest{' '}
                  <span className={cn('font-bold tabular-nums', tone(m.backtest_net_pnl))}>
                    {signed(m.backtest_net_pnl)}
                  </span>
                  <span className="block text-muted-foreground">{reason(m.paper)}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section>
          <h4 className="mb-2 text-[12.5px] font-bold">Sobran en paper ({c.extra.length})</h4>
          {c.extra.length === 0 ? (
            <p className="text-[12px] text-muted-foreground">Ninguna.</p>
          ) : (
            <ul
              className="grid border-t border-border text-[12px]"
              aria-label="Operaciones que sobran en paper"
            >
              {c.extra.map((e) => (
                <li key={e.order_id} className="border-b border-border px-1 py-2">
                  {dateTimeUtc(e.decision_at)} · {SIDE[e.side]} · paper{' '}
                  <span className={cn('font-bold tabular-nums', tone(e.paper_net_pnl))}>
                    {signed(e.paper_net_pnl)}
                  </span>
                  <span className="block text-muted-foreground">{reason(e.backtest)}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      {Object.keys(c.paper_rejections).length > 0 && (
        <p className="text-[11.5px] text-muted-foreground">
          Órdenes de paper rechazadas sin operación del backtest (no mueven el P/L):{' '}
          {Object.entries(c.paper_rejections)
            .map(([k, n]) => `${k} ×${n}`)
            .join(', ')}
          .
        </p>
      )}
    </div>
  )
}

/** Una comparación por instrumento, con un selector si hay varias. */
export function PaperVsBacktest({ comparisons }: { comparisons: PaperComparison[] }) {
  const ids = comparisons.map((c) => c.instrument_id)
  const [picked, setPicked] = useState<string | null>(null)
  const selected = picked && ids.includes(picked) ? picked : ids[0]
  const c = comparisons.find((x) => x.instrument_id === selected)
  if (!c) return null
  return (
    <div className="grid gap-3">
      {ids.length > 1 && (
        <Segmented
          label="Instrumento de la comparación"
          options={ids}
          value={selected}
          onChange={setPicked}
        />
      )}
      <Comparison key={c.comparison_id} c={c} />
    </div>
  )
}
