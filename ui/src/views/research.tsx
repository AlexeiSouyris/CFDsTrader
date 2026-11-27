/**
 * Investigación (#581, etapa 6, `data-section="research"`): un informe editorial que reúne
 * Performance, Propuestas, Liquidez y Glosario en una sola página, con un índice lateral fijo y
 * títulos en serif. Cada parte es la vista de siempre, que sigue abriéndose sola con su hash
 * (`#performance`, `#propuestas`, `#liquidez`, `#glosario`). Solo ubica: no pide nada nuevo ni
 * calcula nada; ordenar una tabla es presentación.
 */
import { useEffect, useState, type ReactNode } from 'react'

import { cn } from '@/lib/utils'
import { CURRENT_MEMBER } from '@/mocks'
import { LiquidityView } from '@/views/liquidity'
import { PerformanceView } from '@/views/performance'
import { GlossaryView, ProposalsView } from '@/views/phase10'

/**
 * Las entradas del índice: las partes de la página (con su vista de siempre) y, dentro de
 * Performance, sus bloques (ids de `views/phase8.tsx`).
 */
const RESEARCH_INDEX = [
  { id: 'inv-performance', label: 'Corridas', view: 'performance' },
  { id: 'perf-paper-vs-backtest', label: 'Paper vs backtest' },
  { id: 'perf-aprendizaje', label: 'Aprendizaje' },
  { id: 'perf-modelo', label: 'Modelo' },
  { id: 'inv-propuestas', label: 'Propuestas', view: 'propuestas' },
  { id: 'inv-liquidez', label: 'Liquidez', view: 'liquidez' },
  { id: 'inv-glosario', label: 'Glosario', view: 'glosario' },
] as const

/** Lleva la página a una parte y le pasa el foco (sin tocar el hash: el hash es la vista). */
function jump(id: string) {
  const el = document.getElementById(id)
  if (!el) return
  const target = (el.closest('section') ?? el) as HTMLElement
  if (!target.hasAttribute('tabindex')) target.setAttribute('tabindex', '-1')
  target.scrollIntoView?.({ block: 'start' })
  target.focus({ preventScroll: true })
}

/** La parte que se está leyendo: la última cuyo comienzo ya pasó el tercio de arriba. */
function useCurrentPart() {
  const [current, setCurrent] = useState<string>(RESEARCH_INDEX[0].id)
  useEffect(() => {
    const onScroll = () => {
      const line = window.innerHeight / 3
      let at: string = RESEARCH_INDEX[0].id
      for (const { id } of RESEARCH_INDEX) {
        const el = document.getElementById(id)
        if (el && el.getBoundingClientRect().top <= line) at = id
      }
      setCurrent(at)
    }
    onScroll()
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])
  return current
}

function Part({
  id,
  kicker,
  title,
  view,
  onOpen,
  children,
}: {
  id: string
  kicker: string
  title: string
  view: string
  onOpen: (view: string) => void
  children: ReactNode
}) {
  return (
    <section aria-labelledby={id} className="grid min-w-0 gap-4 border-t border-border pt-8">
      <header className="flex flex-wrap items-end justify-between gap-2">
        <div>
          <p className="text-[11px] font-bold tracking-[0.14em] text-accent uppercase">{kicker}</p>
          <h2 id={id} data-research-title className="mt-1">
            {title}
          </h2>
        </div>
        <a
          href={`#${view}`}
          onClick={(e) => {
            if (e.metaKey || e.ctrlKey || e.shiftKey) return
            e.preventDefault()
            onOpen(view)
          }}
          className="inline-flex min-h-6 items-center rounded-sm text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
        >
          Abrir sola →
        </a>
      </header>
      {children}
    </section>
  )
}

export function ResearchView({ onOpen }: { onOpen: (view: string) => void }) {
  const current = useCurrentPart()
  return (
    <div
      data-section="research"
      className="grid min-w-0 gap-6 rounded-section bg-section p-4 text-foreground sm:p-6 lg:grid-cols-[176px_minmax(0,1fr)] lg:gap-10"
    >
      <nav
        aria-label="Índice de Investigación"
        className="min-w-0 lg:sticky lg:top-6 lg:self-start"
      >
        <p className="mb-2 text-[11px] font-bold tracking-[0.14em] text-muted-foreground uppercase max-lg:sr-only">
          En esta página
        </p>
        <ol className="flex gap-1 overflow-x-auto pb-1 lg:grid lg:gap-0.5 lg:overflow-visible lg:pb-0">
          {RESEARCH_INDEX.map((p) => {
            const on = current === p.id
            const sub = !('view' in p)
            return (
              <li key={p.id} className="shrink-0">
                <button
                  type="button"
                  aria-current={on ? 'location' : undefined}
                  onClick={() => jump(p.id)}
                  className={cn(
                    'inline-flex min-h-9 items-center gap-2 rounded-sm px-2.5 text-left text-[13px] whitespace-nowrap text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring max-lg:rounded-full max-lg:border max-lg:border-border lg:min-h-8 lg:w-full lg:px-0',
                    sub && 'lg:pl-4 lg:text-[12.5px]',
                    on && 'font-bold text-accent',
                  )}
                >
                  {/* la parte que se lee: el texto en el acento y un punto chico, sin franja lateral */}
                  <span
                    aria-hidden
                    className={cn(
                      'size-1.5 shrink-0 rounded-full bg-accent transition-opacity max-lg:hidden',
                      !on && 'opacity-0',
                    )}
                  />
                  {p.label}
                </button>
              </li>
            )
          })}
        </ol>
      </nav>

      <div className="grid min-w-0 gap-10">
        <p className="max-w-[68ch] font-serif text-[17px] leading-relaxed text-muted-foreground">
          Lo que el motor probó y midió, para leer con calma: corridas con sus costos, la
          comparación con la simulación, el modelo, las propuestas, la liquidez y los términos. Son
          resultados del pasado, no promesas.
        </p>
        <Part
          id="inv-performance"
          kicker="Corridas y modelo"
          title="Performance"
          view="performance"
          onOpen={onOpen}
        >
          <PerformanceView onOpen={onOpen} />
        </Part>
        <Part
          id="inv-propuestas"
          kicker="Lo que propone el motor"
          title="Propuestas"
          view="propuestas"
          onOpen={onOpen}
        >
          <ProposalsView member={CURRENT_MEMBER} />
        </Part>
        <Part
          id="inv-liquidez"
          kicker="Niveles y desequilibrio"
          title="Liquidez"
          view="liquidez"
          onOpen={onOpen}
        >
          <LiquidityView headingLevel={3} />
        </Part>
        <Part
          id="inv-glosario"
          kicker="Para entender los términos"
          title="Glosario"
          view="glosario"
          onOpen={onOpen}
        >
          <GlossaryView />
        </Part>
      </div>
    </div>
  )
}
