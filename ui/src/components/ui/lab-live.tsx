/**
 * Vista en vivo de una corrida del laboratorio (#854): cuántas variantes van y la curva de capital
 * de cada una apenas termina, todas encimadas, con el color de cómo terminó (verde arriba de 1,
 * rojo abajo, gris sin cambio). Las curvas llegan hechas del motor (`curve` de
 * `lab_job_event.v1`); la UI solo las ubica en pantalla.
 *
 * Sin parpadeo: cada curva es un `<path>` memoizado en coordenadas de datos; al cambiar la escala
 * solo cambia la transformación del grupo (`non-scaling-stroke` mantiene el trazo fino), así que
 * una curva nueva agrega un nodo y no vuelve a dibujar las otras.
 *
 * Honestidad (CLAUDE.md §5 y §5b): exploratorio, no vota; ninguna curva es "la ganadora"; el número
 * de variantes va grande porque más pruebas = más chances de que alguna salga bien por suerte.
 */
import { memo } from 'react'

import { ToolLock } from '@/components/ui/tool-lock'
import type { LabJobEvent, LabReport, TrialCurve } from '@/lib/lab-contracts'
import { pctOrDash } from '@/lib/run-format'

const W = 1000
const H = 240
const PAD = 12

type Result = 'up' | 'down' | 'flat'

const STROKE: Record<Result, string> = {
  up: 'var(--chart-up)',
  down: 'var(--chart-down)',
  flat: 'var(--muted-foreground)',
}

/** Cómo terminó la curva frente al capital inicial (1). */
function resultOf(c: TrialCurve): Result {
  const last = Number(c.equity[c.equity.length - 1])
  return last > 1 ? 'up' : last < 1 ? 'down' : 'flat'
}

/** La curva en coordenadas de datos: x de 0 a W, y = −capital (el grupo la escala). */
const Curve = memo(function Curve({ curve }: { curve: TrialCurve }) {
  const n = curve.equity.length
  const d = curve.equity
    .map(
      (v, i) => `${i === 0 ? 'M' : 'L'}${n > 1 ? ((i * W) / (n - 1)).toFixed(1) : 0} ${-Number(v)}`,
    )
    .join('')
  const result = resultOf(curve)
  return (
    <path
      d={n > 1 ? d : `${d}L${W} ${-Number(curve.equity[0])}`}
      data-variant={curve.variant}
      data-result={result}
      fill="none"
      stroke={STROKE[result]}
      strokeOpacity={0.7}
      strokeWidth={1.25}
      vectorEffect="non-scaling-stroke"
    />
  )
})

function Counter({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div className="grid min-w-0 gap-0.5">
      <dt className="text-[10.5px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
        {label}
      </dt>
      <dd className={`font-mono text-[15px] tabular-nums ${tone ?? ''}`}>{value}</dd>
    </div>
  )
}

/** Contador y curvas encimadas de la corrida. */
export function LiveTrials({ event, curves }: { event: LabJobEvent; curves: TrialCurve[] }) {
  let [min, max] = [1, 1]
  for (const c of curves)
    for (const v of c.equity) {
      const x = Number(v)
      if (Number.isFinite(x)) [min, max] = [Math.min(min, x), Math.max(max, x)]
    }
  const span = max - min || 1
  const k = (H - 2 * PAD) / span
  const count = (r: Result) => curves.filter((c) => resultOf(c) === r).length
  const done = event.state === 'done' ? event.variants_total : event.variants_done
  return (
    <section
      aria-label="Variantes en vivo"
      className="grid min-w-0 gap-3 rounded-sm border border-border bg-card p-3"
    >
      <p data-live-label className="text-[11px] font-bold tracking-[0.1em] text-accent uppercase">
        Exploratorio, no vota · resultados del pasado en simulación, no promesas
      </p>
      <div className="flex flex-wrap items-end gap-x-6 gap-y-2">
        <p className="grid">
          <span className="text-[10.5px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
            Variantes probadas
          </span>
          <strong data-live-count className="font-mono text-[28px] leading-none tabular-nums">
            {done}{' '}
            <span className="text-[15px] text-muted-foreground">de {event.variants_total}</span>
          </strong>
        </p>
        <dl className="flex flex-wrap gap-x-5 gap-y-1">
          <Counter label="Terminaron arriba de 1" value={String(count('up'))} tone="text-up" />
          <Counter label="Terminaron abajo" value={String(count('down'))} tone="text-down" />
          <Counter label="Sin cambio" value={String(count('flat'))} />
        </dl>
      </div>
      <svg
        role="img"
        aria-label={`Curvas de capital de ${curves.length} variantes, compuestas desde 1`}
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className="h-48 w-full rounded-sm bg-surface sm:h-60"
      >
        <g transform={`translate(0 ${PAD + max * k}) scale(1 ${k})`}>
          <line
            x1={0}
            x2={W}
            y1={-1}
            y2={-1}
            stroke="var(--chart-grid)"
            strokeDasharray="4 4"
            vectorEffect="non-scaling-stroke"
          />
          {curves.map((c) => (
            <Curve key={c.variant} curve={c} />
          ))}
        </g>
      </svg>
      <p className="text-[11.5px] text-pretty text-muted-foreground">
        Cada línea es una variante (capital compuesto desde 1, mes a mes; la punteada es 1). Más
        variantes probadas = más chances de que alguna salga bien por pura suerte: ninguna curva es
        una ganadora. Todo es dentro de muestra: el laboratorio nunca carga el fuera de muestra.
      </p>
    </section>
  )
}

/**
 * "Suerte o ventaja": el PBO (probabilidad de sobreajuste) que ya trae el informe del motor para
 * las variantes corridas juntas. No calcula nada nuevo.
 */
export function LuckBar({ reports }: { reports: LabReport[] }) {
  const pbo = reports.find((r) => r.pbo)?.pbo ?? null
  const share = pbo ? Math.min(100, Math.max(0, Number(pbo.pbo) * 100)) : null
  return (
    <ToolLock tool="tool.pbo" compact>
      <section aria-label="Suerte o ventaja" className="grid gap-1.5">
        <p className="flex flex-wrap items-baseline justify-between gap-2 text-[12.5px]">
          <strong>Suerte o ventaja</strong>
          <span className="text-muted-foreground tabular-nums">
            Probabilidad de sobreajuste (PBO): {pbo ? pctOrDash(pbo.pbo) : '—'}
          </span>
        </p>
        {share !== null && (
          <div
            role="meter"
            aria-label="Probabilidad de sobreajuste"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(share)}
            className="h-1.5 overflow-hidden rounded-full bg-border"
          >
            <div className="h-full bg-down" style={{ width: `${share}%` }} />
          </div>
        )}
        <p className="text-[11.5px] text-pretty text-muted-foreground">
          {pbo
            ? `Sobre ${pbo.variants} variantes corridas juntas: cuanto más alto, más probable que la mejor sea suerte. El Sharpe deflactado de cada variante está en su informe.`
            : 'El PBO aparece desde 4 variantes de la familia corridas juntas.'}
        </p>
      </section>
    </ToolLock>
  )
}
