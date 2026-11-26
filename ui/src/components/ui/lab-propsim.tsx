/**
 * Simulador de cuenta fondeada (#807, `docs/design/2026-11-academia.md` §6): reglas de una prop
 * firm (la plantilla genérica en blanco del motor; ninguna firma real sin su reglamento citado),
 * operaciones del modelo del simulador de varianza o de un informe del laboratorio, y lo que
 * responde el motor (`POST /api/v1/lab/propsim`, `cfds-propsim`): probabilidad de pasar, cobrar,
 * quemar la cuenta o quedarse sin tiempo, cuotas y valor esperado, y la comparación con operar al
 * azar (esperanza 0).
 *
 * **La UI no hace cuentas** (CLAUDE.md §1): arma el pedido y muestra. Educativo: pagar una
 * evaluación real es dinero real y queda fuera de la app; nada de esto es consejo de inversión.
 */
import { type ReactNode, useEffect, useId, useRef, useState } from 'react'

import { ScrollRegion } from '@/components/ui/scroll-region'
import { Segmented } from '@/components/ui/segmented'
import { decimal, percent } from '@/lib/format'
import { takeLabTool } from '@/lib/lab-focus'
import { PropSimRejected, runPropSim, usePropTemplates } from '@/lib/sim-api'
import type { Probability, PropRules, PropSimResult, RuleError, Stage } from '@/lib/sim-contracts'
import {
  ESMA_URL,
  type PhaseForm,
  type PropForm,
  propFormFrom,
  toPropSimRequest,
} from '@/lib/sim-form'
import { cn } from '@/lib/utils'

/** Divulgación de riesgos de Topstep (cifras de 2025, autodeclaradas por la firma). */
const TOPSTEP_URL = 'https://www.topstep.com/risk-disclosure'

const input =
  'mt-1 block min-h-10 w-full min-w-0 rounded-sm border border-border bg-surface px-2.5 py-1.5 text-[13px] tabular-nums outline-none transition-colors hover:border-foreground/30 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40 aria-[invalid=true]:border-down'
const labelCls =
  'block min-w-0 text-[11px] font-bold tracking-[0.06em] text-muted-foreground uppercase'

type Errors = { rules: RuleError[]; bad: string[] }

function Field({
  label,
  value,
  onChange,
  path,
  name,
  errors,
  placeholder,
  numeric = 'decimal',
}: {
  label: string
  value: string
  onChange: (v: string) => void
  /** JSON Pointer dentro de `prop_rules.v1` (para los errores del motor) */
  path?: string
  /** clave del formulario (para "no es un número") */
  name: string
  errors: Errors
  placeholder?: string
  numeric?: 'decimal' | 'numeric' | 'text'
}) {
  const id = useId()
  const engine = path
    ? errors.rules.find((e) => e.path === path || (path === '/day/timezone' && e.path === '/day'))
    : undefined
  const message = errors.bad.includes(name) ? 'No es un número.' : engine?.message
  return (
    <div className="min-w-0">
      <label htmlFor={id} className={labelCls}>
        {label}
      </label>
      <input
        id={id}
        className={input}
        inputMode={numeric === 'text' ? undefined : numeric}
        placeholder={placeholder}
        value={value}
        aria-invalid={message ? true : undefined}
        aria-describedby={message ? `${id}-err` : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
      {message && (
        <span id={`${id}-err`} className="mt-0.5 block text-[11px] text-down">
          {message}
        </span>
      )}
    </div>
  )
}

function Toggle({
  checked,
  onChange,
  children,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  children: ReactNode
}) {
  return (
    <label className="flex min-h-10 items-center gap-2 text-[12.5px]">
      <input
        type="checkbox"
        className="size-4 accent-[var(--accent)]"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      {children}
    </label>
  )
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <fieldset className="grid min-w-0 gap-2 rounded-sm border border-border p-2.5">
      <legend className="px-1 text-[12px] font-semibold">{title}</legend>
      {children}
    </fieldset>
  )
}

const pct = (s: string) => `${percent(s, 1)} %`
const prob = (p: Probability) =>
  `${pct(p.probability)} (${percent(p.low, 1)}–${percent(p.high, 1)} %)`
const money = (s: string | null, cur: string) => (s == null ? '—' : `${decimal(s, 2)} ${cur}`)

const STATUS: Record<Stage['status'], string> = {
  passed: 'Pasó',
  payout: 'Cobró',
  failed: 'Falló',
  timeout: 'Se acabó el tiempo',
  unfinished: 'Sin terminar',
  not_reached: 'No llegó',
}
const RULE = { daily_loss: 'pérdida diaria', max_drawdown: 'caída máxima' } as const

function Outcomes({ r }: { r: NonNullable<PropSimResult['monte_carlo']> }) {
  const a = r.per_attempt
  const win = a.payout ?? a.pass_evaluation
  const parts: [string, string, string][] = [
    [a.payout ? 'Cobró' : 'Pasó', win.probability, 'bg-up'],
    ['Quemó la cuenta', a.failed.probability, 'bg-down'],
    ['Sin tiempo', a.timeout.probability, 'bg-info'],
    ['Sin terminar', a.unfinished.probability, 'bg-muted-foreground'],
  ]
  return (
    <figure className="grid gap-1.5">
      <div
        role="img"
        aria-label={`Cómo terminó cada intento: ${parts.map(([k, v]) => `${k} ${pct(v)}`).join(', ')}`}
        className="flex h-3 w-full overflow-hidden rounded-xs bg-border"
      >
        {parts.map(([k, v, cls]) => (
          <span key={k} className={cls} style={{ width: `${Number(v) * 100}%` }} />
        ))}
      </div>
      <figcaption className="flex flex-wrap gap-x-4 gap-y-1 text-[11.5px] text-muted-foreground tabular-nums">
        {parts.map(([k, v, cls]) => (
          <span key={k} className="flex items-center gap-1.5">
            <span aria-hidden className={cn('inline-block size-2 rounded-xs', cls)} />
            {k} {pct(v)}
          </span>
        ))}
      </figcaption>
    </figure>
  )
}

export function PropSimResultView({ r }: { r: PropSimResult }) {
  const cur = r.rules.currency ?? ''
  const mc = r.monte_carlo
  const funded = r.rules.funded !== null
  return (
    <div className="grid gap-3" data-propsim-result>
      {r.upper_bound && (
        <p role="note" className="rounded-sm border border-info/40 bg-info/10 p-2 text-[12.5px]">
          <strong>Tope superior:</strong> la regla {r.unsimulable.join(', ')} no se pudo simular (se
          dejó afuera): pasar es, como mucho, lo que se ve acá.
        </p>
      )}
      {mc && (
        <>
          <p className="text-[13.5px] text-pretty">
            Con estas reglas y estas operaciones, en el pasado simulado{' '}
            <strong className="tabular-nums">
              {pct((mc.per_attempt.payout ?? mc.per_attempt.pass_evaluation).probability)}
            </strong>{' '}
            de los intentos {funded ? 'llegaron al primer cobro' : 'pasaron la evaluación'}.
            Operando al azar (esperanza 0 después de costos):{' '}
            <strong className="tabular-nums">
              {pct((mc.zero_edge.payout ?? mc.zero_edge.pass_evaluation).probability)}
            </strong>
            .
          </p>
          <p
            className={cn(
              'rounded-sm border p-2 text-[12.5px] text-pretty',
              mc.beats_zero_edge ? 'border-border' : 'border-down/50 bg-down/10',
            )}
            data-beats={mc.beats_zero_edge}
          >
            {mc.beats_zero_edge
              ? 'Con estos supuestos le gana al azar: los intervalos no se tocan. Igual depende de que tus operaciones del futuro se parezcan a estas.'
              : 'No se distingue del azar: la diferencia con operar sin ventaja puede ser pura suerte.'}
          </p>
          <Outcomes r={mc} />
          <ScrollRegion label="Etapas del Monte Carlo">
            <table
              aria-label="Etapas del Monte Carlo"
              className="w-full min-w-[640px] text-[12px] tabular-nums [&_td]:py-1 [&_td]:pr-3 [&_th]:py-1 [&_th]:pr-3 [&_th]:align-bottom"
            >
              <thead className="text-left text-[11px] text-muted-foreground">
                <tr>
                  <th className="py-1 font-semibold">Etapa</th>
                  <th className="font-semibold">Llegaron</th>
                  <th className="font-semibold">Pasaron (intervalo)</th>
                  <th className="font-semibold">Pérdida diaria</th>
                  <th className="font-semibold">Caída</th>
                  <th className="font-semibold">Sin tiempo</th>
                  <th className="font-semibold">Días hasta pasar</th>
                </tr>
              </thead>
              <tbody>
                {mc.phases.map((p) => (
                  <tr key={p.name} className="border-t border-border">
                    <td className="py-1 pr-2">{p.name}</td>
                    <td>{pct(p.reached)}</td>
                    <td>{prob(p.passed)}</td>
                    <td className="text-down">{pct(p.failed_daily_loss)}</td>
                    <td className="text-down">{pct(p.failed_max_drawdown)}</td>
                    <td>{pct(p.timeout)}</td>
                    <td>
                      {p.days_to_pass
                        ? `${decimal(p.days_to_pass.p50, 0)} (p95 ${decimal(p.days_to_pass.p95, 0)})`
                        : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </ScrollRegion>
          <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[12.5px] sm:grid-cols-3">
            {(
              [
                [
                  `P(${funded ? 'cobrar' : 'pasar'}) en ≤ ${mc.economics.max_attempts} intentos`,
                  pct(mc.economics.success_within_attempts),
                ],
                ['Intentos esperados', decimal(mc.economics.expected_attempts, 2)],
                ['Cuotas esperadas', money(mc.economics.expected_fees, cur), 'text-down'],
                ['Cobro medio (si cobra)', money(mc.economics.expected_payout, cur)],
                [
                  'Valor esperado del programa',
                  money(mc.economics.expected_value, cur),
                  mc.economics.expected_value?.startsWith('-') ? 'text-down' : undefined,
                ],
                ['Valor esperado operando al azar', money(mc.zero_edge.expected_value, cur)],
              ] as [string, string, string?][]
            ).map(([k, v, cls]) => (
              <div key={k} className="min-w-0">
                <dt className="text-[11px] text-muted-foreground">{k}</dt>
                <dd className={cn('font-semibold tabular-nums', cls)}>{v}</dd>
              </div>
            ))}
          </dl>
        </>
      )}
      {r.observed && (
        <section className="grid gap-1">
          <h4 className="text-[12.5px] font-semibold">Tus operaciones, en orden</h4>
          <ul className="grid gap-0.5 text-[12px] tabular-nums">
            {[...r.observed.phases, ...(r.observed.funded ? [r.observed.funded] : [])].map((s) => (
              <li key={s.name}>
                {s.name}: <strong>{STATUS[s.status]}</strong>
                {s.rule && ` por ${RULE[s.rule]}`}
                {s.status !== 'not_reached' &&
                  ` · ${s.trading_days} días · ganancia ${money(s.profit, cur)}${s.end_day ? ` · ${s.end_day}` : ''}${s.payout ? ` · cobro ${money(s.payout, cur)}` : ''}`}
              </li>
            ))}
          </ul>
        </section>
      )}
      <ul className="grid gap-0.5 text-[11.5px] text-muted-foreground">
        {r.notes.map((n) => (
          <li key={n}>· {n}</li>
        ))}
      </ul>
      {mc && (
        <p className="text-[11px] text-muted-foreground tabular-nums">
          {mc.paths} intentos simulados · semilla {mc.seed} · hasta {mc.horizon_days} días operados
          {mc.block_len_days ? ` · bloques de ${mc.block_len_days} días` : ''} · 1 R ={' '}
          {money(r.risk_per_trade_amount, cur)}
        </p>
      )}
    </div>
  )
}

function Form({ template, reports }: { template: PropRules; reports: string[] }) {
  const [form, setForm] = useState<PropForm>(() => propFormFrom(template))
  const [state, setState] = useState<
    | { status: 'idle' }
    | { status: 'running' }
    | { status: 'ok'; r: PropSimResult }
    | { status: 'error'; message: string; rules: RuleError[] }
  >({ status: 'idle' })
  const ctrl = useRef<AbortController | null>(null)
  const set = (patch: Partial<PropForm>) => setForm((f) => ({ ...f, ...patch }))
  const setPhase = (i: number, patch: Partial<PhaseForm>) =>
    setForm((f) => ({ ...f, phases: f.phases.map((p, k) => (k === i ? { ...p, ...patch } : p)) }))
  const report = reports.includes(form.report) ? form.report : (reports[0] ?? '')
  const source = reports.length > 0 ? form.source : 'model'
  const { req, bad, missing } = toPropSimRequest({ ...form, report, source })
  const errors: Errors = { rules: state.status === 'error' ? state.rules : [], bad }
  const run = () => {
    if (!req) return
    ctrl.current?.abort()
    const c = new AbortController()
    ctrl.current = c
    setState({ status: 'running' })
    runPropSim(req, c.signal).then(
      (r) => setState({ status: 'ok', r }),
      (e: unknown) => {
        if (c.signal.aborted) return
        setState({
          status: 'error',
          message: e instanceof Error ? e.message : String(e),
          rules: e instanceof PropSimRejected ? e.rules : [],
        })
      },
    )
  }
  const f = (name: keyof PropForm, label: string, path?: string, placeholder?: string) => (
    <Field
      label={label}
      name={name}
      path={path}
      errors={errors}
      placeholder={placeholder}
      value={String(form[name])}
      onChange={(v) => set({ [name]: v } as Partial<PropForm>)}
    />
  )
  return (
    <div className="grid gap-3">
      <Group title="Cuenta">
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          {f('account_size', 'Tamaño', '/account_size', 'p. ej. 50000')}
          {f('currency', 'Moneda', '/currency', 'p. ej. USD')}
          {f('timezone', 'Zona del corte del día', '/day/timezone', 'p. ej. UTC')}
          {f('cutoff', 'Hora del corte', '/day/cutoff', 'p. ej. 00:00')}
        </div>
      </Group>
      {form.phases.map((p, i) => (
        <Group key={i} title={`Fase de evaluación ${i + 1}`}>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {(
              [
                ['profit_target_pct', 'Objetivo (%)'],
                ['min_trading_days', 'Días mínimos'],
                ['max_trading_days', 'Días máximos'],
                ['consistency_pct', 'Consistencia (%)'],
              ] as const
            ).map(([k, label]) => (
              <Field
                key={k}
                label={label}
                name={`phases.${i}.${k}`}
                path={`/phases/${i}/${k}`}
                errors={errors}
                placeholder={k === 'profit_target_pct' ? 'obligatorio' : 'sin regla'}
                value={p[k]}
                onChange={(v) => setPhase(i, { [k]: v })}
              />
            ))}
          </div>
          {form.phases.length > 1 && (
            <button
              type="button"
              className="justify-self-start text-[12px] text-muted-foreground underline"
              onClick={() => set({ phases: form.phases.filter((_, k) => k !== i) })}
            >
              Quitar esta fase
            </button>
          )}
        </Group>
      ))}
      {form.phases.length < 3 && (
        <button
          type="button"
          className="min-h-10 justify-self-start rounded-sm border border-border px-3 text-[12.5px] hover:bg-surface"
          onClick={() =>
            set({
              phases: [
                ...form.phases,
                {
                  name: `Fase ${form.phases.length + 1}`,
                  profit_target_pct: '',
                  min_trading_days: '',
                  max_trading_days: '',
                  consistency_pct: '',
                },
              ],
            })
          }
        >
          Agregar una fase de evaluación
        </button>
      )}
      <Group title="Pérdidas">
        <Toggle checked={form.daily_loss_on} onChange={(v) => set({ daily_loss_on: v })}>
          Pérdida diaria máxima
        </Toggle>
        {form.daily_loss_on && (
          <div className="flex flex-wrap items-end gap-3">
            <div className="w-40">
              {f('daily_loss_pct', 'Pérdida diaria (%)', '/daily_loss/limit_pct')}
            </div>
            <Segmented
              label="Sobre qué se calcula la pérdida diaria"
              options={['account_size', 'day_start']}
              value={form.daily_loss_base}
              format={(o) => (o === 'account_size' ? 'del tamaño' : 'del saldo al empezar el día')}
              onChange={(v) => set({ daily_loss_base: v as PropForm['daily_loss_base'] })}
            />
          </div>
        )}
        <Toggle checked={form.drawdown_on} onChange={(v) => set({ drawdown_on: v })}>
          Caída máxima
        </Toggle>
        {form.drawdown_on && (
          <div className="grid gap-2">
            <div className="flex flex-wrap items-end gap-3">
              <div className="w-40">
                {f('drawdown_pct', 'Caída máxima (%)', '/max_drawdown/limit_pct')}
              </div>
              <Segmented
                label="Cómo se mueve el piso de la caída"
                options={['static', 'trailing_eod', 'trailing_intraday']}
                value={form.drawdown_mode}
                format={(o) =>
                  o === 'static'
                    ? 'fijo'
                    : o === 'trailing_eod'
                      ? 'sigue al cierre'
                      : 'sigue intradía'
                }
                onChange={(v) => set({ drawdown_mode: v as PropForm['drawdown_mode'] })}
              />
            </div>
            {form.drawdown_mode === 'trailing_intraday' && (
              <p className="text-[11.5px] text-info">
                No simulable: hace falta el capital vela por vela. Se deja afuera y el resultado es
                un tope superior.
              </p>
            )}
            {form.drawdown_mode !== 'static' && (
              <Toggle checked={form.drawdown_lock} onChange={(v) => set({ drawdown_lock: v })}>
                El piso deja de subir al llegar al saldo inicial
              </Toggle>
            )}
          </div>
        )}
      </Group>
      <Group title="Cuenta fondeada y cuotas">
        <Toggle checked={form.funded_on} onChange={(v) => set({ funded_on: v })}>
          Simular la cuenta fondeada hasta el primer cobro
        </Toggle>
        {form.funded_on && (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {f('profit_split_pct', 'Reparto (%)', '/funded/profit_split_pct', 'obligatorio')}
            {f(
              'payout_min_trading_days',
              'Días para cobrar',
              '/funded/payout_min_trading_days',
              'sin mínimo',
            )}
            {f(
              'payout_min_profit_pct',
              'Ganancia mínima (%)',
              '/funded/payout_min_profit_pct',
              'cualquiera',
            )}
            {f(
              'funded_consistency_pct',
              'Consistencia (%)',
              '/funded/consistency_pct',
              'sin regla',
            )}
          </div>
        )}
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          {f('fee_evaluation', 'Cuota por intento', '/fees/evaluation', '0 si no hay')}
          {f('fee_activation', 'Cuota de activación', '/fees/activation', 'no hay')}
        </div>
        {form.funded_on && (
          <Toggle checked={form.refund} onChange={(v) => set({ refund: v })}>
            Devuelven la cuota con el primer cobro
          </Toggle>
        )}
      </Group>
      <Group title="Tus operaciones">
        {reports.length > 0 && (
          <Segmented
            label="De dónde salen las operaciones"
            options={['model', 'report']}
            value={source}
            format={(o) => (o === 'model' ? 'Modelo' : 'Un informe')}
            onChange={(v) => set({ source: v as PropForm['source'] })}
          />
        )}
        {source === 'model' ? (
          <>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              {f('win_rate', 'Win rate (%)', undefined, 'por ejemplo 45')}
              {f('avg_win_r', 'R medio ganador', undefined, 'por ejemplo 1,5')}
              {f('avg_loss_r', 'R medio perdedor')}
              {f('trades_per_day', 'Operaciones por día', undefined, 'por ejemplo 2')}
            </div>
            <div className="flex flex-wrap items-end gap-3">
              <Toggle checked={form.uncertain} onChange={(v) => set({ uncertain: v })}>
                El win rate salió de pocas operaciones (incertidumbre)
              </Toggle>
              {form.uncertain && (
                <div className="w-40">{f('observed', 'Operaciones observadas')}</div>
              )}
            </div>
          </>
        ) : (
          <label className={labelCls}>
            Informe del laboratorio
            <select
              className={input}
              value={report}
              onChange={(e) => set({ report: e.target.value })}
            >
              {reports.map((id, i) => (
                <option key={id} value={id}>
                  Variante {i + 1} · {id.slice(0, 12)}
                </option>
              ))}
            </select>
          </label>
        )}
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
          {f('risk_pct', 'Riesgo (% del tamaño)', undefined, 'por ejemplo 0,5')}
          {f('paths', 'Intentos simulados', undefined, 'motor')}
          {f('horizon_days', 'Días como mucho', undefined, 'motor')}
          {f('max_attempts', 'Cuotas como mucho', undefined, 'motor')}
          {f('seed', 'Semilla', undefined, 'motor')}
        </div>
      </Group>
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={run}
          disabled={!req || state.status === 'running'}
          className="min-h-10 rounded-sm bg-primary px-4 text-[13px] font-semibold text-primary-foreground disabled:cursor-not-allowed disabled:opacity-50"
        >
          {state.status === 'running' ? 'Simulando…' : 'Simular'}
        </button>
        {missing.length > 0 && (
          <p className="text-[12px] text-muted-foreground">
            Completá tus operaciones y el riesgo. Las reglas que falten te las marca el motor.
          </p>
        )}
      </div>
      <div aria-live="polite" className="grid gap-2">
        {state.status === 'error' && (
          <p role="alert" className="text-[12.5px] text-down">
            {state.message}
            {state.rules.length > 0 && ': revisá los campos marcados.'}
          </p>
        )}
        {state.status === 'ok' && <PropSimResultView r={state.r} />}
      </div>
    </div>
  )
}

export function PropSimulator({ reports = [] }: { reports?: string[] }) {
  const id = useId()
  const templates = usePropTemplates()
  const [picked, setPicked] = useState(0)
  const [focus] = useState(() => takeLabTool() === 'propsim')
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => {
    if (!focus) return
    heading.current?.scrollIntoView?.({ block: 'start' })
    heading.current?.focus()
  }, [focus])
  const list = templates.status === 'ok' ? templates.data : []
  const template = list[picked] ?? list[0]
  return (
    <section
      id="simulador-cuenta-fondeada"
      aria-labelledby={`${id}-title`}
      className="grid min-w-0 gap-3 rounded-sm border border-border bg-card p-3"
    >
      <header className="grid gap-1">
        <p className="text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
          Academia · nivel 12 · simulación educativa
        </p>
        <h3 id={`${id}-title`} ref={heading} tabIndex={-1} className="text-[15px] font-semibold">
          Simulador de cuenta fondeada
        </h3>
        <p className="max-w-[72ch] text-[12.5px] text-pretty text-muted-foreground">
          Completá las reglas de una evaluación y mirá qué pasa con tus operaciones en miles de
          intentos: cuántos pasan, cuántos cobran, cuánto se paga en cuotas y si le ganás a operar
          al azar. El motor simula; esto no dice qué te va a pasar.
        </p>
      </header>
      {templates.status === 'loading' && (
        <p className="text-[12.5px] text-muted-foreground">Pidiendo las plantillas al motor…</p>
      )}
      {templates.status === 'error' && (
        <p role="alert" className="text-[12.5px] text-down">
          No se pudieron leer las plantillas del motor: {templates.message}
        </p>
      )}
      {template && (
        <>
          {list.length > 1 && (
            <label className={labelCls}>
              Plantilla
              <select
                className={input}
                value={picked}
                onChange={(e) => setPicked(Number(e.target.value))}
              >
                {list.map((t, i) => (
                  <option key={t.id} value={i}>
                    {t.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <p className="text-[12px] text-pretty text-muted-foreground">
            {template.source ? (
              <>
                Reglas de {template.source.firm} según su{' '}
                <a
                  className="text-info underline"
                  href={template.source.url}
                  target="_blank"
                  rel="noreferrer"
                >
                  reglamento publicado
                </a>{' '}
                (leído el {template.source.read_at}). Las firmas cambian sus reglas: revisá el
                vigente.
              </>
            ) : (
              <>
                <strong>{template.name}:</strong> no trae las reglas de ninguna firma real. Completá
                los valores del reglamento que quieras estudiar.
              </>
            )}
          </p>
          <Form key={template.id} template={template} reports={reports} />
        </>
      )}
      <div className="grid gap-1 border-t border-border pt-2 text-[11px] text-pretty text-muted-foreground">
        <p>
          Lo que se sabe con fuente: Topstep publica que en 2025 se aprobó el 16,8 % de sus
          evaluaciones (
          <a className="text-info underline" href={TOPSTEP_URL} target="_blank" rel="noreferrer">
            divulgación de riesgos de Topstep
          </a>
          ; cifras de la firma, que mezclan entorno simulado y real). En la UE, entre el 74 % y el
          89 % de las cuentas minoristas de CFD pierden dinero (
          <a className="text-info underline" href={ESMA_URL} target="_blank" rel="noreferrer">
            ESMA, 2018
          </a>
          ).
        </p>
        <p>
          El negocio de estas firmas vive sobre todo de las cuotas de evaluación. Pagar una
          evaluación real es dinero real y queda fuera de esta app. No es consejo de inversión.
        </p>
      </div>
    </section>
  )
}
