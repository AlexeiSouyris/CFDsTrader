/**
 * Vistas del adelanto de la fase 10 (#171): propuestas, fondo y glosario.
 * Datos de ejemplo de `mocks/` y definiciones de `docs/GLOSARIO.md`. Nada se calcula acá.
 */
import { Search } from 'lucide-react'
import { useDeferredValue, useState, type ReactNode } from 'react'

import { ProposalCard } from '@/components/ui/proposal-card'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, decimal, isNegative, signed } from '@/lib/format'
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
      {PROPOSALS.length === 0 ? (
        <p className="glass rounded-[20px] p-6 text-center text-[13px]">
          No hay propuestas esperando.
        </p>
      ) : (
        PROPOSALS.map((p) => <ProposalCard key={p.proposal.id} item={p} canApprove={canApprove} />)
      )}
    </div>
  )
}

export function FundView() {
  const stat = (label: ReactNode, value: string, detail: string) => (
    <section className="glass relative flex min-h-28 flex-col justify-between rounded-[20px] p-4 has-[[role=dialog]]:z-30">
      <span className="text-[11px] font-bold text-muted-foreground">{label}</span>
      <strong className="text-[clamp(22px,2.4vw,30px)] leading-none font-extrabold tracking-[-0.04em]">
        {value}
      </strong>
      <span className="text-[11px] text-muted-foreground">{detail}</span>
    </section>
  )
  return (
    <div className="grid gap-3">
      <div className="grid gap-3 sm:grid-cols-3">
        {stat(
          <Term id="nav">Valor de la unidad</Term>,
          `${decimal(FUND.nav)} €`,
          `cierre del ${dateTimeUtc(FUND.navAt)}`,
        )}
        {stat('Valor del fondo', `${decimal(FUND.total)} €`, 'simulación')}
        {stat(<Term id="unidades">Unidades emitidas</Term>, decimal(FUND.units), '6 miembros')}
      </div>

      <section className="glass relative min-w-0 rounded-[20px] p-4 has-[[role=dialog]]:z-30">
        <h2 className="mb-3 text-[14px] font-bold">
          Cada miembro: <Term id="unidades">unidades</Term> y <Term id="pl">P&amp;L</Term>
        </h2>
        <div className="overflow-x-auto">
          <table className="w-full min-w-[520px] text-[13px]">
            <thead>
              <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
                <th className="py-2 font-bold">Miembro</th>
                <th className="py-2 text-right font-bold">Unidades</th>
                <th className="py-2 text-right font-bold">Aportes netos</th>
                <th className="py-2 text-right font-bold">Valor</th>
                <th className="py-2 text-right font-bold">P&amp;L</th>
              </tr>
            </thead>
            <tbody>
              {FUND.members.map((m) => (
                <tr key={m.member.id} className="border-b border-border last:border-b-0">
                  <td className="py-2.5 font-semibold">{m.member.name}</td>
                  <td className="py-2.5 text-right tabular-nums">{decimal(m.units)}</td>
                  <td className="py-2.5 text-right tabular-nums">
                    {decimal(m.netContributions)} €
                  </td>
                  <td className="py-2.5 text-right tabular-nums">{decimal(m.value)} €</td>
                  <td
                    className={cn(
                      'py-2.5 text-right font-bold tabular-nums',
                      isNegative(m.pnl) ? 'text-down' : 'text-up',
                    )}
                  >
                    {signed(m.pnl)} €
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="glass min-w-0 rounded-[20px] p-4">
        <h2 className="text-[14px] font-bold">Historial</h2>
        <p className="mb-3 text-[11px] text-muted-foreground">
          Movimientos ya liquidados. Las cuentas de cada miembro salen de este registro, que no se
          modifica ni se borra.
        </p>
        <div className="overflow-x-auto">
          <table className="w-full min-w-[520px] text-[12px]">
            <thead>
              <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
                <th className="py-2 font-bold">Liquidación</th>
                <th className="py-2 font-bold">Miembro</th>
                <th className="py-2 font-bold">Tipo</th>
                <th className="py-2 text-right font-bold">Monto</th>
                <th className="py-2 text-right font-bold">Unidades</th>
                <th className="py-2 text-right font-bold">NAV</th>
              </tr>
            </thead>
            <tbody>
              {[...FUND.history].reverse().map((mv) => (
                <tr key={mv.id} className="border-b border-border last:border-b-0">
                  <td className="py-2 text-muted-foreground">{dateTimeUtc(mv.settled_at)}</td>
                  <td className="py-2 font-semibold">
                    {MEMBERS.find((m) => m.id === mv.member_id)?.name ?? mv.member_id}
                  </td>
                  <td className={mv.kind === 'contribution' ? 'py-2 text-up' : 'py-2 text-down'}>
                    {mv.kind === 'contribution' ? 'aporte' : 'retiro'}
                  </td>
                  <td className="py-2 text-right tabular-nums">
                    {decimal(mv.amount)} {mv.currency === 'EUR' ? '€' : mv.currency}
                  </td>
                  <td className="py-2 text-right tabular-nums">{decimal(mv.units)}</td>
                  <td className="py-2 text-right tabular-nums">{decimal(mv.nav)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="glass rounded-[20px] p-4">
        <h2 className="text-[14px] font-bold">Movimientos pendientes</h2>
        <p className="mb-3 text-[11px] text-muted-foreground">
          Se hacen el lunes, antes de la apertura europea, al valor de la unidad del cierre del
          viernes.
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
              <span className="ml-auto text-muted-foreground">{dateTimeUtc(mv.at)}</span>
            </li>
          ))}
        </ul>
      </section>
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
      <label className="glass flex items-center gap-2 rounded-full px-4 py-2.5">
        <Search className="size-4 text-muted-foreground" aria-hidden="true" />
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Buscar un término (por ejemplo: sweep, margen, R)"
          aria-label="Buscar en el glosario"
          className="w-full bg-transparent text-[14px] outline-none placeholder:text-muted-foreground"
        />
      </label>
      <p className="text-[11px] text-muted-foreground" aria-live="polite">
        {found.length} de {GLOSSARY.length} términos · fuente: docs/GLOSARIO.md
      </p>
      <div className="grid gap-3 md:grid-cols-2">
        {found.map((e) => (
          <article key={e.id} id={`termino-${e.id}`} className="glass rounded-[20px] p-4">
            <h2 className="text-[15px] font-bold">{e.title}</h2>
            <p className="mt-1 text-[12.5px] leading-relaxed whitespace-pre-line">{e.body}</p>
            {e.example && (
              <p className="mt-2 text-[12px] leading-relaxed whitespace-pre-line text-muted-foreground">
                <em className="font-semibold not-italic">Ejemplo: </em>
                {e.example}
              </p>
            )}
          </article>
        ))}
      </div>
    </div>
  )
}
