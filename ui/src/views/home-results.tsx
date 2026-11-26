/**
 * Resultados recientes del Inicio (#581): las últimas filas del registro de resultados y las
 * últimas corridas del motor, tal como llegan, con el aviso "resultados del pasado, no promesas".
 * El selector de estrategia solo filtra lo que se ve (presentación, no cálculo): qué estrategia
 * corre en simulación lo decide y lo valida el motor.
 */
import { motion, useReducedMotion } from 'motion/react'
import { useId, useState } from 'react'

import { ExampleBadge } from '@/components/ui/data-honesty'
import { MetricBento, type Metric } from '@/components/ui/metric-bento'
import { ToolLock } from '@/components/ui/tool-lock'
import { WorkspacePanel } from '@/components/ui/workspace-panel'
import type { BacktestRun } from '@/lib/backtest-contracts'
import { REGISTRY, registryPrograms, type RegistryRow } from '@/lib/results-registry'
import { dayYear, signedPct, tone } from '@/lib/run-format'
import { cn } from '@/lib/utils'
import { FKEYS } from '@/views/nav'

const ALL = 'Todas'
const STRATEGY_KEY = 'cfds.home.strategy'
const savedStrategy = () => {
  try {
    return localStorage.getItem(STRATEGY_KEY) ?? ALL
  } catch {
    return ALL
  }
}

export function RecentResults({
  runs,
  runsLive,
  onOpen,
}: {
  runs: BacktestRun[]
  runsLive: boolean
  onOpen: (view: string) => void
}) {
  const programs = registryPrograms(REGISTRY)
  const [strategy, setStrategy] = useState(() => {
    const s = savedStrategy()
    return programs.includes(s) ? s : ALL
  })
  const onStrategy = (s: string) => {
    setStrategy(s)
    try {
      localStorage.setItem(STRATEGY_KEY, s)
    } catch {
      // sin almacenamiento: vale para esta visita
    }
  }
  const selectId = useId()
  const reduce = useReducedMotion()
  const rows = REGISTRY.filter((r) => strategy === ALL || r.programa === strategy)
  // el registro va de la fila más vieja a la más nueva: se muestran las últimas, la nueva arriba
  const latest = rows.slice(-5).reverse()
  const lastRow: RegistryRow | undefined = latest[0]
  const lastRun = runs[0]
  const metrics: Metric[] = [
    {
      id: 'registro',
      label: 'Pruebas en el registro',
      value: rows.length,
      detail: strategy === ALL ? 'todas las estrategias' : strategy,
    },
    {
      id: 'ultima-prueba',
      label: 'Última prueba',
      value: lastRow ? `${lastRow.id} · ${lastRow.veredicto}` : '—',
      detail: lastRow ? `${lastRow.fecha} · ${lastRow.instrumentos}` : undefined,
      wide: true,
    },
    {
      id: 'ultima-corrida',
      label: 'Última corrida del motor',
      value: lastRun ? (
        <span className={tone(lastRun.metrics.total_return)}>
          {signedPct(lastRun.metrics.total_return)}
        </span>
      ) : (
        '—'
      ),
      detail: lastRun
        ? `${lastRun.instruments.join(', ')} ${lastRun.timeframe} · ${dayYear(lastRun.finished_at)}`
        : undefined,
      example: !runsLive,
    },
  ]

  return (
    <WorkspacePanel
      id="inicio-resultados"
      title="Resultados recientes"
      open={{ label: 'Investigación', view: 'investigacion', hotkey: FKEYS.investigacion, onOpen }}
    >
      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-2 py-1">
        <p className="text-[11px] text-muted-foreground">
          Resultados del pasado, no promesas. Tal como están en el registro y como los manda el
          motor.
        </p>
        {/*
         * Elegir la estrategia en pantalla (#645, patrón *Glass Radio Group* de 21st.dev,
         * https://21st.dev/@ravikatiyar162/components/glass-radio-group, reescrito; liso desde
         * #708): un selector segmentado con radios de verdad (flechas del teclado y lector de
         * pantalla), en una fila que se desliza si no entra. Solo filtra lo que se ve.
         */}
        <fieldset className="min-w-0 rounded-sm border border-border">
          <legend className="sr-only">Estrategia en pantalla</legend>
          <div
            role="presentation"
            className="flex min-w-0 divide-x divide-border overflow-x-auto [mask-image:linear-gradient(to_right,#000_calc(100%-2.5rem),transparent)] pr-8 [scrollbar-width:none]"
          >
            {[ALL, ...programs].map((p) => {
              const on = p === strategy
              return (
                <label
                  key={p}
                  className={cn(
                    'relative flex min-h-8 shrink-0 cursor-pointer items-center px-3 text-[12px] font-semibold whitespace-nowrap has-[:focus-visible]:outline-2 has-[:focus-visible]:-outline-offset-2 has-[:focus-visible]:outline-ring',
                    on
                      ? 'bg-surface text-foreground shadow-[inset_0_-2px_0_var(--primary)]'
                      : 'text-muted-foreground hover:text-foreground',
                  )}
                >
                  <input
                    type="radio"
                    name={selectId}
                    value={p}
                    checked={on}
                    onChange={(e) => {
                      onStrategy(p)
                      // con las flechas, la elegida entra en la fila visible
                      e.currentTarget.parentElement?.scrollIntoView?.({
                        block: 'nearest',
                        inline: 'nearest',
                      })
                    }}
                    className="sr-only"
                  />
                  {p}
                </label>
              )
            })}
          </div>
        </fieldset>
        <p className="text-[11px] text-muted-foreground">
          El selector solo filtra lo que se ve. Qué estrategia corre en simulación lo decide y lo
          valida el motor.
        </p>

        <MetricBento label="Resumen de resultados" metrics={metrics} />

        <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-x-4 gap-y-3 border-t border-border pt-2 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)]">
          <div className="min-w-0">
            <h3 className="mb-1.5 text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
              Registro de resultados
            </h3>
            {/* candado del registro (#846, lección 9.5) */}
            <ToolLock tool="tool.results_registry">
              <ol aria-label="Últimas pruebas del registro" className="grid gap-2">
                {latest.map((r) => (
                  <motion.li
                    key={r.id}
                    // cada fila nueva del filtro entra suave (las que salen se van sin animación)
                    initial={reduce ? false : { opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="grid min-w-0 gap-0.5 border-t border-border pt-2 first:border-t-0 first:pt-0"
                  >
                    <span className="flex flex-wrap items-baseline justify-between gap-x-3 text-[11px] text-muted-foreground">
                      <span className="font-mono">
                        {r.id} · {r.fecha} · {r.instrumentos}
                      </span>
                      <span>{r.issue_pr}</span>
                    </span>
                    <span className="text-[12.5px] leading-snug">{r.prueba}</span>
                    <span className="text-[12px] font-bold">{r.veredicto}</span>
                  </motion.li>
                ))}
              </ol>
              {latest.length === 0 && (
                <p className="text-[12px] text-muted-foreground">
                  No hay pruebas de esa estrategia.
                </p>
              )}
            </ToolLock>
          </div>

          <div className="min-w-0 lg:border-l lg:border-border lg:pl-4">
            <h3 className="mb-1.5 flex flex-wrap items-center justify-between gap-2 text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
              Corridas del motor
              {!runsLive && <ExampleBadge />}
            </h3>
            <p className="mb-2 text-[11px] text-muted-foreground">
              Las corridas no dicen la estrategia: se ven todas, en el orden en que llegan.
            </p>
            <ol aria-label="Últimas corridas del motor" className="grid gap-2">
              {runs.slice(0, 4).map((run) => (
                <li
                  key={run.run_id}
                  className="flex min-w-0 flex-wrap items-baseline justify-between gap-x-3 border-t border-border pt-2 text-[12px] first:border-t-0 first:pt-0"
                >
                  <span className="min-w-0">
                    <span className="font-bold">
                      {run.instruments.join(', ')} {run.timeframe}
                    </span>
                    <span className="block text-[11px] text-muted-foreground">
                      {dayYear(run.from)} – {dayYear(run.to)} · {run.metrics.trades} operaciones
                    </span>
                  </span>
                  <span
                    className={`font-mono tabular-nums ${tone(run.metrics.total_return) ?? ''}`}
                  >
                    {signedPct(run.metrics.total_return)}
                  </span>
                </li>
              ))}
            </ol>
            <button
              type="button"
              onClick={() => onOpen('performance')}
              className="mt-3 inline-flex min-h-6 items-center text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
            >
              Ver todas en Investigación →
            </button>
          </div>
        </div>
      </div>
    </WorkspacePanel>
  )
}
