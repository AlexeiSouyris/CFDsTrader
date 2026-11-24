/**
 * Las vistas del prototipo (#164). Cada una arma componentes de `components/ui` con los datos
 * de ejemplo de `mocks/`. Ninguna calcula nada: solo ubica y muestra.
 */
import type { ReactNode } from 'react'

import { CoinsIcon } from '@/components/ui/animated-icons'
import { DepthChart } from '@/components/ui/depth-chart'
import { GlassFolder } from '@/components/ui/glass-folder'
import { MarketHeatmap } from '@/components/ui/market-heatmap'
import { SparkTable } from '@/components/ui/spark-table'
import { TradeJournal } from '@/components/ui/trade-journal'
import { TradingChart } from '@/components/ui/trading-chart'
import { Term } from '@/components/ui/term'
import { TrophyCard } from '@/components/ui/trophy-card'
import { decimal, isNegative, signed } from '@/lib/format'
import { cn } from '@/lib/utils'
import {
  CURRENT_MEMBER,
  FOLDERS,
  HEATMAP,
  INDEX_ROWS,
  KPIS,
  RANKING,
  TRADES,
  TROPHIES,
  US100_1H,
  US100_BOOK,
} from '@/mocks'

function Card({
  title,
  children,
  className,
}: {
  title?: string
  children: ReactNode
  className?: string
}) {
  return (
    <section
      className={cn(
        'glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30',
        className,
      )}
    >
      {title && <h2 className="mb-3 text-[14px] font-bold">{title}</h2>}
      {children}
    </section>
  )
}

/** Términos del glosario de cada indicador del panel. */
const KPI_TERM: Record<string, string> = { nav: 'nav', pnl: 'pl', dd: 'drawdown' }

export function OverviewView({ onOpen }: { onOpen: (view: string) => void }) {
  return (
    <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {KPIS.map((k) => (
          <Card key={k.id} className="flex min-h-28 flex-col justify-between gap-2">
            <span className="flex items-start justify-between gap-2 text-[11px] font-bold text-muted-foreground">
              {KPI_TERM[k.id] ? <Term id={KPI_TERM[k.id]}>{k.label}</Term> : k.label}
              {k.id === 'pnl' && <CoinsIcon size={16} />}
            </span>
            <strong
              className={cn(
                'text-[clamp(22px,2.4vw,32px)] leading-none font-extrabold tracking-[-0.04em] whitespace-nowrap tabular-nums',
                k.tone === 'up' && 'text-up',
                k.tone === 'down' && 'text-down',
              )}
            >
              {k.tone ? signed(k.value) : decimal(k.value)}
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
        <Card title="Índices">
          <SparkTable rows={INDEX_ROWS} />
        </Card>
        <Card>
          <MarketHeatmap data={HEATMAP} />
        </Card>
      </div>
      <div className="grid grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
        <Card>
          <TradeJournal trades={TRADES.slice(0, 4)} title="Últimas operaciones" />
          <button
            type="button"
            onClick={() => onOpen('operaciones')}
            className="mt-3 text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
          >
            Ver todas las operaciones
          </button>
        </Card>
        <Card title="Competencia de la semana">
          <Ranking compact />
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

export function ChartsView() {
  return (
    <div className="grid gap-3">
      <Card>
        <TradingChart instrument={US100_1H.instrument_id} bars={US100_1H.candles} />
      </Card>
      <Card>
        <DepthChart instrument="US100" book={US100_BOOK} />
      </Card>
    </div>
  )
}

export function NewsView() {
  return (
    <div>
      <p className="mb-6 max-w-2xl text-[13px] text-muted-foreground">
        Cada miembro guarda en su carpeta las noticias que le importan. Todos pueden ver las
        carpetas de los demás, pero cada uno modifica solo la suya.
      </p>
      <div className="grid justify-items-center gap-x-8 gap-y-24 pt-20 md:grid-cols-2 xl:grid-cols-3">
        {FOLDERS.map((f) => (
          <GlassFolder key={f.owner.id} folder={f} editable={f.owner.id === CURRENT_MEMBER.id} />
        ))}
      </div>
    </div>
  )
}

export function JournalView() {
  return (
    <Card>
      <TradeJournal trades={TRADES} />
    </Card>
  )
}

export function CompetitionView() {
  return (
    <div className="grid gap-6 lg:grid-cols-[auto_1fr]">
      <div className="flex flex-wrap justify-center gap-6">
        {TROPHIES.map((t) => (
          <TrophyCard key={t.serial} trophy={t} />
        ))}
      </div>
      <Card title="Tabla de la semana (R, simulación)">
        <Ranking />
      </Card>
    </div>
  )
}

function Ranking({ compact = false }: { compact?: boolean }) {
  const rows = compact ? RANKING.slice(0, 3) : RANKING
  return (
    <ol className="grid gap-1.5">
      {rows.map((r, i) => (
        <li
          key={r.member.id}
          className="flex items-center gap-3 rounded-xl border border-border px-3 py-2"
        >
          <span className="w-5 shrink-0 text-[12px] font-extrabold text-muted-foreground tabular-nums">
            {i + 1}
          </span>
          <span
            aria-hidden="true"
            className="grid size-8 shrink-0 place-items-center rounded-full border border-border text-[10px] font-bold"
          >
            {r.member.initials}
          </span>
          <span className="min-w-0 flex-1 truncate text-[13px] font-semibold">{r.member.name}</span>
          {!compact && (
            <span className="shrink-0 text-[11px] text-muted-foreground tabular-nums">
              {r.trades} operaciones
            </span>
          )}
          <span
            className={cn(
              'shrink-0 text-right text-[13px] font-extrabold whitespace-nowrap tabular-nums',
              isNegative(r.result) ? 'text-down' : 'text-up',
            )}
          >
            {signed(r.result)} R
          </span>
        </li>
      ))}
    </ol>
  )
}
