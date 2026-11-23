/**
 * Contexto de mercado del Panel (#294), solo lectura: el índice de sentimiento de EE. UU. y los
 * análogos históricos, tal como los calculó Python y los sirve el gateway. Los instrumentos salen
 * de lo que hay guardado (nada fijo en el código). No calcula nada.
 */
import { useState, type ReactNode } from 'react'

import {
  AnalogDistribution,
  SentimentGauge,
  SentimentHistory,
} from '@/components/ui/market-context'
import { Segmented } from '@/components/ui/segmented'
import { Term } from '@/components/ui/term'
import { useAnalogs, useLatestSentiment, useSentimentHistory } from '@/lib/context-api'

function Card({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <section className="glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30">
      <h3 className="mb-3 text-[14px] font-bold">{title}</h3>
      {children}
    </section>
  )
}

const muted = (text: string) => <p className="text-[12.5px] text-muted-foreground">{text}</p>

export function MarketContextView() {
  const latest = useLatestSentiment()
  const analogs = useAnalogs()
  const [picked, setPicked] = useState<string | null>(null)

  const ids = [
    ...new Set([
      ...(latest.status === 'ok' ? latest.data.map((s) => s.instrument_id) : []),
      ...(analogs.status === 'ok' ? analogs.data.map((a) => a.instrument_id) : []),
    ]),
  ].sort()
  const selected = picked && ids.includes(picked) ? picked : (ids[0] ?? null)
  const history = useSentimentHistory(selected)
  const index =
    latest.status === 'ok' ? latest.data.find((s) => s.instrument_id === selected) : undefined
  const report =
    analogs.status === 'ok' ? analogs.data.find((a) => a.instrument_id === selected) : undefined

  if (latest.status === 'error' && analogs.status === 'error')
    return (
      <p role="alert" className="text-[12.5px]">
        No se pudo leer al motor ({latest.message}).
      </p>
    )

  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <p className="max-w-2xl text-[12.5px] text-muted-foreground">
          Solo informativo: no suma al puntaje de las propuestas ni es una recomendación.
        </p>
        {ids.length > 1 && selected && (
          <Segmented label="Índice" options={ids} value={selected} onChange={setPicked} />
        )}
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        <Card title={<Term id="indice-sentimiento">Índice de sentimiento</Term>}>
          {latest.status === 'loading' ? (
            muted('Pidiendo los datos al motor…')
          ) : index ? (
            <div className="grid gap-4">
              <SentimentGauge index={index} />
              {history.status === 'ok' && <SentimentHistory history={history.data} />}
            </div>
          ) : (
            muted('Sin índice guardado para este instrumento.')
          )}
        </Card>
        <Card title={<Term id="analogos">Análogos históricos</Term>}>
          {analogs.status === 'loading' ? (
            muted('Pidiendo los datos al motor…')
          ) : report ? (
            <AnalogDistribution report={report} />
          ) : (
            muted('Sin análogos guardados para este instrumento.')
          )}
        </Card>
      </div>
    </div>
  )
}
