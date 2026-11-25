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
import type { BacktestRun } from '@/lib/backtest-contracts'
import { REGISTRY, registryPrograms, type RegistryRow } from '@/lib/results-registry'
import { dayYear, signedPct, tone } from '@/lib/run-format'

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
    <section
      aria-labelledby="inicio-resultados"
      className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3"
    >
      <div className="flex flex-wrap items-end justify-between gap-3 px-1">
        <div>
          <h2
            id="inicio-resultados"
            className="text-[clamp(20px,2vw,26px)] font-bold tracking-[-0.03em]"
          >
            Resultados recientes
          </h2>
          <p className="text-[12px] text-muted-foreground">
            Resultados del pasado, no promesas. Tal como están en el registro y como los manda el
            motor.
          </p>
        </div>
        <label
          htmlFor={selectId}
          className="grid gap-1 text-[11px] font-bold text-muted-foreground"
        >
          Estrategia en pantalla
          <select
            id={selectId}
            value={strategy}
            onChange={(e) => onStrategy(e.target.value)}
            className="glass-strong max-w-[min(20rem,85vw)] rounded-full px-3 py-2 text-[13px] font-semibold text-foreground"
          >
            <option value={ALL}>{ALL}</option>
            {programs.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        </label>
      </div>
      <p className="px-1 text-[11px] text-muted-foreground">
        El selector solo filtra lo que se ve. Qué estrategia corre en simulación lo decide y lo
        valida el motor.
      </p>

      <MetricBento label="Resumen de resultados" metrics={metrics} />

      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)]">
        <div className="glass min-w-0 rounded-[20px] p-4">
          <h3 className="mb-2 text-[13px] font-bold">Registro de resultados</h3>
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
            <p className="text-[12px] text-muted-foreground">No hay pruebas de esa estrategia.</p>
          )}
        </div>

        <div className="glass min-w-0 rounded-[20px] p-4">
          <h3 className="mb-2 flex flex-wrap items-center justify-between gap-2 text-[13px] font-bold">
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
                <span className={`font-mono tabular-nums ${tone(run.metrics.total_return) ?? ''}`}>
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
    </section>
  )
}
