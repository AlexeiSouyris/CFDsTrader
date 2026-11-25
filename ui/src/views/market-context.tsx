/**
 * Contexto de mercado del Panel (#294), solo lectura: el índice de sentimiento de EE. UU. y los
 * análogos históricos, tal como los calculó Python y los sirve el gateway, y al lado del índice los
 * estados emocionales de la spec §9.1 (#220) con los valores de su regla. Los instrumentos salen
 * de lo que hay guardado (nada fijo en el código). No calcula nada.
 */
import type { ReactNode } from 'react'

import {
  AnalogDistribution,
  SentimentGauge,
  SentimentHistory,
} from '@/components/ui/market-context'
import { EmotionalStates } from '@/components/ui/emotional-states'
import { Segmented } from '@/components/ui/segmented'
import { Term } from '@/components/ui/term'
import { useMarketContext } from '@/lib/use-market-context'
import { cn } from '@/lib/utils'

function Card({
  title,
  children,
  wide,
}: {
  title: ReactNode
  children: ReactNode
  wide?: boolean
}) {
  return (
    <section className={cn('glass relative min-w-0 rounded-[20px] p-4', wide && 'lg:col-span-2')}>
      <h3 className="mb-3 text-[14px] font-bold">{title}</h3>
      {children}
    </section>
  )
}

/** Lo que se muestra mientras llega una lectura o cuando no hay nada guardado. */
const muted = (text: string) => <p className="text-[12.5px] text-muted-foreground">{text}</p>
const waiting = (
  <p role="status" className="text-[12.5px] text-muted-foreground">
    Pidiendo los datos al motor…
  </p>
)
const failed = (text: string) => (
  <p role="alert" className="text-[12.5px]">
    {text} Se vuelve a intentar cada minuto.
  </p>
)

export function MarketContextView() {
  const { latest, analogs, emotions, ids, selected, setPicked, history, index, report, moods } =
    useMarketContext()

  if (latest.status === 'error' && analogs.status === 'error')
    return (
      <p role="alert" className="text-[12.5px]">
        No se pudo leer al motor ({latest.message}).
      </p>
    )

  return (
    <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
      <div className="flex min-w-0 flex-wrap items-end justify-between gap-3">
        <p className="max-w-2xl text-[12.5px] text-muted-foreground">
          Solo informativo: no suma al puntaje de las propuestas ni es una recomendación.
        </p>
        {ids.length > 1 && selected && (
          // Con muchos instrumentos el selector se desliza en vez de ensanchar la página (#357).
          <div className="max-w-full min-w-0 overflow-x-auto [scrollbar-width:thin]">
            <Segmented label="Índice" options={ids} value={selected} onChange={setPicked} />
          </div>
        )}
      </div>
      <div className="grid grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-2">
        <Card title={<Term id="indice-sentimiento">Índice de sentimiento</Term>}>
          {latest.status === 'loading' ? (
            waiting
          ) : index ? (
            <div className="grid gap-4">
              <SentimentGauge index={index} />
              {history.status === 'ok' ? (
                <SentimentHistory history={history.data} />
              ) : history.status === 'error' ? (
                muted('No se pudo leer la historia del índice; se vuelve a intentar cada minuto.')
              ) : null}
            </div>
          ) : latest.status === 'error' ? (
            failed('No se pudo leer el índice del motor.')
          ) : (
            muted('Sin índice guardado para este instrumento.')
          )}
        </Card>
        <Card title="Estados emocionales">
          {emotions.status === 'loading' ? (
            waiting
          ) : moods.length ? (
            <EmotionalStates rows={moods} />
          ) : emotions.status === 'error' ? (
            failed('No se pudieron leer los estados emocionales.')
          ) : (
            muted('Sin estados guardados para este instrumento.')
          )}
          <p className="mt-3 text-[11px] text-muted-foreground">
            Son reglas medidas sobre lo que ya pasó, no un pronóstico.
          </p>
        </Card>
        <Card title={<Term id="analogos">Análogos históricos</Term>} wide>
          {analogs.status === 'loading' ? (
            waiting
          ) : report ? (
            <AnalogDistribution report={report} />
          ) : analogs.status === 'error' ? (
            failed('No se pudieron leer los análogos.')
          ) : (
            muted('Sin análogos guardados para este instrumento.')
          )}
        </Card>
      </div>
    </div>
  )
}
