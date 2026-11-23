/**
 * Contexto de mercado del Panel (#294): el índice de sentimiento (medidor de −1 a +1 y su
 * historia) y la distribución de los análogos históricos. **No calcula nada:** muestra los
 * números que mandó el motor; `Number()` se usa solo para ubicar cada cosa en pantalla. Los
 * análogos son resultados pasados, nunca un pronóstico.
 */
import { AlertTriangle } from 'lucide-react'

import type { AnalogGroup, AnalogReport, SentimentIndex } from '@/lib/context-contracts'
import { dateTimeUtc, dateUtc, decimal, signed, signedPercent } from '@/lib/format'
import { cn } from '@/lib/utils'

const clamp = (v: number) => Math.max(-1, Math.min(1, v))

/** Medidor de −1 (miedo) a +1 (euforia), con el valor tal como llegó. */
export function SentimentGauge({ index }: { index: SentimentIndex }) {
  const pos = ((clamp(Number(index.value)) + 1) / 2) * 100
  const used = index.components.filter((c) => c.status === 'valid').length
  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <strong className="text-[clamp(26px,3vw,36px)] leading-none font-extrabold tracking-[-0.04em] tabular-nums">
          {signed(index.value)}
        </strong>
        <span className="text-[11px] text-muted-foreground">
          al {dateTimeUtc(index.published_at)}
        </span>
      </div>
      <div
        role="meter"
        aria-label={`Índice de sentimiento de ${index.instrument_id}`}
        aria-valuemin={-1}
        aria-valuemax={1}
        aria-valuenow={Number(index.value)}
        aria-valuetext={signed(index.value)}
        className="relative h-2 rounded-full bg-gradient-to-r from-down/50 via-border to-up/50"
      >
        <span
          className="absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-background bg-foreground"
          style={{ left: `${pos}%` }}
        />
      </div>
      <div className="flex justify-between text-[10.5px] text-muted-foreground">
        <span>−1 miedo</span>
        <span>0</span>
        <span>+1 euforia</span>
      </div>
      <p className="text-[11.5px] text-muted-foreground">
        {used} de {index.components.length} componentes válidos (mínimo {index.min_components})
        {index.warmup_complete ? '' : ' · todavía en calentamiento'}.
      </p>
    </div>
  )
}

/** La historia del índice: una línea en la escala fija de −1 a +1. */
export function SentimentHistory({
  history,
  width = 320,
  height = 72,
}: {
  history: SentimentIndex[]
  width?: number
  height?: number
}) {
  if (history.length < 2)
    return <p className="text-[12px] text-muted-foreground">Todavía no hay historia.</p>
  const x = (i: number) => (i / (history.length - 1)) * width
  const y = (v: string) => ((1 - clamp(Number(v))) / 2) * height
  const d = history.map((h, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(h.value).toFixed(1)}`)
  const first = history[0]
  const last = history[history.length - 1]
  return (
    <figure className="grid gap-1">
      <svg
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={`Historia del índice: ${history.length} valores, del ${dateUtc(first.published_at)} al ${dateUtc(last.published_at)}`}
        className="h-[72px] w-full"
        preserveAspectRatio="none"
      >
        <line x1={0} x2={width} y1={height / 2} y2={height / 2} stroke="var(--chart-grid)" />
        <path d={d.join('')} fill="none" stroke="var(--chart-accent)" strokeWidth={1.5} />
      </svg>
      <figcaption className="flex justify-between text-[10.5px] text-muted-foreground">
        <span>{dateUtc(first.published_at)}</span>
        <span>{dateUtc(last.published_at)}</span>
      </figcaption>
    </figure>
  )
}

const pick = (h: AnalogGroup['horizons'][number], p: number) =>
  h.percentiles.find((x) => x.p === p)?.return ?? null

/** Una fila por horizonte: p10, p50 y p90 del retorno, % positivos. */
function HorizonRows({ group }: { group: AnalogGroup }) {
  return (
    <table className="w-full text-[12.5px]">
      <thead>
        <tr className="border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
          <th className="py-1.5 font-bold">Sesiones después</th>
          <th className="py-1.5 text-right font-bold">p10</th>
          <th className="py-1.5 text-right font-bold">p50</th>
          <th className="py-1.5 text-right font-bold">p90</th>
          <th className="py-1.5 text-right font-bold">Positivos</th>
        </tr>
      </thead>
      <tbody>
        {group.horizons.map((h) => (
          <tr key={h.horizon} className="border-b border-border last:border-b-0">
            <td className="py-1.5">{h.horizon}</td>
            {[10, 50, 90].map((p) => {
              const v = pick(h, p)
              return (
                <td
                  key={p}
                  className={cn(
                    'py-1.5 text-right tabular-nums',
                    v && (v.startsWith('-') ? 'text-down' : 'text-up'),
                  )}
                >
                  {v === null ? '—' : `${signedPercent(v)} %`}
                </td>
              )
            })}
            <td className="py-1.5 text-right tabular-nums">{decimal(h.positive_pct, 0)} %</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

/**
 * La distribución de lo que pasó después de las situaciones más parecidas. Siempre con el
 * tamaño de la muestra y el aviso de que no es un pronóstico; con pocos vecinos, el aviso de
 * muestra chica.
 */
export function AnalogDistribution({ report }: { report: AnalogReport }) {
  const all = report.groups.find((g) => g.group === 'all')
  return (
    <div className="grid gap-3">
      <p className="text-[11.5px] text-muted-foreground">
        Ventana de {report.window} sesiones al {dateUtc(report.as_of)} ·{' '}
        {report.source === 'futures' ? 'diarias de futuros' : 'diarias de IG'}
      </p>
      {!all || all.count === 0 ? (
        <p className="text-[12.5px]">No hay vecinos para esta ventana.</p>
      ) : (
        <>
          <p className="text-[12.5px]">
            <strong>n = {all.count}</strong> situaciones parecidas
          </p>
          {all.insufficient && (
            <p
              role="alert"
              className="flex items-start gap-2 rounded-xl border border-[var(--chart-signal)]/50 bg-[var(--chart-signal)]/10 p-2.5 text-[12px]"
            >
              <AlertTriangle className="mt-0.5 size-4 shrink-0 text-[var(--chart-signal)]" />
              Muestra chica: {all.count} casos, menos que los {report.min_neighbors} que pide el
              mínimo. Datos insuficientes para leer esta distribución.
            </p>
          )}
          <HorizonRows group={all} />
        </>
      )}
      <p className="text-[11px] text-muted-foreground">
        Es la distribución de lo que pasó después, en el pasado.{' '}
        <strong>No es un pronóstico ni una recomendación</strong>.
      </p>
    </div>
  )
}
