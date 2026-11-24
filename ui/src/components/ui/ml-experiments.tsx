/**
 * «Modelo (informativo)» en Performance (#426): el último experimento de ML v1 de cada
 * instrumento, tal como lo guardó Python (`ml_experiment.v1`). Muestra el veredicto contra B1, las
 * métricas de B0, B1 y M1 y la fecha. Solo presenta: no calcula métricas ni decide nada, y el
 * modelo no decide operaciones.
 */
import { dateTimeUtc } from '@/lib/format'
import { ML_MODELS, type MlExperimentEntry, type MlModel, type MlStatus } from '@/lib/ml-api'
import { cn } from '@/lib/utils'

const th =
  'py-2 pr-3 text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase'
const td = 'py-2 pr-3 tabular-nums whitespace-nowrap'

const MODEL_LABEL: Record<MlModel, string> = {
  B0: 'frecuencia global',
  B1: 'tabla de §13 (lo que se muestra hoy)',
  M1: 'regresión logística',
}

const VERDICT: Record<MlStatus, { label: string; detail: string; tone: string }> = {
  m1_improves: {
    label: 'M1 mejora a B1',
    detail:
      'El log loss de M1 es menor y el intervalo de la diferencia queda entero por debajo de cero.',
    tone: 'border-primary/50 text-foreground',
  },
  b1_stays: {
    label: 'M1 no mejora: queda B1',
    detail: 'La diferencia con B1 no alcanza el criterio fijado antes de mirar.',
    tone: 'border-border text-foreground',
  },
  suspended: {
    label: 'En suspenso: se sigue mostrando B1',
    detail: 'No hubo datos suficientes para evaluar M1 contra B1.',
    tone: 'border-border text-muted-foreground',
  },
}

const SOURCE_LABEL: Record<MlExperimentEntry['label_source'], string> = {
  ml_label_runs: 'etiquetas sin P/L',
  backtest_runs: 'corrida de backtest',
}

/** Métricas del modelo (números, no dinero), con coma decimal y signo menos tipográfico. */
const num = (x: number | null | undefined, digits: number) =>
  x === null || x === undefined
    ? '—'
    : new Intl.NumberFormat('es-ES', {
        minimumFractionDigits: digits,
        maximumFractionDigits: digits,
        useGrouping: false,
      })
        .format(x)
        .replace('-', '−')

const signedNum = (x: number, digits: number) => (x > 0 ? '+' : '') + num(x, digits)

/** Cuántos motivos distintos de las ventanas sin entrenar se listan (los más frecuentes). */
const SHOWN_REASONS = 3

/** Las ventanas que no entrenaron, agrupadas por su motivo tal como lo escribió Python. */
function skippedReasons(e: MlExperimentEntry['experiment']) {
  const by = new Map<string, number>()
  for (const w of e.windows) if (w.skipped) by.set(w.skipped, (by.get(w.skipped) ?? 0) + 1)
  return [...by].sort((a, b) => b[1] - a[1])
}

function Experiment({ entry }: { entry: MlExperimentEntry }) {
  const e = entry.experiment
  const r = e.result
  const v = VERDICT[r.status]
  const trained = e.windows.filter((w) => w.skipped === null).length
  const reasons = skippedReasons(e)
  const conf = Math.round(e.hyperparameters.confidence * 100)
  const models = ML_MODELS.filter((m) => e.pooled[m])
  return (
    <article
      aria-label={`Modelo de ${entry.instrument_id}`}
      data-experiment={e.experiment_id}
      className="grid gap-3 rounded-xl border border-border p-3"
    >
      <header className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <h3 className="text-[13px] font-bold">{entry.instrument_id}</h3>
        <span className="text-[11px] text-muted-foreground tabular-nums">
          {dateTimeUtc(e.created_at)} · {SOURCE_LABEL[entry.label_source]} ·{' '}
          <span className="[overflow-wrap:anywhere]">{e.backtest_run_id}</span>
        </span>
      </header>

      <div className="grid gap-1">
        <p
          data-verdict={r.status}
          className={cn('w-fit rounded-full border px-2.5 py-1 text-[12px] font-bold', v.tone)}
        >
          {v.label}
        </p>
        <p className="text-[12px] text-muted-foreground">{v.detail}</p>
        {r.difference !== null && (
          <p className="text-[12px] tabular-nums">
            Log loss de M1 menos el de B1: <strong>{signedNum(r.difference, 4)}</strong>
            {r.low !== null && r.high !== null && (
              <>
                {' '}
                (intervalo del {conf} %: {num(r.low, 4)} a {num(r.high, 4)}
                {r.blocks !== null && `, bootstrap por ${r.blocks} semanas`})
              </>
            )}
          </p>
        )}
        <p className="text-[12px] text-muted-foreground">
          {trained} de {e.windows.length}{' '}
          {e.windows.length === 1 ? 'ventana entrenó' : 'ventanas entrenaron'}
          {reasons.length > 0 && ' · sin entrenar: '}
          {reasons.slice(0, SHOWN_REASONS).map(([why, n], i) => (
            <span key={why}>
              {i > 0 && '; '}
              {why} ({n})
            </span>
          ))}
          {reasons.length > SHOWN_REASONS &&
            ` y ${reasons.length - SHOWN_REASONS} ${reasons.length - SHOWN_REASONS === 1 ? 'motivo más' : 'motivos más'}`}
        </p>
      </div>

      {models.length === 0 ? (
        <p className="rounded-xl border border-dashed border-border px-3 py-2 text-[12px] text-muted-foreground">
          Sin métricas: ninguna validación entró en la medición conjunta.
        </p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-[12px]" aria-label="Métricas por modelo">
            <thead>
              <tr className="border-b border-border">
                <th className={th}>Modelo</th>
                <th className={th}>Log loss</th>
                <th className={th}>Brier</th>
                <th className={th}>AUC</th>
              </tr>
            </thead>
            <tbody>
              {models.map((m) => {
                const s = e.pooled[m]!
                return (
                  <tr key={m} data-model={m} className="border-b border-border/60 last:border-0">
                    <td className="py-2 pr-3">
                      <strong>{m}</strong>
                      <span className="block text-[11px] text-muted-foreground">
                        {MODEL_LABEL[m]}
                      </span>
                    </td>
                    <td className={td}>{num(s.log_loss, 4)}</td>
                    <td className={td}>{num(s.brier, 4)}</td>
                    <td className={td}>{num(s.auc, 3)}</td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
      <p className="text-[11px] text-muted-foreground">
        Más bajo es mejor en log loss y Brier; en AUC, más alto. La calibración por grupos todavía
        no se guarda con el experimento.
      </p>
    </article>
  )
}

export function MlExperiments({ entries }: { entries: MlExperimentEntry[] }) {
  return (
    <div className="grid gap-3">
      {entries.map((e) => (
        <Experiment key={`${e.instrument_id}:${e.experiment.experiment_id}`} entry={e} />
      ))}
    </div>
  )
}
