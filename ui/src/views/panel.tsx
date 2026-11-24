/**
 * Pestaña Panel (#286): el panel de siempre (fondo, índices, operaciones y competencia), el
 * contexto de mercado (sentimiento y análogos, #294) y, debajo, la liquidez en vivo que manda el
 * motor. Reúne dos vistas que ya existen; no calcula nada. La sección "Operación" (#314) muestra
 * el estado del broker simulado y sus eventos y alertas.
 * `#liquidez` sigue existiendo sola.
 *
 * La columna es `minmax(0,1fr)` (#357): ningún hijo ancho (una tabla, un selector con muchas
 * opciones) puede ensanchar la página y empujar las tarjetas fuera de la pantalla del celular.
 */
import type { ReactNode } from 'react'

import { OverviewView } from '@/views'
import { LiquidityView } from '@/views/liquidity'
import { MarketContextView } from '@/views/market-context'
import { OperationView } from '@/views/operation'

function PanelSection({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section aria-labelledby={id} className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
      <h2
        id={id}
        className="scroll-mt-24 px-1 text-[clamp(20px,2vw,26px)] font-bold tracking-[-0.03em]"
      >
        {title}
      </h2>
      {children}
    </section>
  )
}

export function PanelView({ onOpen }: { onOpen: (view: string) => void }) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)] gap-8">
      <OverviewView onOpen={onOpen} />
      <PanelSection id="panel-operacion" title="Operación">
        <OperationView />
      </PanelSection>
      <PanelSection id="panel-contexto" title="Contexto de mercado">
        <MarketContextView />
      </PanelSection>
      <PanelSection id="panel-liquidez" title="Liquidez">
        <LiquidityView headingLevel={3} />
      </PanelSection>
    </div>
  )
}
