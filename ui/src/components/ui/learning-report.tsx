/**
 * "Aprendizaje" (fase 8b, spec §15b, #331): el último `learning_report.v1` tal como lo armó el
 * motor con el diario. Tabla de confiabilidad con el intervalo de Wilson, aporte de cada señal y
 * propuestas de cambio.
 *
 * **No calcula nada** (los porcentajes solo corren la coma en el texto) y **no se vota acá**:
 * cada propuesta se muestra con su estado. Cuando el gateway exponga los votos (#330), llegan por
 * `voteStatus`; votar sigue fuera de la UI (cuentas y 2FA, decisión aparte).
 */
import { ScrollRegion } from '@/components/ui/scroll-region'
import { dateTimeUtc } from '@/lib/format'
import type { LearningReport, Signal, VoteStatus } from '@/lib/learning-api'
import { numOrDash, pctOrDash } from '@/lib/run-format'
import { percent } from '@/lib/format'
import { cn } from '@/lib/utils'

const SIGNAL_LABEL: Record<Signal, string> = {
  trend: 'Tendencia',
  sma_side: 'Lado de la SMA',
  candle_pattern: 'Patrón de velas',
  double_break: 'Ruptura de doble techo o piso',
  level_rejection: 'Rechazo de nivel',
  sweep: 'Sweep',
  fvg_or_order_block: 'FVG u order block',
  recent_choch: 'CHoCH reciente',
  news: 'Noticias',
}

const KIND_LABEL = {
  recalibrate: 'Recalibrar',
  review_signal_weight: 'Revisar el peso de una señal',
} as const

const SOURCE_LABEL = { backtest: 'backtest', paper: 'paper', live: 'real' } as const

const th =
  'py-2 pr-3 text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase'
const td = 'py-2 pr-3 tabular-nums whitespace-nowrap'

function Badge({ children, warn }: { children: string; warn?: boolean }) {
  return (
    <span
      className={cn(
        'inline-block rounded-full border px-2 py-0.5 text-[10.5px] font-bold whitespace-nowrap',
        warn
          ? 'border-[var(--chart-signal)]/50 text-[var(--chart-signal)]'
          : 'border-border text-muted-foreground',
      )}
    >
      {children}
    </span>
  )
}

const PENDING: VoteStatus = { label: 'pendiente de voto' }

export function LearningReportView({
  report: r,
  voteStatus,
}: {
  report: LearningReport
  /** Estado del voto de cada propuesta (#330). Sin él, todas "pendiente de voto". */
  voteStatus?: (changeId: string) => VoteStatus | undefined
}) {
  return (
    <div className="grid gap-5" data-report={r.report_id}>
      <p className="text-[12px] text-muted-foreground">
        {dateTimeUtc(r.from)} → {dateTimeUtc(r.to)} · {r.proposals} propuestas con resultado ·
        fuentes: {r.sources.map((s) => SOURCE_LABEL[s]).join(', ') || '—'}
        {r.calibration.without_probability > 0 &&
          ` · ${r.calibration.without_probability} sin probabilidad mostrada`}
      </p>

      <section>
        <h4 className="mb-2 text-[12.5px] font-bold">
          Calibración: probabilidad mostrada contra la frecuencia real (TP1 antes que el stop)
        </h4>
        <ScrollRegion label="Tabla de confiabilidad">
          <table className="w-full min-w-[560px] text-[12px]" aria-label="Tabla de confiabilidad">
            <thead>
              <tr className="border-b border-border">
                <th className={th}>Rango mostrado</th>
                <th className={th}>Casos</th>
                <th className={th}>Mostrado (promedio)</th>
                <th className={th}>Frecuencia real</th>
                <th className={th}>Intervalo de Wilson</th>
                <th className={th}>
                  <span className="sr-only">Aviso</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {r.calibration.rows.map((row) => (
                <tr
                  key={row.from}
                  data-insufficient={row.insufficient}
                  className={cn(
                    'border-b border-border last:border-b-0',
                    row.insufficient && 'text-muted-foreground',
                  )}
                >
                  <td className={td}>
                    {percent(row.from, 0)}–{percent(row.to, 0)} %
                  </td>
                  <td className={td}>
                    {row.hits}/{row.cases}
                  </td>
                  <td className={td}>{pctOrDash(row.mean_shown)}</td>
                  <td className={td}>{pctOrDash(row.frequency)}</td>
                  <td className={td}>
                    {row.low && row.high ? `${pctOrDash(row.low)} – ${pctOrDash(row.high)}` : '—'}
                  </td>
                  <td className={td}>
                    {row.insufficient && <Badge warn>datos insuficientes</Badge>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </ScrollRegion>
      </section>

      <section>
        <h4 className="mb-2 text-[12.5px] font-bold">Aporte de cada señal (en R)</h4>
        <ScrollRegion label="Aporte de cada señal">
          <table className="w-full min-w-[560px] text-[12px]" aria-label="Aporte de cada señal">
            <thead>
              <tr className="border-b border-border">
                <th className={th}>Señal</th>
                <th className={th}>Con ella (casos, TP1, R medio)</th>
                <th className={th}>Sin ella</th>
                <th className={th}>Diferencia de R</th>
                <th className={th}>
                  <span className="sr-only">Aviso</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {r.signals.map((s) => (
                <tr
                  key={s.signal}
                  data-small-sample={s.small_sample}
                  className="border-b border-border last:border-b-0"
                >
                  <td className={cn(td, 'font-semibold')}>{SIGNAL_LABEL[s.signal]}</td>
                  <td className={td}>
                    {s.with.cases} · {pctOrDash(s.with.hit_rate)} · {numOrDash(s.with.mean_r)} R
                  </td>
                  <td className={td}>
                    {s.without.cases} · {pctOrDash(s.without.hit_rate)} ·{' '}
                    {numOrDash(s.without.mean_r)} R
                  </td>
                  <td className={cn(td, 'font-bold')}>{numOrDash(s.difference_r)} R</td>
                  <td className={td}>{s.small_sample && <Badge warn>muestra chica</Badge>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </ScrollRegion>
      </section>

      <section>
        <h4 className="mb-1 text-[12.5px] font-bold">Propuestas de cambio</h4>
        <p className="mb-2 text-[11px] text-muted-foreground">
          El sistema nunca se cambia solo: cada propuesta pasa por la votación del grupo. Desde acá
          no se vota.
        </p>
        {r.change_proposals.length === 0 ? (
          <p className="text-[12px] text-muted-foreground">El informe no propone cambios.</p>
        ) : (
          <ul className="grid gap-2" aria-label="Propuestas de cambio">
            {r.change_proposals.map((p) => {
              const vote = voteStatus?.(p.change_id) ?? PENDING
              return (
                <li
                  key={p.change_id}
                  data-change={p.change_id}
                  className="grid gap-1 rounded-xl border border-border px-3 py-2 text-[12px]"
                >
                  <span className="flex flex-wrap items-center gap-2">
                    <span className="font-bold">{KIND_LABEL[p.kind]}</span>
                    <Badge warn>{vote.label}</Badge>
                    {vote.detail && (
                      <span className="text-[11px] text-muted-foreground">{vote.detail}</span>
                    )}
                  </span>
                  <span className="text-pretty">{p.text}</span>
                  <span className="text-[11px] text-muted-foreground">
                    Evidencia: <code className="[overflow-wrap:anywhere]">{p.evidence}</code>
                  </span>
                </li>
              )
            })}
          </ul>
        )}
      </section>
    </div>
  )
}
