/**
 * Estados emocionales de la spec §9.1 (#220) de un instrumento: cada uno como una etiqueta con
 * los valores de su regla, tal como llegan en `emotional_state.v1`. **No calcula nada** (solo
 * pone nombres y formatea los números con coma) y no es un pronóstico.
 */
import type { ReactNode } from 'react'

import { Term } from '@/components/ui/term'
import type { EmotionalState, EmotionKind } from '@/lib/context-contracts'
import { dateTimeUtc, decimal } from '@/lib/format'
import { cn } from '@/lib/utils'

const LABEL: Record<EmotionKind, ReactNode> = {
  fomo: <Term id="fomo">FOMO de precio</Term>,
  selling_climax: <Term id="capitulacion">Clímax de venta</Term>,
  rejection: <Term id="rechazo">Rechazo</Term>,
  extreme_fear: 'Miedo extremo',
  extreme_euphoria: 'Euforia extrema',
  complacency: <Term id="complacencia">Complacencia</Term>,
}

const TONE: Record<EmotionKind, string> = {
  fomo: 'border-[var(--chart-signal)]/50',
  extreme_euphoria: 'border-[var(--chart-signal)]/50',
  selling_climax: 'border-down/40',
  extreme_fear: 'border-down/40',
  rejection: 'border-border',
  complacency: 'border-border',
}

const BASIS: Record<EmotionalState['basis'], string> = {
  '15m': 'Velas de 15m',
  '1h': 'Velas de 1h',
  sentiment_index: 'Índice de sentimiento',
}

const VIA: Record<string, string> = {
  level: 'soporte o resistencia',
  fvg: 'FVG',
  order_block: 'order block',
  sweep: 'sweep',
}

const num = (v: string | undefined) => (v === undefined ? '—' : decimal(v))

/** La regla con sus valores, en palabras. Solo arma el texto. */
function ruleText(state: EmotionKind, v: Record<string, string>): string {
  switch (state) {
    case 'fomo':
    case 'selling_climax':
      return (
        `(cierre − EMA) / ATR = ${num(v.stretch)} (umbral ${num(v.atr_multiple)}) · ` +
        `cierre ${num(v.close)}, EMA ${num(v.ema)}, ATR ${num(v.atr)} · ` +
        `${state === 'fomo' ? 'máximo' : 'mínimo'} de ${v.lookback ?? '—'} velas`
      )
    case 'rejection':
      return (
        `${v.side === 'short' ? 'bajista' : 'alcista'} · por ` +
        (v.via ?? '')
          .split(',')
          .filter(Boolean)
          .map((x) => VIA[x] ?? x)
          .join(', ')
      )
    case 'extreme_fear':
      return `índice ${num(v.index)} ≤ ${num(v.threshold)}`
    case 'extreme_euphoria':
      return `índice ${num(v.index)} ≥ ${num(v.threshold)}`
    case 'complacency':
      return `percentil de la volatilidad ${num(v.percentile)} ≤ ${num(v.threshold)}`
  }
}

const ORDER: EmotionalState['basis'][] = ['sentiment_index', '1h', '15m']

export function EmotionalStates({ rows }: { rows: EmotionalState[] }) {
  const sorted = [...rows].sort((a, b) => ORDER.indexOf(a.basis) - ORDER.indexOf(b.basis))
  return (
    <div className="grid gap-3" aria-label="Estados emocionales">
      {sorted.map((r) => (
        <section key={`${r.source}|${r.basis}`} className="grid gap-1.5" data-basis={r.basis}>
          <p className="flex flex-wrap justify-between gap-x-2 text-[11px] text-muted-foreground">
            <span className="font-bold tracking-[0.06em] uppercase">{BASIS[r.basis]}</span>
            <span>{dateTimeUtc(r.as_of)}</span>
          </p>
          {r.states.length === 0 ? (
            <p className="text-[12.5px] text-muted-foreground">Ninguno activo.</p>
          ) : (
            <ul className="grid gap-1.5">
              {r.states.map((s, i) => (
                <li
                  key={`${s.state}-${i}`}
                  data-state={s.state}
                  className={cn('min-w-0 rounded-xl border px-3 py-2', TONE[s.state])}
                >
                  <span className="text-[12.5px] font-bold">{LABEL[s.state]}</span>
                  <span className="mt-0.5 block text-[11.5px] break-words text-muted-foreground tabular-nums">
                    {ruleText(s.state, s.values)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
      ))}
    </div>
  )
}
