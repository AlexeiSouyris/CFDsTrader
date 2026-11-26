/**
 * Laboratorio de estrategias (#778, `docs/design/2026-11-laboratorio.md` §1, §3 y §5). Un
 * constructor por bloques **dibujado desde el catálogo del motor** (`strategy_catalog.v1`): la UI
 * solo arma la `strategy_spec.v1`; el motor la valida, el `lab_worker` la corre aparte con velas
 * guardadas (nunca el fuera de muestra) y el informe llega hecho (`lab_report.v1`).
 *
 * Todo es **exploratorio, no vota**: el modo confirmatorio sigue apagado (llega con el
 * pre-registro). La UI no calcula nada de trading (CLAUDE.md §1) y ningún texto promete ganancias.
 */
import { FlaskConical, Play, Plus, ShieldCheck, Trash2, X } from 'lucide-react'
import { useId, useState, type ReactNode } from 'react'

import { LabReports } from '@/components/ui/lab-report'
import { RiskCalculator } from '@/components/ui/lab-risk'
import { Segmented } from '@/components/ui/segmented'
import type {
  BlockSchema,
  LabJobEvent,
  ParamSchema,
  SpecValidation,
  StrategyCatalog,
  StrategySpec,
} from '@/lib/lab-contracts'
import {
  cancelJob,
  isEnded,
  submitJob,
  useLabCatalog,
  useLabJob,
  validateSpec,
} from '@/lib/lab-api'
import {
  blockDraft,
  blockSummary,
  blocksFor,
  equityText,
  errorsAt,
  initialDraft,
  schemaOf,
  toSpec,
  type BlockDraft,
  type Draft,
} from '@/lib/lab-spec'
import { cn } from '@/lib/utils'

const field =
  'mt-1 block min-h-10 w-full min-w-0 rounded-sm border border-border bg-surface px-2.5 py-1.5 text-[13px] tabular-nums outline-none transition-colors hover:border-foreground/30 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40 disabled:opacity-50 aria-[invalid=true]:border-down/70'
const label =
  'block min-w-0 text-[11px] font-bold tracking-[0.06em] text-muted-foreground uppercase'
const button =
  'inline-flex min-h-10 items-center justify-center gap-1.5 rounded-sm border border-border px-3 text-[12.5px] font-bold transition-colors hover:border-foreground/40 disabled:cursor-not-allowed disabled:opacity-50'

const DIRECTIONS = ['long', 'short', 'both'] as const
const DIRECTION_LABEL: Record<string, string> = { long: 'Largos', short: 'Cortos', both: 'Ambos' }

const STAGE_LABEL: Record<NonNullable<LabJobEvent['stage']>, string> = {
  started: 'Tomado por el worker',
  loading_candles: 'Cargando velas guardadas',
  running_variants: 'Corriendo variantes',
  luck_test: 'Test de suerte',
  building_reports: 'Armando informes',
  saving: 'Guardando',
}
const STATE_LABEL: Record<LabJobEvent['state'], string> = {
  queued: 'En cola',
  running: 'Corriendo',
  done: 'Listo',
  failed: 'Falló',
  cancelled: 'Cancelado',
}

function Errors({ messages, id }: { messages: string[]; id?: string }) {
  if (messages.length === 0) return null
  return (
    <ul id={id} className="mt-1 grid gap-0.5 text-[11.5px] text-down normal-case">
      {messages.map((m) => (
        <li key={m}>{m}</li>
      ))}
    </ul>
  )
}

function Fieldset({
  legend,
  hint,
  children,
}: {
  legend: string
  hint?: string
  children: ReactNode
}) {
  return (
    <fieldset className="grid min-w-0 gap-3 border-t border-border pt-4">
      <legend className="pr-2 text-[12px] font-bold tracking-[0.1em] text-accent uppercase">
        {legend}
      </legend>
      {hint && <p className="-mt-1 text-[12px] text-pretty text-muted-foreground">{hint}</p>}
      {children}
    </fieldset>
  )
}

const rangeText = (p: ParamSchema) =>
  p.kind === 'integer' || p.kind === 'decimal' ? `${p.min} a ${p.max}` : null

/** Un parámetro del catálogo: su control según `kind`, apagable si es opcional. */
function ParamField({
  p,
  value,
  onChange,
  errors,
}: {
  p: ParamSchema
  value: { on: boolean; value: string }
  onChange: (v: { on: boolean; value: string }) => void
  errors: string[]
}) {
  const id = useId()
  const invalid = errors.length > 0 ? true : undefined
  const describedBy = invalid ? `${id}-err` : undefined
  const off = !value.on
  const set = (v: string) => onChange({ ...value, value: v })
  const range = rangeText(p)
  const control =
    p.kind === 'enum' || p.kind === 'boolean' ? (
      <select
        id={id}
        className={field}
        disabled={off}
        aria-invalid={invalid}
        aria-describedby={describedBy}
        value={value.value}
        onChange={(e) => set(e.target.value)}
      >
        {(p.kind === 'enum' ? p.options : ['true', 'false']).map((o) => (
          <option key={o} value={o}>
            {o === 'true' ? 'sí' : o === 'false' ? 'no' : o}
          </option>
        ))}
      </select>
    ) : (
      <input
        id={id}
        className={field}
        disabled={off}
        aria-invalid={invalid}
        aria-describedby={describedBy}
        type={p.kind === 'time' ? 'time' : 'text'}
        inputMode={p.kind === 'integer' ? 'numeric' : p.kind === 'decimal' ? 'decimal' : undefined}
        value={value.value}
        onChange={(e) => set(e.target.value)}
      />
    )
  return (
    <div className="min-w-0">
      <div className="flex items-center justify-between gap-2">
        <label htmlFor={id} className={label}>
          {p.label}
        </label>
        {!p.required && (
          <label className="flex items-center gap-1 text-[11px] text-muted-foreground">
            <input
              type="checkbox"
              checked={value.on}
              onChange={(e) => onChange({ ...value, on: e.target.checked })}
              aria-label={`Usar ${p.label}`}
            />
            usar
          </label>
        )}
      </div>
      {control}
      {range && <p className="mt-0.5 text-[11px] text-muted-foreground tabular-nums">{range}</p>}
      <Errors id={describedBy} messages={errors} />
    </div>
  )
}

/** Un bloque: qué tipo (de los que sirven para el lugar y el timeframe) y sus parámetros. */
function BlockEditor({
  title,
  options,
  cat,
  draft,
  path,
  validation,
  onChange,
  onRemove,
}: {
  title: string
  options: BlockSchema[]
  cat: StrategyCatalog
  draft: BlockDraft
  path: string
  validation: SpecValidation | null
  onChange: (d: BlockDraft) => void
  onRemove?: () => void
}) {
  const id = useId()
  const schema = schemaOf(cat, draft.type)
  const own = errorsAt(validation, path).filter(
    (_, i) => !validation?.errors[i]?.path.startsWith(`${path}/params/`),
  )
  return (
    <div className="grid min-w-0 gap-2 rounded-sm border border-border bg-card p-3">
      <div className="flex items-end gap-2">
        <div className="min-w-0 flex-1">
          <label htmlFor={id} className={label}>
            {title}
          </label>
          <select
            id={id}
            className={field}
            value={draft.type}
            onChange={(e) => {
              const next = schemaOf(cat, e.target.value)
              if (next) onChange(blockDraft(next))
            }}
          >
            {options.map((b) => (
              <option key={b.type} value={b.type}>
                {b.label}
              </option>
            ))}
          </select>
        </div>
        {onRemove && (
          <button
            type="button"
            className={button}
            onClick={onRemove}
            aria-label={`Quitar ${title}`}
          >
            <Trash2 className="size-4" aria-hidden />
          </button>
        )}
      </div>
      {schema && (
        <p className="text-[12px] text-pretty text-muted-foreground">{schema.description}</p>
      )}
      <div className="grid gap-3 sm:grid-cols-2">
        {schema?.params.map((p) => (
          <ParamField
            key={p.name}
            p={p}
            value={draft.params[p.name] ?? { on: false, value: '' }}
            errors={errorsAt(validation, `${path}/params/${p.name}`)}
            onChange={(v) => onChange({ ...draft, params: { ...draft.params, [p.name]: v } })}
          />
        ))}
      </div>
      <Errors
        messages={own.length > 0 ? errorsAt(validation, path).filter((m) => own.includes(m)) : []}
      />
    </div>
  )
}

function Progress({ event, onCancel }: { event: LabJobEvent; onCancel: () => void }) {
  const ended = isEnded(event)
  return (
    <section
      aria-label="Progreso de la corrida"
      className="grid gap-2 rounded-sm border border-border bg-card p-3"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-[13px]">
          <strong
            data-job-state={event.state}
            className={cn(
              event.state === 'failed' && 'text-down',
              event.state === 'done' && 'text-info',
            )}
          >
            {STATE_LABEL[event.state]}
          </strong>
          {event.stage && (
            <span className="text-muted-foreground"> · {STAGE_LABEL[event.stage]}</span>
          )}
          <span className="text-muted-foreground tabular-nums">
            {' '}
            · variante {Math.min(event.variants_done + 1, event.variants_total)} de{' '}
            {event.variants_total}
          </span>
        </p>
        {!ended && (
          <button type="button" className={button} onClick={onCancel}>
            <X className="size-4" aria-hidden />
            Cancelar
          </button>
        )}
      </div>
      <div
        role="progressbar"
        aria-label="Avance de la corrida"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={event.percent}
        className="h-1.5 overflow-hidden rounded-full bg-border"
      >
        <div
          className="h-full bg-accent transition-[width]"
          style={{ width: `${event.percent}%` }}
        />
      </div>
      {event.error && <p className="text-[12.5px] text-down">{event.error}</p>}
      <p className="text-[11px] text-muted-foreground">
        Trabajo <code>{event.job_id.slice(0, 12)}</code>. Corre aparte, con velas guardadas y nunca
        con el fuera de muestra.
      </p>
    </section>
  )
}

function message(e: unknown) {
  return e instanceof Error ? e.message : String(e)
}

function Builder({ cat }: { cat: StrategyCatalog }) {
  const [draft, setDraft] = useState<Draft>(() => initialDraft(cat))
  const [variants, setVariants] = useState<StrategySpec[]>([])
  const [validation, setValidation] = useState<SpecValidation | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [first, setFirst] = useState<LabJobEvent | null>(null)
  const { event, error: jobError } = useLabJob(first?.job_id ?? null, first)
  const ids = useId()
  const limits = cat.limits
  const market = cat.markets.find((m) => m.instrument_id === draft.instrument_id)
  const change = (patch: Partial<Draft>) => {
    setDraft((d) => ({ ...d, ...patch }))
    setValidation(null)
  }
  const options = (slot: BlockSchema['slot']) => blocksFor(cat, slot, draft.timeframe)
  const setTimeframe = (tf: string) => {
    const next = { ...draft, timeframe: tf as Draft['timeframe'] }
    const keep = (d: BlockDraft, slot: BlockSchema['slot']) =>
      blocksFor(cat, slot, next.timeframe).some((b) => b.type === d.type)
    const fallback = (slot: BlockSchema['slot']) => {
      const b = blocksFor(cat, slot, next.timeframe)[0]
      return b ? blockDraft(b) : { type: '', params: {} }
    }
    change({
      timeframe: next.timeframe,
      entry: draft.entry.map((d) => (keep(d, 'entry') ? d : fallback('entry'))),
      filters: draft.filters.filter((d) => keep(d, 'filter')),
      stop: keep(draft.stop, 'stop') ? draft.stop : fallback('stop'),
      exit: keep(draft.exit, 'exit') ? draft.exit : fallback('exit'),
    })
  }
  const built = toSpec(cat, draft)
  const errs = (path: string) => errorsAt(validation, path)

  async function validate(): Promise<SpecValidation | null> {
    if (!built.spec) {
      setNotice(`Completá: ${built.missing.join(', ')}.`)
      return null
    }
    setNotice(null)
    try {
      const v = await validateSpec(built.spec)
      setValidation(v)
      return v
    } catch (e) {
      setNotice(message(e))
      return null
    }
  }

  async function run() {
    const equity = equityText(draft.initial_equity)
    if (!equity) {
      setNotice('Escribí el capital hipotético inicial (un número positivo).')
      return
    }
    setBusy(true)
    try {
      let specs = variants
      if (specs.length === 0) {
        const v = await validate()
        if (!v?.valid || !built.spec) return
        specs = [built.spec]
      }
      setFirst(await submitJob({ specs, initial_equity: equity }))
      setNotice(null)
    } catch (e) {
      setNotice(message(e))
    } finally {
      setBusy(false)
    }
  }

  async function addVariant() {
    const v = await validate()
    if (!v?.valid || !built.spec) return
    const spec = built.spec
    setVariants((vs) => [...vs, spec])
  }

  async function cancel() {
    if (!event) return
    try {
      setFirst(await cancelJob(event.job_id))
    } catch (e) {
      setNotice(message(e))
    }
  }

  const running = event !== null && !isEnded(event)
  return (
    <div className="grid min-w-0 gap-6 xl:grid-cols-[minmax(0,440px)_minmax(0,1fr)]">
      <form
        aria-label="Constructor de estrategias"
        className="grid min-w-0 content-start gap-5"
        onSubmit={(e) => {
          e.preventDefault()
          void run()
        }}
      >
        <Fieldset legend="Mercado">
          <label className={label}>
            Instrumento
            <select
              className={field}
              value={draft.instrument_id}
              aria-invalid={errs('/market/instrument_id').length > 0 || undefined}
              onChange={(e) => change({ instrument_id: e.target.value })}
            >
              {cat.markets.map((m) => (
                <option key={m.instrument_id} value={m.instrument_id}>
                  {m.instrument_id} · velas de futuros
                </option>
              ))}
            </select>
          </label>
          <div className="flex flex-wrap items-center gap-3">
            <Segmented
              label="Timeframe de la señal"
              options={market?.timeframes ?? ['1h']}
              value={draft.timeframe}
              onChange={setTimeframe}
            />
            <Segmented
              label="Dirección"
              options={DIRECTIONS}
              value={draft.direction}
              format={(o) => DIRECTION_LABEL[o] ?? o}
              onChange={(o) => change({ direction: o as Draft['direction'] })}
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            {(['from', 'to'] as const).map((k) => (
              <label key={k} className={label}>
                {k === 'from' ? 'Desde (UTC)' : 'Hasta, sin incluir (UTC)'}
                <input
                  type="date"
                  className={field}
                  value={draft[k]}
                  aria-invalid={
                    errs(`/period/${k}`).length > 0 || errs('/period').length > 0 || undefined
                  }
                  onChange={(e) => change({ [k]: e.target.value })}
                />
              </label>
            ))}
          </div>
          <Errors messages={[...errs('/period'), ...errs('/market')]} />
        </Fieldset>

        <Fieldset
          legend="Entrada"
          hint={`Condiciones unidas con "y" (hasta ${limits.max_entry_conditions}). Se decide al cierre de la vela y se entra en la apertura siguiente.`}
        >
          {draft.entry.map((b, i) => (
            <BlockEditor
              key={i}
              title={`Condición ${i + 1}`}
              options={options('entry')}
              cat={cat}
              draft={b}
              path={`/entry/${i}`}
              validation={validation}
              onChange={(d) => change({ entry: draft.entry.map((x, j) => (j === i ? d : x)) })}
              onRemove={
                draft.entry.length > 1
                  ? () => change({ entry: draft.entry.filter((_, j) => j !== i) })
                  : undefined
              }
            />
          ))}
          <button
            type="button"
            className={button}
            disabled={
              draft.entry.length >= limits.max_entry_conditions || options('entry').length === 0
            }
            onClick={() => change({ entry: [...draft.entry, blockDraft(options('entry')[0])] })}
          >
            <Plus className="size-4" aria-hidden />
            Agregar condición
          </button>
          <Errors
            messages={errs('/entry').filter(
              (m) => !draft.entry.some((_, i) => errs(`/entry/${i}`).includes(m)),
            )}
          />
        </Fieldset>

        <Fieldset
          legend="Filtros"
          hint={`Solo quitan entradas, nunca agregan (hasta ${limits.max_filters}).`}
        >
          {draft.filters.map((b, i) => (
            <BlockEditor
              key={i}
              title={`Filtro ${i + 1}`}
              options={options('filter')}
              cat={cat}
              draft={b}
              path={`/filters/${i}`}
              validation={validation}
              onChange={(d) => change({ filters: draft.filters.map((x, j) => (j === i ? d : x)) })}
              onRemove={() => change({ filters: draft.filters.filter((_, j) => j !== i) })}
            />
          ))}
          <button
            type="button"
            className={button}
            disabled={draft.filters.length >= limits.max_filters || options('filter').length === 0}
            onClick={() =>
              change({ filters: [...draft.filters, blockDraft(options('filter')[0])] })
            }
          >
            <Plus className="size-4" aria-hidden />
            Agregar filtro
          </button>
        </Fieldset>

        <Fieldset legend="Stop y salida" hint="El stop es obligatorio: sin stop no hay estrategia.">
          <BlockEditor
            title="Stop"
            options={options('stop')}
            cat={cat}
            draft={draft.stop}
            path="/stop"
            validation={validation}
            onChange={(d) => change({ stop: d })}
          />
          <BlockEditor
            title="Salida"
            options={options('exit')}
            cat={cat}
            draft={draft.exit}
            path="/exit"
            validation={validation}
            onChange={(d) => change({ exit: d })}
          />
        </Fieldset>

        <Fieldset
          legend="Tamaño y costos"
          hint="El tamaño lo calcula siempre el módulo de riesgo del motor. Los costos del catálogo solo se pueden subir."
        >
          <div className="grid grid-cols-2 gap-3">
            <label className={label}>
              Riesgo por operación (%)
              <input
                className={field}
                inputMode="decimal"
                value={draft.risk_pct}
                aria-invalid={errs('/risk_pct').length > 0 || undefined}
                onChange={(e) => change({ risk_pct: e.target.value })}
              />
              <span className="mt-0.5 block font-normal tracking-normal normal-case tabular-nums">
                como mucho {limits.max_risk_pct} (config/risk.toml)
              </span>
            </label>
            <label className={label}>
              Costos ×
              <input
                className={field}
                inputMode="decimal"
                value={draft.cost_multiplier}
                aria-invalid={errs('/cost_multiplier').length > 0 || undefined}
                onChange={(e) => change({ cost_multiplier: e.target.value })}
              />
              <span className="mt-0.5 block font-normal tracking-normal normal-case tabular-nums">
                {limits.min_cost_multiplier} a {limits.max_cost_multiplier}
              </span>
            </label>
          </div>
          <Errors messages={[...errs('/risk_pct'), ...errs('/cost_multiplier')]} />
          <label className={label}>
            Capital hipotético inicial (EUR, simulado)
            <input
              className={field}
              inputMode="decimal"
              placeholder="por ejemplo 100000"
              value={draft.initial_equity}
              onChange={(e) => setDraft((d) => ({ ...d, initial_equity: e.target.value }))}
            />
          </label>
        </Fieldset>

        <Fieldset legend="Modo">
          <p className="text-[12.5px]">
            <strong>Exploratorio, no vota.</strong>{' '}
            <span className="text-muted-foreground">
              Cada corrida queda contada en su familia y entra al N del Sharpe deflactado. El modo
              confirmatorio (con pre-registro congelado) todavía no está disponible.
            </span>
          </p>
        </Fieldset>

        {variants.length > 0 && (
          <Fieldset
            legend={`Variantes (${variants.length} de ${limits.max_variants_per_run})`}
            hint="Se corren juntas: tienen que ser de la misma familia y el mismo período. Desde 4 variantes el informe trae el PBO."
          >
            <ol className="grid gap-1.5 text-[12px]">
              {variants.map((s, i) => (
                <li
                  key={i}
                  className="flex items-start justify-between gap-2 rounded-sm border border-border bg-card px-2.5 py-1.5"
                >
                  <span className="min-w-0 break-words">
                    <strong>{i + 1}.</strong> {s.entry.map(blockSummary).join(' y ')} ·{' '}
                    {blockSummary(s.stop)} · {blockSummary(s.exit)}
                  </span>
                  <button
                    type="button"
                    className="shrink-0 text-muted-foreground hover:text-foreground"
                    aria-label={`Quitar la variante ${i + 1}`}
                    onClick={() => setVariants((vs) => vs.filter((_, j) => j !== i))}
                  >
                    <Trash2 className="size-4" aria-hidden />
                  </button>
                </li>
              ))}
            </ol>
          </Fieldset>
        )}

        <div className="flex flex-wrap gap-2 border-t border-border pt-4">
          <button type="button" className={button} disabled={busy} onClick={() => void validate()}>
            <ShieldCheck className="size-4" aria-hidden />
            Validar
          </button>
          <button
            type="button"
            className={button}
            disabled={busy || variants.length >= limits.max_variants_per_run}
            onClick={() => void addVariant()}
          >
            <Plus className="size-4" aria-hidden />
            Agregar como variante
          </button>
          <button
            type="submit"
            className={cn(
              button,
              'border-primary bg-primary text-primary-foreground hover:border-primary',
            )}
            disabled={busy || running}
          >
            <Play className="size-4" aria-hidden />
            {variants.length > 0 ? `Correr ${variants.length} variantes` : 'Correr'}
          </button>
        </div>
        <div aria-live="polite" id={`${ids}-status`} className="grid gap-1 text-[12.5px]">
          {notice && <p className="text-down">{notice}</p>}
          {validation?.valid && (
            <p className="text-info">
              Válida para el motor · spec <code>{validation.spec_sha256?.slice(0, 12)}</code> ·
              familia <code>{validation.family_sha256?.slice(0, 12)}</code>
            </p>
          )}
          {validation && !validation.valid && (
            <p className="text-down">
              El motor encontró {validation.errors.length}{' '}
              {validation.errors.length === 1 ? 'problema' : 'problemas'}: están marcados en cada
              campo.
            </p>
          )}
          {validation && !validation.valid && (
            <Errors
              messages={validation.errors.filter((e) => e.path === '').map((e) => e.message)}
            />
          )}
        </div>
      </form>

      <div className="grid min-w-0 content-start gap-4">
        {event ? (
          <Progress event={event} onCancel={() => void cancel()} />
        ) : (
          <p className="rounded-sm border border-dashed border-border p-4 text-[13px] text-pretty text-muted-foreground">
            Sin corrida todavía. Armá la estrategia, validala y corré: el motor la corre aparte con
            velas guardadas, nunca con el fuera de muestra, y arma un informe honesto con lo que la
            curva esconde.
          </p>
        )}
        {jobError && <p className="text-[12.5px] text-down">{jobError}</p>}
        {event?.state === 'done' && <LabReports ids={event.report_ids} />}
        <RiskCalculator cat={cat} />
      </div>
    </div>
  )
}

export function LabView() {
  const catalog = useLabCatalog()
  return (
    <div
      data-section="research"
      className="grid min-w-0 gap-6 rounded-section bg-section p-4 text-foreground sm:p-6"
    >
      <header className="grid gap-2">
        <p className="flex items-center gap-2 text-[11px] font-bold tracking-[0.14em] text-accent uppercase">
          <FlaskConical className="size-4" aria-hidden />
          Exploratorio · no vota
        </p>
        <h2>Laboratorio de estrategias</h2>
        <p className="max-w-[72ch] text-[13.5px] text-pretty text-muted-foreground">
          Armá una estrategia con bloques y corrila sobre el pasado. El informe muestra lo que la
          curva esconde: cuántas variantes probaste, la mala suerte posible, si le gana a entrar al
          azar y si aguanta costos más altos. Son resultados del pasado en simulación, no promesas.
        </p>
      </header>
      {catalog.status === 'loading' && (
        <p className="text-[13px] text-muted-foreground">Cargando el catálogo del motor…</p>
      )}
      {catalog.status === 'error' && (
        <p role="alert" className="text-[13px] text-down">
          No se pudo leer el catálogo del laboratorio: {catalog.message}
        </p>
      )}
      {catalog.status === 'ok' && <Builder cat={catalog.data} />}
    </div>
  )
}
