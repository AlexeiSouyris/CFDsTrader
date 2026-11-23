/**
 * Pestaña Panel (#286): el panel de siempre (fondo, índices, operaciones y competencia), el
 * contexto de mercado (sentimiento y análogos, #294) y, debajo, la liquidez en vivo que manda el
 * motor. Reúne dos vistas que ya existen; no calcula nada. La sección "Operación" (#314) muestra
 * el estado del broker simulado y sus eventos y alertas.
 * `#liquidez` sigue existiendo sola.
 */
import { OverviewView } from '@/views'
import { LiquidityView } from '@/views/liquidity'
import { MarketContextView } from '@/views/market-context'
import { OperationView } from '@/views/operation'

export function PanelView({ onOpen }: { onOpen: (view: string) => void }) {
  return (
    <div className="grid gap-8">
      <OverviewView onOpen={onOpen} />
      <section aria-labelledby="panel-operacion" className="grid gap-3">
        <h2
          id="panel-operacion"
          className="px-1 text-[clamp(20px,2vw,26px)] font-bold tracking-[-0.03em]"
        >
          Operación
        </h2>
        <OperationView />
      </section>
      <section aria-labelledby="panel-contexto" className="grid gap-3">
        <h2
          id="panel-contexto"
          className="px-1 text-[clamp(20px,2vw,26px)] font-bold tracking-[-0.03em]"
        >
          Contexto de mercado
        </h2>
        <MarketContextView />
      </section>
      <section aria-labelledby="panel-liquidez" className="grid gap-3">
        <h2
          id="panel-liquidez"
          className="px-1 text-[clamp(20px,2vw,26px)] font-bold tracking-[-0.03em]"
        >
          Liquidez
        </h2>
        <LiquidityView />
      </section>
    </div>
  )
}
