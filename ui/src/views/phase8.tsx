/**
 * Fases 8 y 8b en Performance (#331): "Paper vs backtest" y "Aprendizaje", de solo lectura, tal
 * como los manda el gateway. Sin gateway no hay datos de ejemplo: se dice. No se vota desde la UI.
 * También "Modelo (informativo)", los experimentos de ML de la fase 4b (#426), con la misma forma.
 */
import type { ReactNode } from 'react'

import { Term } from '@/components/ui/term'
import { LearningReportView } from '@/components/ui/learning-report'
import { MlExperiments } from '@/components/ui/ml-experiments'
import { PaperVsBacktest } from '@/components/ui/paper-vs-backtest'
import {
  useLearningReport,
  usePaperComparisons,
  useVoteStatuses,
  voteLabel,
} from '@/lib/learning-api'
import { useMlExperiments } from '@/lib/ml-api'
import { cn } from '@/lib/utils'

function Section({
  id,
  title,
  aside,
  children,
}: {
  id: string
  title: ReactNode
  aside: string
  children: ReactNode
}) {
  return (
    <section
      aria-labelledby={id}
      className="glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30"
    >
      <header className="mb-3 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <h2 id={id} className="text-[14px] font-bold">
          {title}
        </h2>
        <span className="text-[11px] text-muted-foreground">{aside}</span>
      </header>
      {children}
    </section>
  )
}

/** Aviso de carga, error o vacío: el de carga se anuncia y los otros dicen qué falta. */
const note = (text: string, kind: 'loading' | 'error' | 'empty') => (
  <p
    data-phase8-note={kind}
    role={kind === 'loading' ? 'status' : undefined}
    className={cn(
      'rounded-xl border border-dashed border-border px-3 py-2.5 text-[12.5px] text-muted-foreground',
      kind === 'loading' && 'motion-safe:animate-pulse',
    )}
  >
    {text}
  </p>
)

export function PaperVsBacktestSection() {
  const r = usePaperComparisons()
  return (
    <Section
      id="perf-paper-vs-backtest"
      title={
        <>
          <Term id="paper-trading">Paper</Term> vs backtest
        </>
      }
      aside="fase 8 · simulación, del motor"
    >
      <p className="mb-3 max-w-3xl text-[12px] text-pretty text-muted-foreground">
        El broker simulado contra el backtest del mismo período, con las mismas velas y la misma
        configuración. La diferencia se explica por causa; no es una promesa de resultados.
      </p>
      {r.status === 'loading' ? (
        note('Pidiendo las comparaciones al motor…', 'loading')
      ) : r.status === 'error' ? (
        note('Sin conexión con el gateway: no hay comparaciones para mostrar.', 'error')
      ) : r.data.length === 0 ? (
        note('Todavía no hay comparaciones guardadas.', 'empty')
      ) : (
        <PaperVsBacktest comparisons={r.data} />
      )}
    </Section>
  )
}

export function LearningSection() {
  const r = useLearningReport()
  // El estado de cada propuesta, contado por el motor (#331). Sin él, "pendiente de voto".
  const votes = useVoteStatuses()
  const statusOf = (changeId: string) => {
    const s = votes.status === 'ok' ? votes.data.find((v) => v.change_id === changeId) : undefined
    return s && voteLabel(s)
  }
  return (
    <Section id="perf-aprendizaje" title="Aprendizaje" aside="fase 8b · informe del diario">
      {r.status === 'loading' ? (
        note('Pidiendo el informe al motor…', 'loading')
      ) : r.status === 'error' ? (
        note('Sin conexión con el gateway: no hay informe para mostrar.', 'error')
      ) : r.data === null ? (
        note('Todavía no hay un informe de aprendizaje guardado.', 'empty')
      ) : (
        <LearningReportView report={r.data} voteStatus={statusOf} />
      )}
    </Section>
  )
}

export function MlExperimentsSection() {
  const r = useMlExperiments()
  return (
    <Section id="perf-modelo" title="Modelo (informativo)" aside="fase 4b · experimentos de ML">
      <p className="mb-3 max-w-3xl text-[12px] text-pretty text-muted-foreground">
        El modelo de ML v1 (M1) se compara contra lo que se muestra hoy (B1) con datos que no vio.
        Es <strong className="text-foreground">solo informativo</strong>: no decide operaciones y no
        promete resultados. El último experimento de cada instrumento, tal como lo guardó el motor.
      </p>
      {r.status === 'loading' ? (
        note('Pidiendo los experimentos al motor…', 'loading')
      ) : r.status === 'error' ? (
        note('Sin conexión con el gateway: no hay experimentos para mostrar.', 'error')
      ) : r.data.length === 0 ? (
        note('Todavía no hay experimentos de ML guardados.', 'empty')
      ) : (
        <MlExperiments entries={r.data} />
      )}
    </Section>
  )
}
