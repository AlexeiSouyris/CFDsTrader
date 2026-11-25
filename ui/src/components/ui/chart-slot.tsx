/**
 * El gráfico de un lugar de una grilla (#286, #540): el `LiquidityChart` con las velas que eligió
 * `lib/chart-slots.ts`, o el aviso de que no hay velas. Lo comparten la Terminal y el
 * Multigráfico. Las velas, niveles, FVG y el bloque `analysis` llegan del motor
 * (`liquidity_view.v1`); acá no se calcula nada. Las velas de IBKR (#556) se ven sin capas: el
 * motor no manda niveles ni FVG para ellas y no se inventan.
 */
import type { ChartMode } from '@/components/ui/chart-measure'
import { ExampleNotice } from '@/components/ui/data-honesty'
import type { DrawTool } from '@/lib/chart-drawings'
import type { CursorSync } from '@/lib/cursor-sync'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import type { SlotData } from '@/lib/chart-slots'
import { CHART_SIGNALS } from '@/mocks'

/** El gráfico de un lugar, o el aviso mientras se pide o si no hay velas. */
export function SlotChart({
  slot,
  height,
  ruler,
  onRuler,
  mode,
  onMode,
  compact,
  controlsTarget,
  draw,
  onDraw,
  syncCursor,
  ohlc,
}: {
  slot: SlotData
  height: number
  /** herramientas manejadas desde afuera (la barra del Multigráfico); sin esto, las del gráfico */
  ruler?: boolean
  onRuler?: (on: boolean) => void
  mode?: ChartMode
  onMode?: (m: ChartMode) => void
  /** modo compacto del Multigráfico (#545): ver `LiquidityChart` */
  compact?: boolean
  controlsTarget?: HTMLElement | null
  /** dibujos del usuario (#543): ver `LiquidityChart`; sin esto, no hay capa de dibujos */
  draw?: DrawTool
  onDraw?: (t: DrawTool) => void
  /** cursor sincronizado del Multigráfico (#544): ver `LiquidityChart` */
  syncCursor?: CursorSync
  /** OHLC de la vela bajo el cursor en la cabecera (#581): ver `LiquidityChart` */
  ohlc?: boolean
}) {
  const { sel, view, ibkr } = slot
  const ibkrSel = sel.feed === 'ibkr'
  if (slot.bars.length === 0)
    return (
      <p
        role="status"
        className="grid place-items-center rounded-xl border border-dashed border-border px-4 text-center text-[12.5px] text-muted-foreground"
        style={{ minHeight: height }}
      >
        {slot.loading
          ? `Pidiendo ${sel.instrument} (${sel.timeframe}) ${ibkrSel ? 'de IBKR' : 'al motor'}…`
          : slot.offline
            ? `Sin velas de ${sel.instrument} en ${sel.timeframe}: el gateway no respondió.`
            : ibkrSel
              ? `IBKR todavía no tiene velas de ${sel.instrument} en ${sel.timeframe}.`
              : `El motor todavía no tiene velas de ${sel.instrument} en ${sel.timeframe}.`}
      </p>
    )
  const chart = (
    <LiquidityChart
      key={`${sel.feed ?? ''}|${sel.instrument}|${sel.timeframe}`}
      instrument={ibkr?.instrument_id ?? view?.instrument_id ?? sel.instrument}
      timeframe={ibkr?.timeframe ?? view?.timeframe ?? sel.timeframe}
      caption={ibkr?.series ? `IBKR · ${ibkr.series.contract} · solo datos` : undefined}
      liquidity={!ibkrSel}
      bars={slot.bars}
      forming={slot.forming ?? undefined}
      levels={view?.levels ?? []}
      fvgs={view?.fvgs ?? []}
      imbalance={view?.imbalance ?? []}
      signals={slot.source === 'ejemplo' ? CHART_SIGNALS : []}
      analysis={view?.analysis}
      loadOlder={slot.loadOlder}
      height={height}
      ruler={ruler}
      onRulerChange={onRuler}
      mode={mode}
      onModeChange={onMode}
      compact={compact}
      controlsTarget={controlsTarget}
      drawTool={draw}
      onDrawToolChange={onDraw}
      syncCursor={syncCursor}
      ohlc={ohlc}
    />
  )
  if (slot.source !== 'ejemplo') return chart
  // ejemplo solo sin gateway (#577): con el sello a la vista, nunca como si fuera del motor
  return (
    <div className="grid gap-2" data-example>
      <ExampleNotice>Velas inventadas: el gateway no respondió.</ExampleNotice>
      {chart}
    </div>
  )
}
