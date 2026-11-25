/**
 * Trofeos (#581, etapa 8 de docs/design/2026-11-rediseno-ui.md): un tablero de logros sobrio, con
 * acento dorado contenido. Premian el **proceso** (respetar el stop, completar el diario, quedarse
 * dentro del límite diario, responder las propuestas a tiempo), **nunca la ganancia**. Sin confeti
 * ni lenguaje de casino, y sin ordenar a nadie por resultado.
 *
 * Datos reales (#616): el motor todavía no calcula ningún trofeo (no hay operaciones del runner de
 * paper), así que no se muestra ningún estado, avance, fecha ni miembro inventado: cada trofeo
 * previsto aparece "sin datos" con su regla, y la tabla del grupo como estado vacío. Cuando el
 * motor los calcule, el estado llega hecho: la UI no cuenta.
 */
import { CircleDashed } from 'lucide-react'

import { TROPHY_RULES, type TrophyRule } from '@/lib/trophy-rules'

const PERIOD = { día: 'De la jornada', semana: 'De la semana', mes: 'Del mes' } as const

function Badge({ rule }: { rule: TrophyRule }) {
  return (
    <li
      data-state="sin datos"
      className="relative grid min-w-0 content-start gap-3 rounded-section border border-border p-4"
    >
      <div className="flex items-start justify-between gap-3">
        <span
          aria-hidden="true"
          className="grid size-10 shrink-0 place-items-center rounded-full border border-border text-muted-foreground"
        >
          <CircleDashed className="size-5" strokeWidth={1.75} />
        </span>
        <span className="rounded-md px-1.5 py-0.5 text-[10.5px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
          sin datos
        </span>
      </div>
      <div className="grid gap-1">
        <h3 className="text-[15px] font-bold">{rule.title}</h3>
        <p className="text-[12.5px] leading-snug text-muted-foreground">{rule.rule}</p>
      </div>
      <p className="mt-auto text-[11.5px] text-muted-foreground">{PERIOD[rule.period]}</p>
    </li>
  )
}

export function CompetitionView() {
  return (
    <div
      data-section="trophies"
      className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-6 rounded-section bg-section p-4 text-foreground sm:p-6"
    >
      <header className="grid max-w-[68ch] gap-2">
        <h2 className="text-[clamp(22px,2.4vw,28px)] font-bold tracking-[-0.02em]">
          Logros del proceso
        </h2>
        <p className="text-[13px] text-muted-foreground">
          Los trofeos premian cómo se opera, no cuánto se gana: respetar el stop, anotar cada
          operación, quedarse dentro del límite diario y responder a tiempo. Es simulación.
        </p>
      </header>

      <p
        role="status"
        className="max-w-[68ch] rounded-xl border border-dashed border-border px-4 py-3 text-[13.5px] leading-relaxed text-muted-foreground"
      >
        Todavía no hay trofeos: se ganan con las operaciones en simulación, y el runner arranca el
        27-11. Abajo están los previstos con su regla; ninguno está logrado todavía.
      </p>

      <ul aria-label="Trofeos" className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {TROPHY_RULES.map((r) => (
          <Badge key={r.id} rule={r} />
        ))}
      </ul>

      <section aria-labelledby="trofeos-grupo" className="grid min-w-0 gap-3">
        <h2 id="trofeos-grupo" className="text-[15px] font-bold">
          El grupo: reglas cumplidas
        </h2>
        <p className="rounded-section border border-border px-4 py-6 text-[13.5px] text-muted-foreground">
          Todavía no hay datos del grupo: nadie operó en simulación.
        </p>
        <p className="text-[11.5px] text-muted-foreground">
          Sin puestos ni resultados en dinero: cada uno se mide contra las reglas, no contra los
          demás.
        </p>
      </section>
    </div>
  )
}
