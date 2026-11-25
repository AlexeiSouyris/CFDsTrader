/**
 * El gráfico de un lugar de una grilla (#286, #540): el `LiquidityChart` con las velas que eligió
 * `lib/chart-slots.ts`, o el aviso de que no hay velas. Lo comparten la Terminal y el
 * Multigráfico. Las velas, niveles, FVG y el bloque `analysis` llegan del motor
 * (`liquidity_view.v1`); acá no se calcula nada.
 */
import type { ChartMode } from '@/components/ui/chart-measure'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import type { SlotData } from '@/lib/chart-slots'
import { fetchOlderBars } from '@/lib/liquidity-api'
import { chartBars } from '@/lib/liquidity-bars'
import { CHART_SIGNALS } from '@/mocks'

/** El gráfico de un lugar, o el aviso mientras se pide o si no hay velas. */
export function SlotChart({
  slot,
  height,
  ruler,
  onRuler,
  mode,
  onMode,
}: {
  slot: SlotData
  height: number
  /** herramientas manejadas desde afuera (la barra del Multigráfico); sin esto, las del gráfico */
  ruler?: boolean
  onRuler?: (on: boolean) => void
  mode?: ChartMode
  onMode?: (m: ChartMode) => void
}) {
  const { sel, data, live } = slot
  if (!data)
    return (
      <p
        role="status"
        className="grid place-items-center rounded-xl border border-dashed border-border px-4 text-center text-[12.5px] text-muted-foreground"
        style={{ minHeight: height }}
      >
        {slot.loading
          ? `Pidiendo ${sel.instrument} (${sel.timeframe}) al motor…`
          : `Sin velas de ${sel.instrument} en ${sel.timeframe}: el gateway no respondió o todavía no las tiene.`}
      </p>
    )
  return (
    <LiquidityChart
      key={`${sel.instrument}|${sel.timeframe}`}
      instrument={data.instrument_id}
      timeframe={data.timeframe ?? sel.timeframe}
      bars={chartBars(data)}
      levels={data.levels}
      fvgs={data.fvgs}
      imbalance={data.imbalance}
      signals={live ? [] : CHART_SIGNALS}
      analysis={data.analysis}
      loadOlder={
        live
          ? (before) => fetchOlderBars(live.instrument_id, live.timeframe ?? sel.timeframe, before)
          : undefined
      }
      height={height}
      ruler={ruler}
      onRulerChange={onRuler}
      mode={mode}
      onModeChange={onMode}
    />
  )
}
