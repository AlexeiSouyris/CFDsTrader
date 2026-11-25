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
import { dateTimeUtc, price as priceText, priceDigits } from '@/lib/format'

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
  from = 0,
  to,
  ohlc,
}: {
  store: PointerStore
  starts: string[]
  /** cierres como llegan (texto decimal) */
  closes: string[]
  rangeLabel: string
  /** primera vela del tramo visible (#401): contra su cierre se mide el cambio */
  from?: number
  /** última vela del tramo visible (por defecto, la última) */
  to?: number
  /**
   * Las velas tal como llegan (#581, Terminal): con esto se ve el OHLC de la vela bajo el cursor (o
   * de la última visible), como en una terminal. Mismo orden que `starts` y `closes`.
   */
  ohlc?: readonly { open: string; high: string; low: string; close: string }[]
}) {
  const hover = usePointer(store, (s) => s.hover)
  const measure = usePointer(store, (s) => s.measure)
  const n = closes.length
  if (n === 0) return null
  const digits = priceDigits(closes.slice(-50))
  const fmt = (t: string) => priceText(t, digits)
  const at = (iso: string) => starts.indexOf(iso)

  let state: 'idle' | 'hover' | 'measure' = 'idle'
  const clampI = (i: number) => Math.max(0, Math.min(n - 1, i))
  const first = closes[clampI(from)]
  let price = closes[clampI(to ?? n - 1)]
  let change = changeText(Number(first), Number(price), digits)
  let note = rangeLabel
  let barAt = clampI(to ?? n - 1)
  if (measure && at(measure.from.start) >= 0 && at(measure.to.start) >= 0) {
    state = 'measure'
    price = closes[at(measure.to.start)]
    change = changeText(measure.from.price, measure.to.price, digits)
    note = dateRange(measure.from.start, measure.to.start)
  } else if (hover !== null && hover >= 0 && hover < n) {
    state = 'hover'
    barAt = hover
    price = closes[hover]
    change = changeText(Number(first), Number(closes[hover]), digits)
    note = dateTimeUtc(starts[hover])
  }
  return (
    <div data-chart-readout={state}>
      <p className="mt-1 text-[26px] leading-none font-bold tracking-[-0.03em] tabular-nums">
        {state === 'idle' ? <AnimatedNumber value={price} format={fmt} /> : fmt(price)}
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
      {ohlc?.[barAt] && <OhlcLine bar={ohlc[barAt]} fmt={fmt} />}
    </div>
  )
}

/** "O 1.0841  H 1.0850  L 1.0838  C 1.0846": los cuatro precios de una vela, tal como llegan. */
function OhlcLine({
  bar,
  fmt,
}: {
  bar: { open: string; high: string; low: string; close: string }
  fmt: (t: string) => string
}) {
  const parts = [
    ['O', 'apertura', bar.open],
    ['H', 'máximo', bar.high],
    ['L', 'mínimo', bar.low],
    ['C', 'cierre', bar.close],
  ] as const
  return (
    <dl data-chart-ohlc className="mt-1 flex flex-wrap gap-x-3 font-mono text-[11px] tabular-nums">
      {parts.map(([k, name, v]) => (
        <div key={k} className="flex gap-1">
          <dt className="text-muted-foreground">
            <abbr title={name} className="no-underline">
              {k}
            </abbr>
          </dt>
          <dd>{fmt(v)}</dd>
        </div>
      ))}
    </dl>
  )
}
