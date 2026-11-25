/**
 * Trofeos (#581, etapa 8 de docs/design/2026-11-rediseno-ui.md): un tablero de logros sobrio, con
 * acento dorado contenido. Premian el **proceso** (respetar el stop, completar el diario, quedarse
 * dentro del límite diario, responder las propuestas a tiempo), **nunca la ganancia**. Sin confeti
 * ni lenguaje de casino, y sin ordenar a nadie por resultado.
 *
 * Honestidad de los datos (#577): los trofeos y la tabla todavía no tienen datos del motor, así que
 * son de ejemplo y llevan el sello "EJEMPLO". El estado y el avance llegan hechos: la UI no cuenta.
 */
import { Award, CircleDashed, Lock } from 'lucide-react'

import { ExampleNotice } from '@/components/ui/data-honesty'
import { ScrollRegion } from '@/components/ui/scroll-region'
import { dateUtc } from '@/lib/format'
import { cn } from '@/lib/utils'
import { PROCESS_BADGES, PROCESS_TABLE, type ProcessBadge } from '@/mocks'

const PERIOD = { día: 'De la jornada', semana: 'De la semana', mes: 'Del mes' } as const
const STATE_ICON = { logrado: Award, 'en curso': CircleDashed, bloqueado: Lock } as const

function Badge({ badge }: { badge: ProcessBadge }) {
  const Icon = STATE_ICON[badge.state]
  const done = badge.state === 'logrado'
  return (
    <li
      data-state={badge.state}
      className={cn(
        'relative grid min-w-0 content-start gap-3 rounded-section border p-4',
        done ? 'border-accent/60 bg-accent/[0.07]' : 'border-border',
        badge.state === 'bloqueado' && 'opacity-75',
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <span
          aria-hidden="true"
          className={cn(
            'grid size-10 shrink-0 place-items-center rounded-full border',
            done ? 'border-accent text-accent' : 'border-border text-muted-foreground',
          )}
        >
          <Icon className="size-5" strokeWidth={1.75} />
        </span>
        <span
          className={cn(
            'rounded-md px-1.5 py-0.5 text-[10.5px] font-bold tracking-[0.08em] uppercase',
            done ? 'bg-accent/15 text-accent' : 'text-muted-foreground',
          )}
        >
          {badge.state}
        </span>
      </div>
      <div className="grid gap-1">
        <h3 className="text-[15px] font-bold">{badge.title}</h3>
        <p className="text-[12.5px] leading-snug text-muted-foreground">{badge.rule}</p>
      </div>
      <p className="mt-auto text-[11.5px] text-muted-foreground">
        {PERIOD[badge.period]}
        {badge.earned_at && (
          <>
            {' · '}logrado el <time dateTime={badge.earned_at}>{dateUtc(badge.earned_at)}</time>
          </>
        )}
        {badge.progress && <> · avance {badge.progress}</>}
      </p>
    </li>
  )
}

export function CompetitionView() {
  return (
    <div
      data-section="trophies"
      className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-6 rounded-section bg-section p-4 text-foreground sm:p-6"
    >
      <ExampleNotice>
        Miembros, trofeos y avances de ejemplo: la competencia todavía no tiene datos del motor.
      </ExampleNotice>
      <header className="grid max-w-[68ch] gap-2">
        <h2 className="text-[clamp(22px,2.4vw,28px)] font-bold tracking-[-0.02em]">
          Logros del proceso
        </h2>
        <p className="text-[13px] text-muted-foreground">
          Los trofeos premian cómo se opera, no cuánto se gana: respetar el stop, anotar cada
          operación, quedarse dentro del límite diario y responder a tiempo. Es simulación.
        </p>
      </header>

      <ul aria-label="Trofeos" className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {PROCESS_BADGES.map((b) => (
          <Badge key={b.id} badge={b} />
        ))}
      </ul>

      <section aria-labelledby="trofeos-grupo" className="grid min-w-0 gap-3">
        <h2 id="trofeos-grupo" className="text-[15px] font-bold">
          El grupo esta semana: reglas cumplidas
        </h2>
        <div className="min-w-0 rounded-section border border-border">
          <ScrollRegion label="Reglas cumplidas">
            <table className="w-full min-w-[480px] text-[13px]" aria-label="Reglas cumplidas">
              <thead>
                <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
                  <th className="px-3 py-2 font-bold">Miembro</th>
                  <th className="px-3 py-2 text-right font-bold">Stop respetado</th>
                  <th className="px-3 py-2 text-right font-bold">Notas en el diario</th>
                  <th className="px-3 py-2 text-right font-bold">Dentro del límite</th>
                </tr>
              </thead>
              <tbody>
                {PROCESS_TABLE.map((r) => (
                  <tr key={r.member.id} className="border-b border-border last:border-b-0">
                    <td className="px-3 py-2.5 font-semibold whitespace-nowrap">{r.member.name}</td>
                    <td className="px-3 py-2.5 text-right tabular-nums">{r.stops}</td>
                    <td className="px-3 py-2.5 text-right tabular-nums">{r.journal}</td>
                    <td className="px-3 py-2.5 text-right tabular-nums">{r.limits}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </ScrollRegion>
        </div>
        <p className="text-[11.5px] text-muted-foreground">
          Sin puestos ni resultados en dinero: cada uno se mide contra las reglas, no contra los
          demás.
        </p>
      </section>
    </div>
  )
}
