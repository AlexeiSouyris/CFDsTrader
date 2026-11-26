/**
 * El informe del laboratorio (`lab_report.v1`, #778, §3 del diseño). Lo calcula **todo** el motor:
 * la UI lo muestra tal cual, sin cuentas (CLAUDE.md §1). Los gráficos solo ubican en pantalla los
 * números que llegan (capital, caída, conteos del histograma).
 *
 * Cada informe exploratorio dice **"exploratorio, no vota"**: ninguna cifra cuenta para el N del
 * proyecto. Son resultados del pasado en simulación, con su incertidumbre: nunca una promesa.
 */
import { type ReactNode, useState } from 'react'

import { ScrollRegion } from '@/components/ui/scroll-region'
import { Segmented } from '@/components/ui/segmented'
import { decimal, percent } from '@/lib/format'
import type { LabReport, Quantiles, Slice, Summary } from '@/lib/lab-contracts'
import { useLabReports } from '@/lib/lab-api'
import { dayYear, numOrDash, pctOrDash, signedPct, tone } from '@/lib/run-format'
import { cn } from '@/lib/utils'

const r = (f: string | null | undefined, digits = 2) =>
  f == null ? '—' : `${decimal(f, digits)} R`
const sPct = (f: string | null | undefined) => (f == null ? '—' : signedPct(f))

function Block({
  title,
  kicker,
  children,
}: {
  title: string
  kicker?: string
  children: ReactNode
}) {
  return (
    <section className="grid min-w-0 content-start gap-2 rounded-sm border border-border bg-card p-3">
      <header>
        {kicker && (
          <p className="text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
            {kicker}
          </p>
        )}
        <h4 className="text-[14px] font-semibold">{title}</h4>
      </header>
      {children}
    </section>
  )
}

function Stats({ items }: { items: [ReactNode, ReactNode, string?][] }) {
  return (
    <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[12.5px]">
      {items.map(([k, v, cls], i) => (
        <div key={i} className="min-w-0">
          <dt className="text-[11px] text-muted-foreground">{k}</dt>
          <dd className={cn('font-semibold tabular-nums', cls)}>{v}</dd>
        </div>
      ))}
    </dl>
  )
}

const W = 600
const H = 150
const DD = 60

/** Capital mensual (desde 1) y caída desde el máximo, tal como los manda el motor. */
function EquityChart({ months }: { months: LabReport['months'] }) {
  if (months.length < 2)
    return <p className="text-[12px] text-muted-foreground">Pocos meses para dibujar.</p>
  const eq = months.map((m) => Number(m.equity))
  const dd = months.map((m) => Number(m.drawdown))
  const lo = Math.min(...eq, 1)
  const hi = Math.max(...eq, 1)
  const span = hi - lo || 1
  const worst = Math.max(...dd) || 1
  const x = (i: number) => (i / (months.length - 1)) * W
  const y = (v: number) => H - ((v - lo) / span) * (H - 8) - 4
  const line = eq.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(' ')
  const area = `0,0 ${dd.map((v, i) => `${x(i).toFixed(1)},${((v / worst) * DD).toFixed(1)}`).join(' ')} ${W},0`
  return (
    <figure className="grid gap-1">
      <svg
        viewBox={`0 0 ${W} ${H + DD + 12}`}
        role="img"
        aria-label={`Capital mensual de ${months[0].month} a ${months[months.length - 1].month}, con la caída desde el máximo debajo`}
        className="h-auto w-full"
        preserveAspectRatio="none"
      >
        <line x1={0} x2={W} y1={y(1)} y2={y(1)} stroke="var(--border)" strokeDasharray="3 4" />
        <polyline
          points={line}
          fill="none"
          stroke="var(--accent)"
          strokeWidth={2}
          vectorEffect="non-scaling-stroke"
        />
        <g transform={`translate(0 ${H + 12})`}>
          <polygon points={area} fill="var(--chart-down)" fillOpacity={0.35} />
          <line x1={0} x2={W} y1={0} y2={0} stroke="var(--border)" />
        </g>
      </svg>
      <figcaption className="flex flex-wrap justify-between gap-2 text-[11px] text-muted-foreground tabular-nums">
        <span>
          {months[0].month} → {months[months.length - 1].month}
        </span>
        <span>
          capital final {decimal(months[months.length - 1].equity, 4)} (desde 1) · abajo, la caída
          desde el máximo
        </span>
      </figcaption>
    </figure>
  )
}

/** Histograma del R neto (y el bruto al lado), con los conteos del motor. */
function RHistogram({ dist }: { dist: LabReport['r_distribution'] }) {
  if (dist.bins.length === 0)
    return <p className="text-[12px] text-muted-foreground">Sin operaciones.</p>
  const top = Math.max(...dist.bins.flatMap((b) => [b.net, b.gross]), 1)
  const bw = W / dist.bins.length
  return (
    <figure className="grid gap-1">
      <svg
        viewBox={`0 0 ${W} 130`}
        role="img"
        aria-label="Histograma del R neto y bruto por operación"
        className="h-auto w-full"
      >
        {dist.bins.map((b, i) => {
          const hn = (b.net / top) * 110
          const hg = (b.gross / top) * 110
          return (
            <g key={b.from} transform={`translate(${i * bw} 0)`}>
              <rect
                x={bw * 0.12}
                y={115 - hg}
                width={bw * 0.36}
                height={hg}
                fill="var(--muted-foreground)"
                fillOpacity={0.35}
              />
              <rect x={bw * 0.5} y={115 - hn} width={bw * 0.36} height={hn} fill="var(--accent)" />
              <text
                x={bw / 2}
                y={128}
                textAnchor="middle"
                fontSize={10}
                fill="var(--muted-foreground)"
              >
                {decimal(b.from, 1)}
              </text>
            </g>
          )
        })}
      </svg>
      <figcaption className="text-[11px] text-muted-foreground">
        Tramos de {decimal(dist.step, 2)} R desde su borde izquierdo. Barra de color: neto (con
        costos); gris: bruto (a precio medio).
      </figcaption>
    </figure>
  )
}

function SummaryRow({ name, s, fmt }: { name: string; s: Summary; fmt: (v: string) => string }) {
  return (
    <tr className="border-t border-border/60">
      <th scope="row" className="py-1 pr-2 text-left font-normal">
        {name}
      </th>
      {[s.p5, s.p50, s.p95, s.mean].map((v, i) => (
        <td key={i} className="py-1 pl-2 text-right whitespace-nowrap tabular-nums">
          {fmt(v)}
        </td>
      ))}
    </tr>
  )
}

function QuantRow({ name, q }: { name: string; q: Quantiles | null }) {
  return (
    <tr className="border-t border-border/60">
      <th scope="row" className="py-1 pr-2 text-left font-normal">
        {name}
      </th>
      {q ? (
        [q.p25, q.p50, q.p75, q.p90].map((v, i) => (
          <td key={i} className="py-1 pl-2 text-right whitespace-nowrap tabular-nums">
            {r(v)}
          </td>
        ))
      ) : (
        <td colSpan={4} className="py-1 pl-2 text-right text-muted-foreground">
          —
        </td>
      )}
    </tr>
  )
}

function SliceTable({ title, rows }: { title: string; rows: Slice[] }) {
  return (
    <table aria-label={title} className="w-full text-[12px]">
      <caption className="pb-1 text-left text-[11px] font-bold text-muted-foreground uppercase">
        {title}
      </caption>
      <thead className="text-[10.5px] text-muted-foreground">
        <tr>
          <th className="text-left">Corte</th>
          <th className="text-right">Op.</th>
          <th className="text-right">R neto medio</th>
          <th className="text-right">Aciertos</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((s) => (
          <tr key={s.key} className="border-t border-border/60">
            <th scope="row" className="py-1 text-left font-normal">
              {s.key}
            </th>
            <td className="py-1 text-right tabular-nums">{s.trades}</td>
            <td className={cn('py-1 text-right tabular-nums', tone(s.mean_r_net))}>
              {r(s.mean_r_net)}
            </td>
            <td className="py-1 text-right tabular-nums">{pctOrDash(s.win_rate)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

export function LabReportView({ report: x }: { report: LabReport }) {
  const e = x.expectancy
  const mc = x.monte_carlo
  const ci = e?.mean_r_net_ci
  return (
    <article
      aria-label={`Informe ${x.report_id.slice(0, 12)}`}
      className="grid min-w-0 gap-3"
      data-report-id={x.report_id}
    >
      <header className="grid gap-1 rounded-sm border border-accent/50 bg-card p-3">
        <p data-lab-label className="text-[12px] font-bold tracking-[0.08em] text-accent uppercase">
          {x.label}
          {!x.votes && ' · ninguna cifra vota'}
        </p>
        <p className="text-[13px]">
          <strong>{x.instrument_id}</strong> · {x.spec.market.timeframe} · {dayYear(x.period.from)}{' '}
          – {dayYear(x.period.to)} · {x.trades.count} operaciones ({x.trades.long} largas,{' '}
          {x.trades.short} cortas) · costos × {decimal(x.cost_multiplier, 2)}
        </p>
        <p className="text-[11.5px] text-muted-foreground">
          Fuera de muestra desde {dayYear(x.holdout_start)}: no se cargó ni se evaluó.
        </p>
      </header>

      <div className="grid gap-3 lg:grid-cols-2">
        <Block kicker="Curva y caída" title="Capital mensual">
          <EquityChart months={x.months} />
        </Block>
        <Block kicker="Distribución de R" title="R por operación">
          <RHistogram dist={x.r_distribution} />
        </Block>

        <Block kicker="Esperanza" title="R medio e intervalos">
          {e ? (
            <Stats
              items={[
                ['R neto medio', r(e.mean_r_net, 3), tone(e.mean_r_net)],
                [
                  `IC ${percent(e.confidence, 0)} %`,
                  ci ? `${decimal(ci.low, 3)} a ${decimal(ci.high, 3)} R` : '—',
                ],
                ['R bruto medio', r(e.mean_r_gross, 3)],
                [
                  'Aciertos',
                  `${pctOrDash(e.win_rate)} (${pctOrDash(e.win_rate_ci.low)} a ${pctOrDash(e.win_rate_ci.high)}, Wilson)`,
                ],
                ['Profit factor', numOrDash(e.profit_factor)],
              ]}
            />
          ) : (
            <p className="text-[12px] text-muted-foreground">
              Sin operaciones: no hay esperanza que medir.
            </p>
          )}
        </Block>

        <Block kicker="Sharpe y sobreajuste" title="Cuánto se probó">
          <Stats
            items={[
              ['Sharpe mensual', numOrDash(x.sharpe.sharpe)],
              ['Sortino mensual', numOrDash(x.sharpe.sortino)],
              ['Sharpe deflactado', numOrDash(x.sharpe.deflated, 3)],
              [
                'N de pruebas',
                `${x.sharpe.trials} (proyecto ${x.sharpe.project_trials} + familia ${x.sharpe.family_variants})`,
              ],
              ['PBO', x.pbo ? pctOrDash(x.pbo.pbo) : '—'],
              ['Variantes de la familia', `probaste ${x.sharpe.family_variants}`],
            ]}
          />
          {!x.pbo && (
            <p className="text-[11.5px] text-muted-foreground">
              El PBO aparece desde 4 variantes de la familia corridas juntas.
            </p>
          )}
        </Block>

        <Block kicker="Monte Carlo" title={mc ? `Bandas de ${mc.paths} caminos` : 'Bandas'}>
          {mc ? (
            <>
              <ScrollRegion label="Bandas de Monte Carlo">
                <table aria-label="Bandas de Monte Carlo" className="w-full text-[12px]">
                  <thead className="text-[10.5px] text-muted-foreground">
                    <tr>
                      <th className="text-left">Medida</th>
                      <th className="text-right">p5</th>
                      <th className="text-right">p50</th>
                      <th className="text-right">p95</th>
                      <th className="text-right">media</th>
                    </tr>
                  </thead>
                  <tbody>
                    <SummaryRow
                      name="Retorno final"
                      s={mc.final_return}
                      fmt={(v) => signedPct(v)}
                    />
                    <SummaryRow name="Caída máxima" s={mc.max_drawdown} fmt={(v) => pctOrDash(v)} />
                    <SummaryRow
                      name="Racha perdedora"
                      s={mc.longest_losing_streak}
                      fmt={(v) => decimal(v, 1)}
                    />
                    <SummaryRow
                      name="Bajo el agua (op.)"
                      s={mc.longest_under_water}
                      fmt={(v) => decimal(v, 0)}
                    />
                  </tbody>
                </table>
              </ScrollRegion>
              <Stats
                items={[
                  ['Riesgo de ruina', pctOrDash(mc.risk_of_ruin, 2)],
                  ['Caída observada', pctOrDash(mc.observed_max_drawdown)],
                ]}
              />
            </>
          ) : (
            <p className="text-[12px] text-muted-foreground">Sin operaciones.</p>
          )}
        </Block>

        <Block kicker="Test de suerte" title="Contra entrar al azar">
          {x.luck ? (
            <Stats
              items={[
                ['Percentil del R medio', `${decimal(x.luck.percentile_mean_r, 1)} %`],
                ['Percentil del total', `${decimal(x.luck.percentile_total, 1)} %`],
                ['Entradas al azar', `${x.luck.simulations} simulaciones`],
                ['Mismas salidas y tamaño', 'sí'],
              ]}
            />
          ) : (
            <p className="text-[12px] text-muted-foreground">Sin operaciones.</p>
          )}
        </Block>

        <Block kicker="Robustez" title="Mitades y costos más altos">
          <Stats
            items={[
              [
                '1.ª mitad (retorno mensual medio)',
                x.halves ? sPct(x.halves.first) : '—',
                x.halves ? tone(x.halves.first) : undefined,
              ],
              [
                '2.ª mitad',
                x.halves ? sPct(x.halves.second) : '—',
                x.halves ? tone(x.halves.second) : undefined,
              ],
              [
                `Costos × ${decimal(x.cost_stress.stress, 2)}: R neto medio`,
                r(x.cost_stress.mean_r_net, 3),
              ],
              ['Con ese estrés: Sharpe mensual', numOrDash(x.cost_stress.sharpe)],
            ]}
          />
        </Block>

        <Block kicker="Comparación" title="Comprar y mantener">
          {x.buy_and_hold ? (
            <Stats
              items={[
                [
                  'Retorno total',
                  sPct(x.buy_and_hold.total_return),
                  tone(x.buy_and_hold.total_return),
                ],
                ['Caída máxima', pctOrDash(x.buy_and_hold.max_drawdown)],
                ['Sharpe mensual', numOrDash(x.buy_and_hold.sharpe)],
                ['Retorno mensual medio', sPct(x.buy_and_hold.monthly_mean)],
              ]}
            />
          ) : (
            <p className="text-[12px] text-muted-foreground">
              No se pudo calcular en este período.
            </p>
          )}
        </Block>

        <Block kicker="Excursiones" title="MAE y MFE">
          <ScrollRegion label="MAE y MFE">
            <table aria-label="MAE y MFE en R" className="w-full text-[12px]">
              <thead className="text-[10.5px] text-muted-foreground">
                <tr>
                  <th className="text-left">Medida</th>
                  <th className="text-right">p25</th>
                  <th className="text-right">p50</th>
                  <th className="text-right">p75</th>
                  <th className="text-right">p90</th>
                </tr>
              </thead>
              <tbody>
                <QuantRow name="MAE (llegó a perder)" q={x.excursions.mae_r} />
                <QuantRow name="MFE (llegó a ganar)" q={x.excursions.mfe_r} />
                <QuantRow name="MFE de las perdedoras" q={x.excursions.mfe_r_losers} />
              </tbody>
            </table>
          </ScrollRegion>
        </Block>

        <Block kicker="Borde bruto" title="¿Paga los costos?">
          <Stats
            items={[
              ['Costo por operación (C)', r(x.edge.cost_per_trade_r, 3)],
              [
                `Barra k × C (k = ${decimal(x.edge.edge_cost_factor, 1)})`,
                r(x.edge.required_gross_r, 3),
              ],
              ['Borde bruto medido', r(x.edge.measured_gross_r, 3)],
            ]}
          />
          <p className="text-[11.5px] text-muted-foreground">
            En exploratorio el borde medido es un resultado: no vale como estimación previa.
          </p>
        </Block>
      </div>

      <Block kicker="Walk-forward" title="Por ventana de validación">
        <ScrollRegion label="Ventanas walk-forward">
          <table
            aria-label="Ventanas walk-forward del informe"
            className="w-full min-w-[360px] text-[12px]"
          >
            <thead className="text-[10.5px] text-muted-foreground">
              <tr>
                <th className="text-left">Ventana</th>
                <th className="text-right">Op.</th>
                <th className="text-right">Retorno</th>
              </tr>
            </thead>
            <tbody>
              {x.walk_forward.map((w) => (
                <tr key={w.from} className="border-t border-border/60">
                  <th scope="row" className="py-1 text-left font-normal">
                    {dayYear(w.from)} – {dayYear(w.to)}
                  </th>
                  <td className="py-1 text-right tabular-nums">{w.trades}</td>
                  <td className={cn('py-1 text-right tabular-nums', tone(w.return))}>
                    {signedPct(w.return)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </ScrollRegion>
      </Block>

      <details className="rounded-sm border border-border bg-card p-3">
        <summary className="cursor-pointer text-[13px] font-semibold">
          Cortes por año, hora y lado (mirar cortes es probar más)
        </summary>
        <div className="mt-3 grid gap-4 md:grid-cols-3">
          <SliceTable title="Por año" rows={x.slices.by_year} />
          <SliceTable title="Por hora UTC" rows={x.slices.by_hour_utc} />
          <SliceTable title="Por lado" rows={x.slices.by_side} />
        </div>
      </details>

      <Block kicker="Operaciones" title="Salidas y rechazos del riesgo">
        <Stats
          items={[
            ['Stop', `${x.trades.exits.stop} (con gap ${x.trades.exits.stop_gap})`],
            ['Objetivo', String(x.trades.exits.target)],
            ['Salida planificada', String(x.trades.exits.planned)],
            ['Cierre forzado', String(x.trades.exits.force_close)],
            [
              'Rechazadas por el riesgo',
              x.trades.rejected.length === 0
                ? 'ninguna'
                : x.trades.rejected.map((j) => `${j.reason} ${j.count}`).join(', '),
            ],
            ['Entrada del lado del stop', String(x.trades.through_stop)],
          ]}
        />
      </Block>

      {x.notes.length > 0 && (
        <ul aria-label="Avisos del motor" className="grid gap-1 text-[12px] text-muted-foreground">
          {x.notes.map((n) => (
            <li key={n}>· {n}</li>
          ))}
        </ul>
      )}
      <p className="text-[11px] text-muted-foreground tabular-nums">
        Informe <code>{x.report_id.slice(0, 12)}</code> · spec{' '}
        <code>{x.spec_sha256.slice(0, 12)}</code> · código{' '}
        {x.reproducibility.code_version ?? 'desconocido'} · última vela{' '}
        {dayYear(x.reproducibility.last_candle)} · semilla {x.reproducibility.seed}. Misma spec,
        mismos datos y misma semilla: mismo informe.
      </p>
    </article>
  )
}

/** Los informes de una corrida terminada, uno por variante. */
export function LabReports({ ids }: { ids: string[] }) {
  const reports = useLabReports(ids)
  const [pick, setPick] = useState(0)
  if (reports.status === 'loading')
    return <p className="text-[13px] text-muted-foreground">Cargando el informe…</p>
  if (reports.status === 'error')
    return (
      <p role="alert" className="text-[13px] text-down">
        No se pudo leer el informe: {reports.message}
      </p>
    )
  const list = reports.data
  const at = Math.min(pick, list.length - 1)
  return (
    <div className="grid min-w-0 gap-3">
      {list.length > 1 && (
        <div className="overflow-x-auto">
          <Segmented
            label="Variante del informe"
            options={list.map((_, i) => String(i))}
            value={String(at)}
            format={(o) => `Variante ${Number(o) + 1}`}
            onChange={(o) => setPick(Number(o))}
          />
        </div>
      )}
      <LabReportView report={list[at]} />
    </div>
  )
}
