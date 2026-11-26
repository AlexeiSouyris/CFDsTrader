/**
 * Simulador de varianza del laboratorio (#806, `docs/design/2026-11-laboratorio.md` §4.2): con la
 * misma esperanza, cuánto puede variar el capital por pura suerte. Miles de caminos desde un win
 * rate, el R medio ganador y perdedor y el riesgo por operación (o los R reales de un informe del
 * laboratorio), con la incertidumbre del win rate.
 *
 * **La UI no hace cuentas** (CLAUDE.md §1): bandas, caídas, rachas y ruina las calcula el motor
 * (`POST /api/v1/lab/variance`, `research::variance`). La UI arma el pedido y dibuja. Educativo:
 * no predice resultados ni es consejo de inversión.
 */
import { useId, useRef, useState } from 'react'

import { FanChart } from '@/components/ui/fan-chart'
import { Segmented } from '@/components/ui/segmented'
import { decimal, percent } from '@/lib/format'
import { runVariance } from '@/lib/sim-api'
import type { VarianceReport } from '@/lib/sim-contracts'
import { ESMA_URL, type VarianceForm as Form, toVarianceRequest } from '@/lib/sim-form'
import { cn } from '@/lib/utils'

const field =
  'mt-1 block min-h-10 w-full min-w-0 rounded-sm border border-border bg-surface px-2.5 py-1.5 text-[13px] tabular-nums outline-none transition-colors hover:border-foreground/30 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40'
const label =
  'block min-w-0 text-[11px] font-bold tracking-[0.06em] text-muted-foreground uppercase'

const times = (s: string) => `×${decimal(s, 2)}`
const pct = (s: string) => `${percent(s, 1)} %`

function Stats({ items }: { items: [string, string, string?][] }) {
  return (
    <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[12.5px] sm:grid-cols-3">
      {items.map(([k, v, cls]) => (
        <div key={k} className="min-w-0">
          <dt className="text-[11px] text-muted-foreground">{k}</dt>
          <dd className={cn('font-semibold tabular-nums', cls)}>{v}</dd>
        </div>
      ))}
    </dl>
  )
}

/** Las rachas que se muestran: las que dicen algo (de 3 en adelante). */
const STREAKS = [3, 5, 7, 10, 15, 20]

export function VarianceResult({ r }: { r: VarianceReport }) {
  const f = r.final_equity
  const ruinLevel = 1 - Number(r.ruin_threshold_pct) / 100
  const streaks = r.losing_streak_at_least.filter((s) => STREAKS.includes(s.k))
  const top = Math.max(...r.max_drawdown.histogram.map((b) => Number(b.share)), 0.0001)
  return (
    <div className="grid gap-3" data-variance-result>
      <Stats
        items={[
          ['Esperanza por operación', `${decimal(r.expectancy_r, 3)} R`],
          [
            'Riesgo de ruina',
            `${pct(r.risk_of_ruin.probability)} (${percent(r.risk_of_ruin.low, 1)}–${percent(r.risk_of_ruin.high, 1)} %)`,
            'text-down',
          ],
          ['Capital final, mediana', times(f.p50)],
          ['Capital final, 90 % de los caminos', `${times(f.p5)} a ${times(f.p95)}`],
          [
            'Caída máxima, mediana (p95)',
            `${pct(r.max_drawdown.summary.p50)} (${pct(r.max_drawdown.summary.p95)})`,
          ],
          [
            'Racha perdedora más larga, mediana (p95)',
            `${decimal(r.longest_losing_streak.p50, 0)} (${decimal(r.longest_losing_streak.p95, 0)})`,
          ],
        ]}
      />
      <FanChart
        bands={r.bands}
        ruinLevel={ruinLevel > 0 ? ruinLevel : undefined}
        label={`Bandas de capital de ${r.paths} caminos de ${r.trades} operaciones: mediana final ${times(f.p50)}, 90 % entre ${times(f.p5)} y ${times(f.p95)}`}
      />
      {r.fixed_win_rate && r.win_rate_draws && (
        <p className="rounded-sm border border-info/40 bg-info/10 p-2 text-[12.5px] text-pretty">
          <strong>Incertidumbre del win rate:</strong> con{' '}
          {r.source.kind === 'model' && r.source.observed_trades} operaciones observadas, el win
          rate de verdad puede estar entre {pct(r.win_rate_draws.p5)} y {pct(r.win_rate_draws.p95)}{' '}
          (90 % de los sorteos). Con eso la ruina es{' '}
          <strong>{pct(r.risk_of_ruin.probability)}</strong>; tomando el win rate como cierto sería{' '}
          {pct(r.fixed_win_rate.risk_of_ruin)}.
        </p>
      )}
      <div className="grid gap-3 sm:grid-cols-2">
        <section className="grid content-start gap-1.5">
          <h4 className="text-[12.5px] font-semibold">Rachas de pérdidas seguidas</h4>
          <ul className="grid gap-1 text-[12px] tabular-nums">
            {streaks.map((s) => (
              <li key={s.k} className="flex items-center gap-2">
                <span className="w-24 shrink-0 text-muted-foreground">{s.k} o más</span>
                <span className="h-2 min-w-0 flex-1 rounded-xs bg-border">
                  <span
                    className="block h-2 rounded-xs bg-down"
                    style={{ width: `${Number(s.probability) * 100}%` }}
                  />
                </span>
                <span className="w-14 shrink-0 text-right">{pct(s.probability)}</span>
              </li>
            ))}
          </ul>
          <p className="text-[11px] text-muted-foreground">
            Probabilidad de ver al menos una racha así en {r.trades} operaciones.
          </p>
        </section>
        <section className="grid content-start gap-1.5">
          <h4 className="text-[12.5px] font-semibold">Caída máxima de cada camino</h4>
          <svg
            viewBox="0 0 200 60"
            role="img"
            aria-label="Histograma de la caída máxima de los caminos"
            className="h-auto w-full"
          >
            {r.max_drawdown.histogram.map((b, i, all) => {
              const w = 200 / all.length
              const h = (Number(b.share) / top) * 52
              return (
                <rect
                  key={b.from}
                  x={i * w + 0.5}
                  y={56 - h}
                  width={w - 1}
                  height={h}
                  fill="var(--chart-down)"
                  fillOpacity={0.7}
                />
              )
            })}
            <line x1={0} x2={200} y1={56} y2={56} stroke="var(--border)" />
          </svg>
          <p className="flex justify-between text-[11px] text-muted-foreground tabular-nums">
            <span>0 %</span>
            <span>tramos de {percent(r.max_drawdown.histogram[0]?.to ?? '0', 0)} %</span>
            <span>100 %</span>
          </p>
        </section>
      </div>
      <ul className="grid gap-0.5 text-[11.5px] text-muted-foreground">
        {r.notes.map((n) => (
          <li key={n}>· {n}</li>
        ))}
      </ul>
      <p className="text-[11px] text-muted-foreground tabular-nums">
        {r.paths} caminos · semilla {r.seed} · ruina = caer {decimal(r.ruin_threshold_pct)} % desde
        el capital inicial
      </p>
    </div>
  )
}

export function VarianceSimulator({ reports = [] }: { reports?: string[] }) {
  const id = useId()
  const [form, setForm] = useState<Form>({
    source: 'model',
    win_rate: '',
    avg_win_r: '',
    avg_loss_r: '1',
    uncertain: true,
    observed: '',
    report: '',
    risk_pct: '',
    trades: '',
    paths: '',
    seed: '',
    ruin: '',
  })
  const [state, setState] = useState<
    | { status: 'idle' }
    | { status: 'running' }
    | { status: 'ok'; r: VarianceReport }
    | { status: 'error'; message: string }
  >({ status: 'idle' })
  const ctrl = useRef<AbortController | null>(null)
  const set = (patch: Partial<Form>) => setForm((f) => ({ ...f, ...patch }))
  // Los informes llegan cuando termina una corrida: si el elegido ya no está, el primero.
  const report = reports.includes(form.report) ? form.report : (reports[0] ?? '')
  const source = reports.length > 0 ? form.source : 'model'
  const req = toVarianceRequest({ ...form, report, source })
  const run = () => {
    if (!req) return
    ctrl.current?.abort()
    const c = new AbortController()
    ctrl.current = c
    setState({ status: 'running' })
    runVariance(req, c.signal).then(
      (r) => setState({ status: 'ok', r }),
      (e: unknown) => {
        if (!c.signal.aborted)
          setState({ status: 'error', message: e instanceof Error ? e.message : String(e) })
      },
    )
  }
  const model = source === 'model'
  return (
    <section
      id="simulador-varianza"
      aria-labelledby={`${id}-title`}
      className="grid min-w-0 gap-3 rounded-sm border border-border bg-card p-3"
    >
      <header className="grid gap-1">
        <p className="text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
          Sin efectos · simulación educativa
        </p>
        <h3 id={`${id}-title`} className="text-[15px] font-semibold">
          Simulador de varianza
        </h3>
        <p className="max-w-[72ch] text-[12.5px] text-pretty text-muted-foreground">
          Con la misma ventaja, la suerte decide mucho: miles de caminos posibles de tu capital, las
          rachas de pérdidas que vas a ver y la probabilidad de arruinarte. El motor simula; esto no
          predice tu resultado.
        </p>
      </header>
      {reports.length > 0 && (
        <Segmented
          label="De dónde salen los R"
          options={['model', 'report']}
          value={source}
          format={(o) => (o === 'model' ? 'Modelo' : 'R de un informe')}
          onChange={(v) => set({ source: v as Form['source'] })}
        />
      )}
      {model ? (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
          <label className={label}>
            Win rate (%)
            <input
              className={field}
              inputMode="decimal"
              placeholder="por ejemplo 45"
              value={form.win_rate}
              onChange={(e) => set({ win_rate: e.target.value })}
            />
          </label>
          <label className={label}>
            R medio ganador
            <input
              className={field}
              inputMode="decimal"
              placeholder="por ejemplo 1,5"
              value={form.avg_win_r}
              onChange={(e) => set({ avg_win_r: e.target.value })}
            />
          </label>
          <label className={label}>
            R medio perdedor
            <input
              className={field}
              inputMode="decimal"
              value={form.avg_loss_r}
              onChange={(e) => set({ avg_loss_r: e.target.value })}
            />
          </label>
        </div>
      ) : (
        <label className={label}>
          Informe del laboratorio
          <select
            className={field}
            value={report}
            onChange={(e) => set({ report: e.target.value })}
          >
            {reports.map((r, i) => (
              <option key={r} value={r}>
                Variante {i + 1} · {r.slice(0, 12)}
              </option>
            ))}
          </select>
        </label>
      )}
      {model && (
        <div className="flex flex-wrap items-end gap-3">
          <label className="flex min-h-10 items-center gap-2 text-[12.5px]">
            <input
              type="checkbox"
              className="size-4 accent-[var(--accent)]"
              checked={form.uncertain}
              onChange={(e) => set({ uncertain: e.target.checked })}
            />
            El win rate salió de pocas operaciones (incertidumbre)
          </label>
          {form.uncertain && (
            <label className={cn(label, 'w-40')}>
              Operaciones observadas
              <input
                className={field}
                inputMode="numeric"
                placeholder="por ejemplo 40"
                value={form.observed}
                onChange={(e) => set({ observed: e.target.value })}
              />
            </label>
          )}
        </div>
      )}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
        <label className={label}>
          Riesgo (%)
          <input
            className={field}
            inputMode="decimal"
            placeholder="por ejemplo 1"
            value={form.risk_pct}
            onChange={(e) => set({ risk_pct: e.target.value })}
          />
        </label>
        <label className={label}>
          Operaciones
          <input
            className={field}
            inputMode="numeric"
            placeholder="por ejemplo 200"
            value={form.trades}
            onChange={(e) => set({ trades: e.target.value })}
          />
        </label>
        <label className={label}>
          Caminos
          <input
            className={field}
            inputMode="numeric"
            placeholder="motor"
            value={form.paths}
            onChange={(e) => set({ paths: e.target.value })}
          />
        </label>
        <label className={label}>
          Ruina (% de caída)
          <input
            className={field}
            inputMode="decimal"
            placeholder="motor"
            value={form.ruin}
            onChange={(e) => set({ ruin: e.target.value })}
          />
        </label>
        <label className={label}>
          Semilla
          <input
            className={field}
            inputMode="numeric"
            placeholder="motor"
            value={form.seed}
            onChange={(e) => set({ seed: e.target.value })}
          />
        </label>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={run}
          disabled={!req || state.status === 'running'}
          className="min-h-10 rounded-sm bg-primary px-4 text-[13px] font-semibold text-primary-foreground disabled:cursor-not-allowed disabled:opacity-50"
        >
          {state.status === 'running' ? 'Simulando…' : 'Simular'}
        </button>
        {!req && (
          <p className="text-[12px] text-muted-foreground">
            Completá {model ? 'el win rate, los R medios, ' : ''}el riesgo y las operaciones.
          </p>
        )}
      </div>
      <div aria-live="polite" className="grid gap-2">
        {state.status === 'error' && (
          <p role="alert" className="text-[12.5px] text-down">
            {state.message}
          </p>
        )}
        {state.status === 'ok' && <VarianceResult r={state.r} />}
      </div>
      <p className="border-t border-border pt-2 text-[11px] text-pretty text-muted-foreground">
        Para tener en cuenta: en la UE, entre el 74 % y el 89 % de las cuentas minoristas de CFD
        pierden dinero (
        <a className="text-info underline" href={ESMA_URL} target="_blank" rel="noreferrer">
          ESMA, medidas de intervención de 2018
        </a>
        ). No es consejo de inversión.
      </p>
    </section>
  )
}
