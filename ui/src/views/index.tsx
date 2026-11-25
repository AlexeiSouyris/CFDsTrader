/**
 * Las vistas del prototipo (#164). Cada una arma componentes de `components/ui`; ninguna calcula
 * nada: solo ubica y muestra.
 *
 * Honestidad de los datos (#577): lo que todavía no tiene fuente en el motor (índices, mapa,
 * operaciones, competencia, noticias) son datos de ejemplo de `mocks/` y
 * llevan siempre el sello "EJEMPLO". El valor de la unidad sale del motor (`useFund`) cuando el
 * gateway responde, y los nombres de los instrumentos, del catálogo (`/api/v1/instruments`).
 */
import type { ReactNode } from 'react'

import { CoinsIcon } from '@/components/ui/animated-icons'
import { ExampleBadge } from '@/components/ui/data-honesty'
import { MarketHeatmap } from '@/components/ui/market-heatmap'
import { SparkTable } from '@/components/ui/spark-table'
import { TradeJournal } from '@/components/ui/trade-journal'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal, signed } from '@/lib/format'
import { useFund } from '@/lib/fund-api'
import { useInstruments } from '@/lib/liquidity-api'
import { cn } from '@/lib/utils'
import { HEATMAP, INDEX_ROWS, KPIS, PROCESS_BADGES, TRADES, type Kpi } from '@/mocks'
import { MultichartView } from '@/views/multichart'

// Noticias, Diario y Trofeos viven en su archivo (#581, etapas 7 y 8); se reexportan para no
// cambiar las importaciones.
export { NewsView } from '@/views/news'
export { JournalView } from '@/views/journal'
export { CompetitionView } from '@/views/trophies'

function Card({
  title,
  children,
  className,
  example = false,
  stampInside = false,
}: {
  title?: string
  children: ReactNode
  className?: string
  /** datos de ejemplo: lleva el sello "EJEMPLO" arriba a la derecha */
  example?: boolean
  /** el sello lo pone el contenido (los indicadores, en su renglón) */
  stampInside?: boolean
}) {
  const stamp = example && !stampInside
  return (
    <section
      data-example={example || undefined}
      className={cn('panel relative min-w-0 rounded-[20px] p-4', className)}
    >
      {(title || stamp) && (
        <div className="mb-3 flex items-start justify-between gap-2">
          {title ? <h2 className="text-[14px] font-bold">{title}</h2> : <span />}
          {stamp && <ExampleBadge />}
        </div>
      )}
      {children}
    </section>
  )
}

/** Términos del glosario de cada indicador del panel. */
const KPI_TERM: Record<string, string> = { nav: 'nav', pnl: 'pl', dd: 'drawdown' }

/** Los nombres del catálogo del motor (`/api/v1/instruments`), o nada sin gateway. */
function useInstrumentNames(): Record<string, string> {
  const catalog = useInstruments()
  return catalog.status === 'ok'
    ? Object.fromEntries(catalog.data.instruments.map((i) => [i.id, i.name]))
    : {}
}

export function OverviewView({ onOpen }: { onOpen: (view: string) => void }) {
  const fund = useFund()
  const names = useInstrumentNames()
  // el valor de la unidad, del motor cuando responde; el resto todavía no tiene fuente: ejemplo
  const kpis: (Kpi & { example: boolean })[] = KPIS.map((k) =>
    k.id === 'nav' && fund.live
      ? {
          ...k,
          value: fund.summary.nav,
          unit: fund.summary.currency,
          detail: fund.summary.valued_at
            ? `del motor, liquidación del ${dateTimeUtc(fund.summary.valued_at)}`
            : 'del motor, sin liquidaciones todavía',
          example: false,
        }
      : { ...k, example: true },
  )
  const indexRows = INDEX_ROWS.map((r) => ({ ...r, label: names[r.id] ?? r.label }))
  return (
    <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {kpis.map((k) => (
          <Card
            key={k.id}
            className="flex min-h-28 flex-col justify-between gap-2"
            example={k.example}
            stampInside
          >
            {/* en el celular, si no entra, el sello sube a su propio renglón y el nombre no se parte */}
            <span className="flex flex-wrap-reverse items-start justify-between gap-x-2 gap-y-1.5 text-[11px] font-bold text-muted-foreground">
              <span className="whitespace-nowrap">
                {KPI_TERM[k.id] ? <Term id={KPI_TERM[k.id]}>{k.label}</Term> : k.label}
              </span>
              <span className="flex items-center gap-1.5">
                {k.id === 'pnl' && <CoinsIcon size={16} />}
                {k.example && <ExampleBadge />}
              </span>
            </span>
            <strong
              className={cn(
                'text-[clamp(22px,2.4vw,32px)] leading-none font-extrabold tracking-[-0.04em] whitespace-nowrap tabular-nums',
                k.tone === 'up' && 'text-up',
                k.tone === 'down' && 'text-down',
              )}
            >
              {k.tone ? signed(k.value) : decimal(k.value, k.id === 'nav' ? 4 : undefined)}
              {k.unit && (
                <span className="ml-1 text-[12px] font-bold text-muted-foreground">{k.unit}</span>
              )}
            </strong>
            {k.id === 'props' ? (
              <button
                type="button"
                onClick={() => onOpen('propuestas')}
                className="-mx-1 self-start rounded-md px-1 text-left text-[10.5px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
              >
                {k.detail} →
              </button>
            ) : (
              <span className="text-[10.5px] text-muted-foreground">{k.detail}</span>
            )}
          </Card>
        ))}
      </div>
      <div className="grid grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
        <Card title="Índices" example>
          <SparkTable rows={indexRows} />
        </Card>
        <Card example>
          <MarketHeatmap data={HEATMAP} />
        </Card>
      </div>
      <div className="grid grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
        <Card example>
          <TradeJournal trades={TRADES.slice(0, 4)} title="Últimas operaciones" />
          <button
            type="button"
            onClick={() => onOpen('operaciones')}
            className="mt-3 text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
          >
            Ver el diario
          </button>
        </Card>
        <Card title="Trofeos de proceso" example>
          <ProcessBadges />
          <button
            type="button"
            onClick={() => onOpen('competencia')}
            className="mt-3 text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
          >
            Ver los trofeos
          </button>
        </Card>
      </div>
    </div>
  )
}

/**
 * Gráficos (#581, etapa 5): un solo gráfico a pantalla completa, con las velas del motor (o las de
 * ejemplo, con su sello, si el gateway no responde). Es el lienzo del Multigráfico con un gráfico.
 * Sin libro de órdenes: IG no lo da (#577).
 */
export function ChartsView() {
  return <MultichartView single />
}

/**
 * Los trofeos de proceso de la semana (#581): reemplazan al ranking por resultado del Panel viejo.
 * Premian respetar las reglas, nunca la ganancia; el estado llega hecho (la UI no cuenta).
 */
function ProcessBadges() {
  return (
    <ul className="grid gap-1.5">
      {PROCESS_BADGES.slice(0, 3).map((b) => (
        <li
          key={b.id}
          className="flex items-center gap-3 rounded-xl border border-border px-3 py-2"
        >
          <span className="min-w-0 flex-1 truncate text-[13px] font-semibold">{b.title}</span>
          <span className="shrink-0 text-[11px] font-bold tracking-[0.06em] text-muted-foreground uppercase">
            {b.state}
          </span>
        </li>
      ))}
    </ul>
  )
}
