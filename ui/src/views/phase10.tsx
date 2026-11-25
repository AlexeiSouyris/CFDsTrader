/**
 * Vistas del adelanto de la fase 10 (#171): propuestas, fondo y glosario.
 * Datos de ejemplo de `mocks/` y definiciones de `docs/GLOSARIO.md`; el fondo llega del motor por el
 * gateway (#312) y, sin gateway, es el de ejemplo. Nada se calcula acá. Lo que es de ejemplo lleva
 * el sello "EJEMPLO" (#577).
 */
import { ScrollRegion } from '@/components/ui/scroll-region'
import { Search } from 'lucide-react'
import { useDeferredValue, useState, type ReactNode } from 'react'

import { ExampleBadge, ExampleNotice } from '@/components/ui/data-honesty'
import { ProposalCard } from '@/components/ui/proposal-card'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, dateUtc, decimal, isNegative, signed } from '@/lib/format'
import { currencySign, useFund, type FundMovement } from '@/lib/fund-api'
import { GLOSSARY } from '@/lib/glossary'
import { cn } from '@/lib/utils'
import { FUND, MEMBERS, PROPOSALS, RESPONSIBLE_ID, type Member } from '@/mocks'

export function ProposalsView({ member }: { member: Member }) {
  const canApprove = member.id === RESPONSIBLE_ID
  return (
    <div className="grid gap-3">
      <p className="max-w-2xl text-[13px] text-muted-foreground">
        El motor propone; el responsable aprueba o rechaza antes de que venza (10 minutos). Cada
        propuesta ya pasó por el módulo de riesgo.
      </p>
      <ExampleNotice className="max-w-2xl">
        Propuestas de ejemplo: el motor todavía no las publica acá, así que no se pueden aprobar.
      </ExampleNotice>
      {/* cada tarjeta lleva un h3: este h2 deja el orden de títulos sin saltos (axe heading-order) */}
      <h2 className="mt-1 text-[14px] font-bold">En espera ({PROPOSALS.length})</h2>
      {PROPOSALS.length === 0 ? (
        <p className="panel rounded-[20px] p-6 text-center text-[13px] text-muted-foreground">
          No hay propuestas esperando.
        </p>
      ) : (
        PROPOSALS.map((p) => (
          <ProposalCard key={p.proposal.id} item={p} canApprove={canApprove} example />
        ))
      )}
    </div>
  )
}

/** Nombre para mostrar: el del miembro si se conoce, si no su id tal como llega. */
const memberName = (id: string) => MEMBERS.find((m) => m.id === id)?.name ?? id

/**
 * El fondo del grupo (fase 6b, #312): lo que manda el motor por el gateway
 * (`fund_summary.v1` y `fund_movement.v1[]`). Sin gateway, el ejemplo, marcado "(ejemplo)".
 */
/**
 * La curva del fondo: el valor de la unidad en cada liquidación, tal como lo manda el motor en cada
 * movimiento (`fund_movement.v1.nav`). Una liquidación por punto, en el orden en que llegan;
 * `Number()` solo ubica cada punto en el dibujo.
 */
function NavCurve({ movements }: { movements: FundMovement[] }) {
  const w = 480
  const h = 160
  if (movements.length < 2)
    return (
      <p className="self-center rounded-xl border border-dashed border-border px-3 py-2.5 text-[12.5px] text-muted-foreground">
        La curva aparece con dos liquidaciones o más.
      </p>
    )
  const navs = movements.map((m) => Number(m.nav))
  const lo = Math.min(...navs)
  const span = Math.max(...navs) - lo || 1
  const x = (i: number) => (i / (movements.length - 1)) * w
  const y = (v: number) => h - 8 - ((v - lo) / span) * (h - 16)
  const d = navs.map((v, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join('')
  const first = movements[0]
  const last = movements[movements.length - 1]
  return (
    <figure className="grid min-w-0 content-end gap-2">
      <svg
        viewBox={`0 0 ${w} ${h}`}
        role="img"
        aria-label={`Valor de la unidad en ${movements.length} liquidaciones, de ${decimal(first.nav, 4)} el ${dateUtc(first.settled_at)} a ${decimal(last.nav, 4)} el ${dateUtc(last.settled_at)}`}
        className="h-40 w-full overflow-visible"
        preserveAspectRatio="none"
      >
        <line x1={0} x2={w} y1={h - 0.5} y2={h - 0.5} stroke="var(--chart-grid)" />
        <path
          d={d}
          fill="none"
          stroke="var(--chart-accent)"
          strokeWidth={2}
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <figcaption className="flex justify-between gap-2 text-[11px] text-muted-foreground">
        <span>{dateUtc(first.settled_at)}</span>
        <span>Valor de la unidad en cada liquidación</span>
        <span>{dateUtc(last.settled_at)}</span>
      </figcaption>
    </figure>
  )
}

export function FundView() {
  const { summary: f, movements, live, loading } = useFund()
  const cur = currencySign(f.currency)
  const origin = live ? 'del motor' : loading ? 'pidiendo al motor…' : '(ejemplo)'
  const originMark =
    live || loading ? (
      <span className="text-[11px] font-semibold text-muted-foreground">{origin}</span>
    ) : (
      <ExampleBadge />
    )
  const stat = (label: ReactNode, value: string, detail: string) => (
    <div className="grid min-w-0 content-start gap-1.5">
      <dt className="text-[11px] leading-tight font-bold text-muted-foreground">{label}</dt>
      <dd className="text-[clamp(18px,1.8vw,22px)] leading-none font-extrabold tracking-[-0.03em] [overflow-wrap:anywhere] tabular-nums">
        {value}
      </dd>
      <dd className="text-[11px] leading-snug text-muted-foreground">{detail}</dd>
    </div>
  )
  return (
    // Fondo (#581, etapa 8): resumen con vidrio, como una extensión del Inicio. El vidrio se
    // prende solo en esta vista (`data-glass`); el fondo animado sigue siendo solo del Inicio.
    <div
      className="grid gap-4"
      data-section="fund"
      data-glass="on"
      data-fund-source={live ? 'engine' : 'example'}
    >
      <p className="max-w-3xl text-[12.5px] text-muted-foreground">
        {live ? (
          <>
            El fondo tal como lo calcula el motor desde el historial, valuado al cerrar la última
            liquidación. Es <strong className="text-foreground">simulación</strong>.
          </>
        ) : (
          <>
            <strong className="text-foreground">Datos de ejemplo</strong>: el gateway no respondió.
          </>
        )}{' '}
        Resultados del pasado, no promesas.
      </p>
      <section
        aria-labelledby="fondo-unidad"
        className="glass relative grid min-w-0 gap-6 rounded-section p-5 sm:p-7 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]"
      >
        <div className="grid min-w-0 content-between gap-6">
          <div className="grid gap-2">
            <h2
              id="fondo-unidad"
              className="flex flex-wrap items-center gap-2 text-[12px] font-bold text-muted-foreground"
            >
              <Term id="nav">Valor de la unidad</Term> {originMark}
            </h2>
            <strong className="text-[clamp(40px,5vw,60px)] leading-none font-extrabold tracking-[-0.05em] [overflow-wrap:anywhere] tabular-nums">
              {`${decimal(f.nav, 4)} ${cur}`}
            </strong>
            <span className="text-[12px] text-muted-foreground">
              {f.valued_at
                ? `liquidación del ${dateTimeUtc(f.valued_at)}`
                : 'sin liquidaciones todavía'}
            </span>
          </div>
          <dl className="grid grid-cols-2 gap-4 border-t border-border pt-4">
            {stat('Valor del fondo', `${decimal(f.total_value)} ${cur}`, `simulación ${origin}`)}
            {stat(
              <Term id="unidades">Unidades emitidas</Term>,
              decimal(f.total_units),
              `${f.members.length} ${f.members.length === 1 ? 'miembro' : 'miembros'}`,
            )}
          </dl>
        </div>
        <NavCurve movements={movements} />
      </section>

      <section className="glass relative min-w-0 rounded-section p-4 sm:p-5">
        <h2 className="mb-3 flex flex-wrap items-center gap-x-2 text-[14px] font-bold">
          Cada miembro: <Term id="unidades">unidades</Term> y <Term id="pl">P&amp;L</Term>{' '}
          {originMark}
        </h2>
        {f.members.length === 0 ? (
          <p className="rounded-xl border border-dashed border-border px-3 py-2.5 text-[12.5px] text-muted-foreground">
            Todavía no hay aportes liquidados.
          </p>
        ) : (
          <div className="rounded-xl border border-border bg-card p-2 sm:px-4">
            <ScrollRegion label="Cada miembro">
              <table className="w-full min-w-[520px] text-[13px]" aria-label="Cada miembro">
                <thead>
                  <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
                    <th className="py-2 font-bold">Miembro</th>
                    <th className="py-2 pl-3 text-right font-bold">Unidades</th>
                    <th className="py-2 pl-3 text-right font-bold">Aportes netos</th>
                    <th className="py-2 pl-3 text-right font-bold">Valor</th>
                    <th className="py-2 pl-3 text-right font-bold">P&amp;L</th>
                  </tr>
                </thead>
                <tbody>
                  {f.members.map((m) => (
                    <tr key={m.member_id} className="border-b border-border last:border-b-0">
                      <td className="py-2.5 font-semibold">{memberName(m.member_id)}</td>
                      <td className="py-2.5 pl-3 text-right tabular-nums">{decimal(m.units)}</td>
                      <td className="py-2.5 pl-3 text-right tabular-nums">
                        {decimal(m.net_contributions)} {cur}
                      </td>
                      <td className="py-2.5 pl-3 text-right tabular-nums">
                        {decimal(m.value)} {cur}
                      </td>
                      <td
                        className={cn(
                          'py-2.5 pl-3 text-right font-bold tabular-nums',
                          isNegative(m.pnl) ? 'text-down' : 'text-up',
                        )}
                      >
                        {signed(m.pnl)} {cur}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ScrollRegion>
          </div>
        )}
      </section>

      <section className="glass min-w-0 rounded-section p-4 sm:p-5">
        <h2 className="flex flex-wrap items-center gap-2 text-[14px] font-bold">
          Historial {originMark}
        </h2>
        <p className="mb-3 text-[11px] text-muted-foreground">
          Movimientos ya liquidados. Las cuentas de cada miembro salen de este registro, que no se
          modifica ni se borra.
        </p>
        {movements.length === 0 ? (
          <p className="rounded-xl border border-dashed border-border px-3 py-2.5 text-[12.5px] text-muted-foreground">
            Todavía no hay movimientos.
          </p>
        ) : (
          <div className="rounded-xl border border-border bg-card p-2 sm:px-4">
            <ScrollRegion label="Movimientos del fondo">
              <table
                className="w-full min-w-[520px] text-[12px]"
                aria-label="Movimientos del fondo"
              >
                <thead>
                  <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
                    <th className="py-2 font-bold">Liquidación</th>
                    <th className="py-2 font-bold">Miembro</th>
                    <th className="py-2 font-bold">Tipo</th>
                    <th className="py-2 pl-3 text-right font-bold">Monto</th>
                    <th className="py-2 pl-3 text-right font-bold">Unidades</th>
                    <th className="py-2 pl-3 text-right font-bold">NAV</th>
                  </tr>
                </thead>
                <tbody>
                  {[...movements].reverse().map((mv) => (
                    <tr key={mv.id} className="border-b border-border last:border-b-0">
                      <td className="py-2 whitespace-nowrap text-muted-foreground tabular-nums">
                        {dateTimeUtc(mv.settled_at)}
                      </td>
                      <td className="py-2 font-semibold">{memberName(mv.member_id)}</td>
                      <td
                        className={mv.kind === 'contribution' ? 'py-2 text-up' : 'py-2 text-down'}
                      >
                        {mv.kind === 'contribution' ? 'aporte' : 'retiro'}
                      </td>
                      <td className="py-2 pl-3 text-right tabular-nums">
                        {decimal(mv.amount)} {currencySign(mv.currency)}
                      </td>
                      <td className="py-2 pl-3 text-right tabular-nums">{decimal(mv.units)}</td>
                      <td className="py-2 pl-3 text-right tabular-nums">{decimal(mv.nav)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ScrollRegion>
          </div>
        )}
      </section>

      {live ? (
        <p className="px-1 text-[11.5px] text-muted-foreground">
          Movimientos pendientes: el motor todavía no publica los pedidos del lunes, así que no se
          muestran.
        </p>
      ) : (
        <section className="glass rounded-section p-4 sm:p-5" data-example>
          <h2 className="flex flex-wrap items-center gap-2 text-[14px] font-bold">
            Movimientos pendientes <ExampleBadge />
          </h2>
          <p className="mb-3 text-[11px] text-muted-foreground">
            Se hacen el lunes, antes de la apertura europea, al valor de la unidad del cierre del
            viernes. El motor todavía no publica los pedidos pendientes: esto es de ejemplo.
          </p>
          <ul className="grid gap-2">
            {FUND.moves.map((mv) => (
              <li
                key={mv.id}
                className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-xl border border-border px-3 py-2 text-[12px]"
              >
                <span className="font-semibold">{mv.member.name}</span>
                <span className={mv.kind === 'aporte' ? 'text-up' : 'text-down'}>{mv.kind}</span>
                <span className="tabular-nums">{decimal(mv.amount)} €</span>
                <span className="text-muted-foreground tabular-nums">
                  {decimal(mv.units)} unidades a {decimal(mv.nav)} €
                </span>
                <span className="text-muted-foreground tabular-nums sm:ml-auto">
                  {dateTimeUtc(mv.at)}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  )
}

const fold = (s: string) =>
  s
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()

export function GlossaryView() {
  const [query, setQuery] = useState('')
  const q = fold(useDeferredValue(query).trim())
  const found = q ? GLOSSARY.filter((e) => fold(`${e.title} ${e.body}`).includes(q)) : GLOSSARY
  return (
    <div className="grid gap-3">
      <label className="flex items-center gap-2 border-b border-foreground/70 px-1 py-2.5 focus-within:border-accent">
        <Search className="size-4 text-muted-foreground" aria-hidden="true" />
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Buscar un término (por ejemplo: sweep, margen, R)"
          aria-label="Buscar en el glosario"
          className="min-h-6 w-full bg-transparent text-[14px] outline-none placeholder:text-muted-foreground"
        />
      </label>
      <p className="text-[11px] text-muted-foreground" aria-live="polite">
        {found.length} de {GLOSSARY.length} términos
      </p>
      {/* un diccionario (#641): término a la izquierda, definición a la derecha, líneas finas */}
      <div className="grid border-t border-border">
        {found.map((e) => (
          <article
            key={e.id}
            id={`termino-${e.id}`}
            className="grid gap-x-8 gap-y-1 border-b border-border py-4 md:grid-cols-[14rem_minmax(0,1fr)]"
          >
            <h2 className="font-serif text-[17px] leading-snug font-semibold">{e.title}</h2>
            <div className="max-w-[68ch]">
              <p className="text-[13px] leading-relaxed whitespace-pre-line">{e.body}</p>
              {e.example && (
                <p className="mt-2 text-[12px] leading-relaxed whitespace-pre-line text-muted-foreground">
                  <em className="font-semibold not-italic">Ejemplo: </em>
                  {e.example}
                </p>
              )}
            </div>
          </article>
        ))}
      </div>
    </div>
  )
}
