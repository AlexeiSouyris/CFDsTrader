/**
 * Vista de liquidez en vivo (#231, spec §5.4.1). Solo ubica componentes y les pasa lo que manda el
 * motor: la UI no calcula niveles, estados, FVG, tasas ni el desequilibrio (CLAUDE.md §1).
 *
 * Los datos llegan del gateway (`GET /api/v1/liquidity`, `contracts/liquidity_view.v1`) y se
 * vuelven a pedir cada minuto. Sin velas guardadas muestra "sin datos todavía".
 */
import type { ReactNode } from 'react'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { FvgTable, ImbalanceGauge, LevelsTable, StatsPanel } from '@/components/ui/liquidity-panels'
import { Term } from '@/components/ui/term'
import { useLiquidityView } from '@/lib/liquidity-api'
import type { LiquidityViewData } from '@/lib/liquidity-contracts'
import { dateTimeUtc } from '@/lib/format'

function Card({ title, children }: { title?: ReactNode; children: ReactNode }) {
  return (
    <section className="glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30">
      {title && <h2 className="mb-3 text-[14px] font-bold">{title}</h2>}
      {children}
    </section>
  )
}

/** Instrumento que muestra la vista mientras no haya selector (el gateway valida el catálogo). */
export const DEFAULT_INSTRUMENT = 'US100'

const INTRO = (
  <p className="max-w-3xl text-[13px] text-muted-foreground">
    Dónde probablemente hay stops (<Term id="liquidez">liquidez</Term>) y qué pasó con cada nivel,
    calculado por el motor con <Term id="vela-cerrada">velas cerradas</Term>. Es una capa{' '}
    <strong>informativa</strong>: todavía no suma al puntaje de las propuestas.
  </p>
)

/** La vista conectada al motor. */
export function LiquidityView({ instrument = DEFAULT_INSTRUMENT }: { instrument?: string }) {
  const state = useLiquidityView(instrument)
  if (state.status === 'loading')
    return (
      <div className="grid gap-3">
        {INTRO}
        <Card>
          <p role="status" className="text-[13px]">
            Pidiendo los datos al motor…
          </p>
        </Card>
      </div>
    )
  if (state.status === 'error')
    return (
      <div className="grid gap-3">
        {INTRO}
        <Card title="No se pudo leer al motor">
          <p role="alert" className="text-[13px]">
            El gateway no respondió ({state.message}). Para verlo en vivo hay que levantarlo con{' '}
            <code>cargo run -p cfds-gateway</code>. Se vuelve a intentar cada minuto.
          </p>
        </Card>
      </div>
    )
  return <LiquidityContent data={state.data} />
}

/** Lo que muestra la vista con una respuesta del gateway (también la usan los tests). */
export function LiquidityContent({ data }: { data: LiquidityViewData }) {
  if (data.as_of === null || data.candles.length === 0)
    return (
      <div className="grid gap-3">
        {INTRO}
        <Card title="Sin datos todavía">
          <p className="text-[13px]">
            El motor todavía no tiene velas de 15 m de {data.instrument_id} guardadas. La vista se
            llena sola cuando lleguen.
          </p>
        </Card>
      </div>
    )
  const open = data.fvgs.filter((f) => f.status === 'open')
  const closed = data.fvgs.filter((f) => f.status !== 'open')
  const current = data.imbalance.at(-1)
  return (
    <div className="grid gap-3">
      {INTRO}
      <p className="text-[11.5px] text-muted-foreground">
        Datos del motor al {dateTimeUtc(data.as_of)} · {data.candles.length}{' '}
        {data.candles.length === 1 ? 'vela' : 'velas'} de 15 m
      </p>

      <div className="grid gap-3 xl:grid-cols-[1fr_320px]">
        <Card>
          <LiquidityChart
            instrument={data.instrument_id}
            bars={data.candles}
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
