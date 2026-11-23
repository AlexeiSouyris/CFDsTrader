/**
 * Fases 8 y 8b en Performance (#331): "Paper vs backtest" y "Aprendizaje", de solo lectura, tal
 * como los manda el gateway. Sin gateway no hay datos de ejemplo: se dice. No se vota desde la UI.
 */
import type { ReactNode } from 'react'

import { Term } from '@/components/ui/term'
import { LearningReportView } from '@/components/ui/learning-report'
import { PaperVsBacktest } from '@/components/ui/paper-vs-backtest'
import {
  useLearningReport,
  usePaperComparisons,
  useVoteStatuses,
  voteLabel,
} from '@/lib/learning-api'

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
      <div className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
        <h3 id={id} className="text-[14px] font-bold">
          {title}
        </h3>
        <span className="text-[11px] text-muted-foreground">{aside}</span>
      </div>
      {children}
    </section>
  )
}

const note = (text: string) => (
  <p data-phase8-note className="text-[12.5px] text-muted-foreground">
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
      <p className="mb-3 max-w-3xl text-[12px] text-muted-foreground">
        El broker simulado contra el backtest del mismo período, con las mismas velas y la misma
        configuración. La diferencia se explica por causa; no es una promesa de resultados.
      </p>
      {r.status === 'loading' ? (
        note('Pidiendo las comparaciones al motor…')
      ) : r.status === 'error' ? (
        note('Sin conexión con el gateway: no hay comparaciones para mostrar.')
      ) : r.data.length === 0 ? (
        note('Todavía no hay comparaciones guardadas.')
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
        note('Pidiendo el informe al motor…')
      ) : r.status === 'error' ? (
        note('Sin conexión con el gateway: no hay informe para mostrar.')
      ) : r.data === null ? (
        note('Todavía no hay un informe de aprendizaje guardado.')
      ) : (
        <LearningReportView report={r.data} voteStatus={statusOf} />
      )}
    </Section>
  )
}
