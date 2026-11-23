/**
 * Vista de liquidez en vivo (#231, spec §5.4.1). Solo ubica componentes y les pasa lo que manda el
 * motor: la UI no calcula niveles, estados, FVG, tasas ni el desequilibrio (CLAUDE.md §1).
 *
 * Los datos llegan del gateway: los instrumentos y timeframes de `GET /api/v1/instruments` y la
 * vista de `GET /api/v1/liquidity` (`contracts/liquidity_view.v1`), que se vuelve a pedir cada
 * minuto. Sin velas guardadas muestra "sin datos todavía".
 */
import { useState, type ReactNode } from 'react'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { FvgTable, ImbalanceGauge, LevelsTable, StatsPanel } from '@/components/ui/liquidity-panels'
import { Segmented } from '@/components/ui/segmented'
import { Term } from '@/components/ui/term'
import { useInstruments, useLiquidityView } from '@/lib/liquidity-api'
import { chartBars } from '@/lib/liquidity-bars'
import type { LiquidityViewData, ViewTimeframe } from '@/lib/liquidity-contracts'
import { dateTimeUtc } from '@/lib/format'

function Card({ title, children }: { title?: ReactNode; children: ReactNode }) {
  return (
    <section className="glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30">
      {title && <h2 className="mb-3 text-[14px] font-bold">{title}</h2>}
      {children}
    </section>
  )
}

const INTRO = (
  <p className="max-w-3xl text-[13px] text-muted-foreground">
    Dónde probablemente hay stops (<Term id="liquidez">liquidez</Term>) y qué pasó con cada nivel,
    calculado por el motor con <Term id="vela-cerrada">velas cerradas</Term>. Es una capa{' '}
    <strong>informativa</strong>: todavía no suma al puntaje de las propuestas.
  </p>
)

const TF_TEXT: Record<ViewTimeframe, [string, string]> = {
  '15m': ['vela de 15 m', 'velas de 15 m'],
  '1h': ['vela de 1 h', 'velas de 1 h'],
  '1d': ['vela diaria', 'velas diarias'],
}

function Shell({ children, controls }: { children: ReactNode; controls?: ReactNode }) {
  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-end justify-between gap-3">
        {INTRO}
        {controls}
      </div>
      {children}
    </div>
  )
}

function Waiting() {
  return (
    <Card>
      <p role="status" className="text-[13px]">
        Pidiendo los datos al motor…
      </p>
    </Card>
  )
}

function Unreachable({ message }: { message: string }) {
  return (
    <Card title="No se pudo leer al motor">
      <p role="alert" className="text-[13px]">
        El gateway no respondió ({message}). Para verlo en vivo hay que levantarlo con{' '}
        <code>infra/scripts/ui_liquidez.sh</code>. Se vuelve a intentar cada minuto.
      </p>
    </Card>
  )
}

/** La vista conectada al motor: instrumento y timeframe salen del gateway (catálogo y config). */
export function LiquidityView() {
  const catalog = useInstruments()
  const [picked, setPicked] = useState<{ instrument: string; timeframe: ViewTimeframe } | null>(
    null,
  )
  const sel =
    picked ??
    (catalog.status === 'ok'
      ? { instrument: catalog.data.default_instrument, timeframe: catalog.data.default_timeframe }
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
  const controls = sel && (
    <div className="flex flex-wrap gap-2">
      <Segmented
        label="Instrumento"
        options={catalog.data.instruments.map((i) => i.id)}
        value={sel.instrument}
        onChange={(instrument) => setPicked({ ...sel, instrument })}
      />
      <Segmented
        label="Timeframe"
        options={catalog.data.timeframes}
        value={sel.timeframe}
        onChange={(tf) => setPicked({ ...sel, timeframe: tf as ViewTimeframe })}
      />
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
  if (data.as_of === null || data.candles.length === 0)
    return (
      <Card title="Sin datos todavía">
        <p className="text-[13px]">
          El motor todavía no tiene {TF_TEXT[tf][1]} de {data.instrument_id} guardadas. La vista se
          llena sola cuando lleguen.
        </p>
      </Card>
    )
  const open = data.fvgs.filter((f) => f.status === 'open')
  const closed = data.fvgs.filter((f) => f.status !== 'open')
  const current = data.imbalance.at(-1)
  return (
    <div className="grid gap-3">
      <p className="text-[11.5px] text-muted-foreground">
        Datos del motor al {dateTimeUtc(data.as_of)} · {data.candles.length}{' '}
        {TF_TEXT[tf][data.candles.length === 1 ? 0 : 1]}
      </p>

      <div className="grid gap-3 xl:grid-cols-[1fr_320px]">
        <Card>
          <LiquidityChart
            key={`${data.instrument_id}|${tf}`}
            instrument={data.instrument_id}
            timeframe={tf}
            bars={chartBars(data)}
            levels={data.levels}
            fvgs={data.fvgs}
            imbalance={data.imbalance}
          />
        </Card>
        <Card title="Índice de desequilibrio">
          {current ? (
            <ImbalanceGauge imbalance={current} />
          ) : (
            <p className="text-[12.5px]">Todavía no hay valor (calentamiento).</p>
          )}
        </Card>
      </div>

      <Card
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

      <div className="grid gap-3 2xl:grid-cols-2">
        <Card title={`FVG abiertos (${open.length})`}>
          {open.length ? (
            <FvgTable fvgs={open} />
          ) : (
            <p className="text-[12.5px]">No hay FVG abiertos.</p>
          )}
        </Card>
        <Card title={`FVG llenos o vencidos (${closed.length})`}>
          {closed.length ? (
            <FvgTable fvgs={closed} />
          ) : (
            <p className="text-[12.5px]">Ningún FVG se llenó todavía.</p>
          )}
        </Card>
      </div>

      <Card title="Estadísticas">
        {data.stats.length ? (
          <StatsPanel stats={data.stats} />
        ) : (
          <p className="text-[12.5px]">Todavía no hay estadísticas guardadas.</p>
        )}
      </Card>
    </div>
  )
}
