/**
 * Pestaña Panel (#286): el panel de siempre (fondo, índices, operaciones y competencia) y, debajo,
 * la liquidez en vivo que manda el motor. Reúne dos vistas que ya existen; no calcula nada.
 * `#liquidez` sigue existiendo sola.
 */
import { OverviewView } from '@/views'
import { LiquidityView } from '@/views/liquidity'

export function PanelView({ onOpen }: { onOpen: (view: string) => void }) {
  return (
    <div className="grid gap-8">
      <OverviewView onOpen={onOpen} />
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
