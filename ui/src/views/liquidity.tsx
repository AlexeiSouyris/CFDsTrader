/**
 * Vista de liquidez en vivo (#231, spec §5.4.1). Solo ubica componentes y les pasa lo que manda el
 * motor: la UI no calcula niveles, estados, FVG, tasas ni el desequilibrio (CLAUDE.md §1).
 *
 * TODO(#231): hoy lee `LIQUIDITY_US100` de `mocks/`. Cuando estén en `main` los contratos
 * `liquidity_level.v1`, `fvg_event.v1` y `liquidity_stats.v1` y el `gateway` los publique, cambiar
 * la fuente por la suscripción en vivo, sin tocar los componentes.
 */
import type { ReactNode } from 'react'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { FvgTable, ImbalanceGauge, LevelsTable, StatsPanel } from '@/components/ui/liquidity-panels'
import { Term } from '@/components/ui/term'
import { LIQUIDITY_US100, type LiquiditySnapshot } from '@/mocks'

function Card({ title, children }: { title?: ReactNode; children: ReactNode }) {
  return (
    <section className="glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30">
      {title && <h2 className="mb-3 text-[14px] font-bold">{title}</h2>}
      {children}
    </section>
  )
}

export function LiquidityView({ data = LIQUIDITY_US100 }: { data?: LiquiditySnapshot }) {
  const open = data.fvgs.filter((f) => f.status === 'open')
  const closed = data.fvgs.filter((f) => f.status !== 'open')
  return (
    <div className="grid gap-3">
      <p className="max-w-3xl text-[13px] text-muted-foreground">
        Dónde probablemente hay stops (<Term id="liquidez">liquidez</Term>) y qué pasó con cada
        nivel, calculado por el motor con <Term id="vela-cerrada">velas cerradas</Term>. Es una capa{' '}
        <strong>informativa</strong>: todavía no suma al puntaje de las propuestas.
      </p>

      <div className="grid gap-3 xl:grid-cols-[1fr_320px]">
        <Card>
          <LiquidityChart
            instrument={data.instrument_id}
            bars={data.candles}
            levels={data.levels}
            fvgs={data.fvgs}
            displacement={data.displacement}
          />
        </Card>
        <Card title="Índice de desequilibrio">
          <ImbalanceGauge imbalance={data.imbalance} />
        </Card>
      </div>

      <Card
        title={
          <>
            Niveles: intacto, <Term id="sweep">barrido con reversión</Term> o roto
          </>
        }
      >
        <LevelsTable levels={data.levels} />
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
        <StatsPanel stats={data.stats} />
      </Card>
    </div>
  )
}
