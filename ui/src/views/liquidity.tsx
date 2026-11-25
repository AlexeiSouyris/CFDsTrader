/**
 * Vista de liquidez en vivo (#231, spec §5.4.1). Solo ubica componentes y les pasa lo que manda el
 * motor: la UI no calcula niveles, estados, FVG, tasas ni el desequilibrio (CLAUDE.md §1).
 *
 * Los datos llegan del gateway: los instrumentos y timeframes de `GET /api/v1/instruments` y la
 * vista de `GET /api/v1/liquidity` (`contracts/liquidity_view.v1`), que se vuelve a pedir cada
 * minuto. Sin velas guardadas muestra "sin datos todavía".
 */
import { StaleNote } from '@/components/ui/data-honesty'
import { createContext, useContext, useState, type ReactNode } from 'react'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { FvgTable, ImbalanceGauge, LevelsTable, StatsPanel } from '@/components/ui/liquidity-panels'
import { Segmented } from '@/components/ui/segmented'
import { Term } from '@/components/ui/term'
import { fetchOlderBars, useInstruments, useLiquidityView } from '@/lib/liquidity-api'
import { chartBars } from '@/lib/liquidity-bars'
import type { LiquidityViewData, SourceChoice, ViewTimeframe } from '@/lib/liquidity-contracts'
import { dateTimeUtc } from '@/lib/format'

/**
 * Nivel de los títulos de las tarjetas: `h2` en `#liquidez` sola y `h3` dentro del Panel, donde
 * "Liquidez" ya es el `h2` (#357). Solo cambia la etiqueta, no el aspecto.
 */
const HeadingLevel = createContext<2 | 3>(2)

/**
 * Una sección del informe (#626, patrón *Article With Author Sidebar* de 21st.dev,
 * https://21st.dev/@olewandowski1/components/article-5, reescrito): una regla fina arriba, número
 * y título, y el contenido directo sobre la página. Sin caja, sin fondo y sin esquinas: así
 * Liquidez se lee como un informe y no como una pila de tarjetas iguales (#625).
 */
function Card({ n, title, children }: { n?: string; title?: ReactNode; children: ReactNode }) {
  const Heading = useContext(HeadingLevel) === 3 ? 'h3' : 'h2'
  return (
    <section data-report-section className="relative min-w-0 border-t border-foreground/70 pt-3">
      {title && (
        <div className="mb-3 flex min-w-0 items-baseline gap-3">
          {n && (
            <span
              aria-hidden="true"
              className="shrink-0 font-mono text-[11px] font-bold tracking-[0.12em] text-accent"
            >
              {n}
            </span>
          )}
          <Heading className="min-w-0 text-[15px] font-bold">{title}</Heading>
        </div>
      )}
      {children}
    </section>
  )
}

/** Un aviso del informe (esperando, sin conexión, sin datos): una nota al margen, no una tarjeta. */
function Note({ title, children }: { title?: ReactNode; children: ReactNode }) {
  const Heading = useContext(HeadingLevel) === 3 ? 'h3' : 'h2'
  return (
    <section className="min-w-0 border-l-2 border-accent py-1 pl-4">
      {title && <Heading className="mb-1 text-[14px] font-bold">{title}</Heading>}
      {children}
    </section>
  )
}

/** Un selector que, si no entra (muchos instrumentos en un celular), se desliza en vez de
 * ensanchar la página. */
function ScrollRow({ children }: { children: ReactNode }) {
  return <div className="max-w-full min-w-0 overflow-x-auto [scrollbar-width:thin]">{children}</div>
}

const INTRO = (
  <p className="max-w-3xl text-[13px] text-muted-foreground">
    Dónde probablemente hay stops (<Term id="liquidez">liquidez</Term>) y qué pasó con cada nivel,
    calculado por el motor con <Term id="vela-cerrada">velas cerradas</Term>. Es una capa{' '}
    <strong>informativa</strong>: todavía no suma al puntaje de las propuestas.
  </p>
)

/** Por qué 1d no se puede elegir con la fuente de futuros. */
const NO_DAILY_FUTURES = { '1d': 'Los futuros no tienen velas diarias.' }

const TF_TEXT: Record<ViewTimeframe, [string, string]> = {
  '15m': ['vela de 15 m', 'velas de 15 m'],
  '1h': ['vela de 1 h', 'velas de 1 h'],
  '1d': ['vela diaria', 'velas diarias'],
}

function Shell({ children, controls }: { children: ReactNode; controls?: ReactNode }) {
  return (
    <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4">
      <div className="flex min-w-0 flex-wrap items-end justify-between gap-3">
        {INTRO}
        {controls}
      </div>
      {children}
    </div>
  )
}

function Waiting() {
  return (
    <Note>
      <p role="status" className="text-[13px]">
        Pidiendo los datos al motor…
      </p>
    </Note>
  )
}

function Unreachable({ message }: { message: string }) {
  return (
    <Note title="No se pudo leer al motor">
      <p role="alert" className="text-[13px]">
        No se pudo conectar con el motor ({message}). Se vuelve a intentar cada minuto.
      </p>
    </Note>
  )
}

/** La vista conectada al motor: instrumento y timeframe salen del gateway (catálogo y config). */
export function LiquidityView({ headingLevel = 2 }: { headingLevel?: 2 | 3 }) {
  return (
    <HeadingLevel.Provider value={headingLevel}>
      <LiveLiquidity />
    </HeadingLevel.Provider>
  )
}

function LiveLiquidity() {
  const catalog = useInstruments()
  const [picked, setPicked] = useState<{
    instrument: string
    timeframe: ViewTimeframe
    source?: SourceChoice
  } | null>(null)
  // Sin elegir, la primera fuente que manda el motor (#628: `auto`, IG o, sin velas, futuros).
  const sel =
    picked ??
    (catalog.status === 'ok'
      ? {
          instrument: catalog.data.default_instrument,
          timeframe: catalog.data.default_timeframe,
          source: catalog.data.sources?.[0]?.id,
        }
      : null)
  const state = useLiquidityView(sel)
  if (catalog.status === 'loading')
    return (
      <Shell>
        <Waiting />
      </Shell>
    )
  if (catalog.status === 'error')
    return (
      <Shell>
        <Unreachable message={catalog.message} />
      </Shell>
    )
  // Las fuentes las manda el motor; sin ellas (gateway anterior) solo existe la del CFD.
  const sources = catalog.data.sources ?? []
  const futuresSel = sel?.source === 'futures'
  const controls = sel && (
    <div className="flex min-w-0 flex-wrap gap-2">
      <ScrollRow>
        <Segmented
          label="Instrumento"
          options={catalog.data.instruments.map((i) => i.id)}
          value={sel.instrument}
          onChange={(instrument) => setPicked({ ...sel, instrument })}
        />
      </ScrollRow>
      <ScrollRow>
        <Segmented
          label="Timeframe"
          options={catalog.data.timeframes}
          value={sel.timeframe}
          onChange={(tf) => setPicked({ ...sel, timeframe: tf as ViewTimeframe })}
          disabled={futuresSel ? NO_DAILY_FUTURES : undefined}
        />
      </ScrollRow>
      {sources.length > 1 && (
        <ScrollRow>
          <Segmented
            label="Fuente"
            options={sources.map((s) => s.label)}
            value={(sources.find((s) => s.id === (sel.source ?? 'ig')) ?? sources[0]).label}
            onChange={(label) => {
              const source = sources.find((s) => s.label === label)?.id ?? 'ig'
              // los futuros no tienen velas diarias: con 1d se pasa al primer timeframe que sí
              const timeframe =
                source === 'futures' && sel.timeframe === '1d'
                  ? (catalog.data.timeframes.find((t) => t !== '1d') ?? sel.timeframe)
                  : sel.timeframe
              setPicked({ ...sel, source, timeframe })
            }}
          />
        </ScrollRow>
      )}
    </div>
  )
  return (
    <Shell controls={controls}>
      {state.status === 'loading' && <Waiting />}
      {state.status === 'error' && <Unreachable message={state.message} />}
      {state.status === 'ok' && <LiquidityContent data={state.data} />}
    </Shell>
  )
}

/** Lo que muestra la vista con una respuesta del gateway (también la usan los tests). */
export function LiquidityContent({ data }: { data: LiquidityViewData }) {
  const tf = data.timeframe ?? '15m'
  const source = data.source?.label ?? 'IG CFD'
  const futures = data.source?.id === 'futures'
  if (data.as_of === null || data.candles.length === 0)
    return (
      <Note title="Sin datos todavía">
        <p className="text-[13px]">
          El motor todavía no tiene {TF_TEXT[tf][1]} de {data.instrument_id} guardadas. Fuente:{' '}
          {source}.{' '}
          {futures && tf === '1d'
            ? 'Los futuros no tienen velas diarias.'
            : 'La vista se llena sola cuando lleguen.'}
        </p>
      </Note>
    )
  const open = data.fvgs.filter((f) => f.status === 'open')
  const closed = data.fvgs.filter((f) => f.status !== 'open')
  const current = data.imbalance.at(-1)
  return (
    <div className="grid gap-6">
      <p className="font-mono text-[11px] text-muted-foreground">
        Datos del motor al {dateTimeUtc(data.as_of)} · {data.candles.length}{' '}
        {TF_TEXT[tf][data.candles.length === 1 ? 0 : 1]} · fuente: <strong>{source}</strong>{' '}
        <StaleNote at={data.as_of} />
      </p>
      {data.fallback ? (
        <p role="note" className="text-[12px]">
          <strong>Fuente automática:</strong> {data.fallback.note}
        </p>
      ) : (
        futures && (
          <p role="note" className="text-[12px] text-muted-foreground">
            Son precios del futuro, no del CFD: la diferencia entre los dos (la base) cambia con
            cada vencimiento, así que los niveles no son precios del CFD.
          </p>
        )
      )}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_300px]">
        {/* el gráfico, en un marco plano de 4 px: es la figura del informe */}
        <figure className="m-0 min-w-0 rounded-[4px] border border-border bg-card p-2">
          <LiquidityChart
            key={`${data.instrument_id}|${tf}`}
            instrument={data.instrument_id}
            timeframe={tf}
            bars={chartBars(data)}
            levels={data.levels}
            fvgs={data.fvgs}
            imbalance={data.imbalance}
            analysis={data.analysis}
            loadOlder={(before) =>
              fetchOlderBars(data.instrument_id, tf, before, data.source?.id ?? 'ig')
            }
          />
        </figure>
        <Card n="01" title="Índice de desequilibrio">
          {current ? (
            <ImbalanceGauge imbalance={current} />
          ) : (
            <p className="text-[12.5px]">Todavía no hay valor (calentamiento).</p>
          )}
        </Card>
      </div>

      <Card
        n="02"
        title={
          <>
            Niveles: intacto, <Term id="sweep">barrido con reversión</Term> o roto
          </>
        }
      >
        {data.levels.length ? (
          <LevelsTable levels={data.levels} />
        ) : (
          <p className="text-[12.5px]">Todavía no hay niveles guardados en esta ventana.</p>
        )}
      </Card>

      <div className="grid gap-6 2xl:grid-cols-2">
        <Card n="03" title={`FVG abiertos (${open.length})`}>
          {open.length ? (
            <FvgTable fvgs={open} label="FVG abiertos" />
          ) : (
            <p className="text-[12.5px]">No hay FVG abiertos.</p>
          )}
        </Card>
        <Card n="04" title={`FVG llenos o vencidos (${closed.length})`}>
          {closed.length ? (
            <FvgTable fvgs={closed} label="FVG llenos o vencidos" />
          ) : (
            <p className="text-[12.5px]">Ningún FVG se llenó todavía.</p>
          )}
        </Card>
      </div>

      <Card n="05" title="Estadísticas">
        {data.stats.length ? (
          <StatsPanel stats={data.stats} />
        ) : (
          <p className="text-[12.5px]">Todavía no hay estadísticas guardadas.</p>
        )}
      </Card>
    </div>
  )
}
