/**
 * Cabecera de los gráficos de precio (#377): el precio grande y el cambio debajo, que siguen al
 * cursor y a la medición de `chart-measure`. Solo texto de pantalla (CLAUDE.md §1).
 */
import { AnimatedNumber } from '@/components/ui/animated-number'
import {
  changeText,
  TREND_COLOR,
  usePointer,
  type PointerStore,
} from '@/components/ui/chart-measure'
import { dateTimeUtc, decimal } from '@/lib/format'

/** "16 nov, 15:00 – 20 nov, 20:00 UTC" (o una sola fecha si es la misma vela). */
function dateRange(a: string, b: string) {
  const [x, y] = Date.parse(a) <= Date.parse(b) ? [a, b] : [b, a]
  return x === y ? dateTimeUtc(x) : `${dateTimeUtc(x).replace(/ UTC$/, '')} – ${dateTimeUtc(y)}`
}

/**
 * Cabecera del gráfico (#377): el precio grande y, debajo, el cambio. Quieta muestra el último
 * cierre y el cambio del rango visible; al pasar el cursor, el cierre de esa vela y su cambio
 * contra el inicio del rango; al medir, el cambio entre los dos puntos y sus fechas.
 * Es solo texto de pantalla: las cuentas son las de la medición, no lógica de trading.
 */
export function ChartReadout({
  store,
  starts,
  closes,
  rangeLabel,
}: {
  store: PointerStore
  starts: string[]
  /** cierres como llegan (texto decimal) */
  closes: string[]
  rangeLabel: string
}) {
  const hover = usePointer(store, (s) => s.hover)
  const measure = usePointer(store, (s) => s.measure)
  const n = closes.length
  if (n === 0) return null
  const digits = (closes[n - 1].split('.')[1] ?? '').length
  const at = (iso: string) => starts.indexOf(iso)

  let state: 'idle' | 'hover' | 'measure' = 'idle'
  let price = closes[n - 1]
  let change = changeText(Number(closes[0]), Number(closes[n - 1]), digits)
  let note = rangeLabel
  if (measure && at(measure.from.start) >= 0 && at(measure.to.start) >= 0) {
    state = 'measure'
    price = closes[at(measure.to.start)]
    change = changeText(measure.from.price, measure.to.price, digits)
    note = dateRange(measure.from.start, measure.to.start)
  } else if (hover !== null && hover >= 0 && hover < n) {
    state = 'hover'
    price = closes[hover]
    change = changeText(Number(closes[0]), Number(closes[hover]), digits)
    note = dateTimeUtc(starts[hover])
  }
  return (
    <div data-chart-readout={state}>
      <p className="mt-1 text-[26px] leading-none font-bold tracking-[-0.03em] tabular-nums">
        {state === 'idle' ? <AnimatedNumber value={price} format={decimal} /> : decimal(price)}
      </p>
      <p className="mt-1.5 flex flex-wrap items-baseline gap-x-1.5 text-[12px] leading-tight tabular-nums">
        <span
          data-chart-change={change.trend}
          className="font-bold"
          style={{ color: TREND_COLOR[change.trend] }}
        >
          {change.text}
        </span>
        {/* el rango de fechas de una medición es largo: va en su propia línea */}
        <span
          className={
            state === 'measure' ? 'basis-full text-muted-foreground' : 'text-muted-foreground'
          }
        >
          {state === 'measure' ? note : `· ${note}`}
        </span>
      </p>
    </div>
  )
}
