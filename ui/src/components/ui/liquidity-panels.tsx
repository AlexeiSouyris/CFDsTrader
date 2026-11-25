/**
 * Paneles de la vista de liquidez (#231, spec §5.4.1): la tabla de niveles con su estado, los FVG
 * abiertos y llenos, las estadísticas (tasas con IC 95 % de Wilson) y el índice de desequilibrio.
 * Todo llega calculado por el motor con la forma de sus contratos; acá solo se da formato
 * (CLAUDE.md §1).
 */
import { ScrollRegion } from '@/components/ui/scroll-region'
import { useState } from 'react'

import { roundedText } from '@/components/ui/rounded-text'
import { Term } from '@/components/ui/term'
import { dateTimeUtc, dateUtc, decimal, percent, signed } from '@/lib/format'
import { fvgKey, levelKey, levelName, STATE_LABEL } from '@/lib/liquidity-labels'
import { cn } from '@/lib/utils'
import type {
  FvgEvent,
  FvgPoint,
  Imbalance,
  LevelState,
  LiquidityLevel,
  LiquidityStats,
  StatsDistribution,
  StatsRate,
} from '@/lib/liquidity-contracts'

const STATE_TONE: Record<LevelState, string> = {
  intact: 'text-foreground',
  swept: 'text-[var(--chart-signal)]',
  broken: 'text-muted-foreground',
}
const DIRECTION = { bullish: 'Alcista', bearish: 'Bajista' } as const
const TERCILE = { low: 'Chico', mid: 'Mediano', high: 'Grande', all: 'Todos los tamaños' } as const
const FVG_STATUS = { open: 'Abierto', filled: 'Lleno', expired: 'Vencido' } as const

const th = 'py-2 font-bold'
const headRow =
  'border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase'
const bodyRow = 'border-b border-border last:border-b-0'
const table = 'w-full text-[12.5px] [&_td]:px-2 [&_th]:px-2'

/** Niveles con su estado. Los vencidos (`expired_at`) van atenuados y lo dicen. */
export function LevelsTable({ levels }: { levels: LiquidityLevel[] }) {
  return (
    <ScrollRegion label="Niveles de liquidez">
      <table className={cn(table, 'min-w-[560px]')}>
        <caption className="sr-only">Niveles de liquidez y su estado</caption>
        <thead>
          <tr className={headRow}>
            <th className={th}>Nivel</th>
            <th className={cn(th, 'text-right')}>Precio</th>
            <th className={th}>Estado</th>
            <th className={th}>Existe desde</th>
            <th className={th}>Cambió o venció</th>
          </tr>
        </thead>
        <tbody>
          {levels.map((l) => {
            const expired = l.state === 'intact' && l.expired_at != null
            return (
              <tr
                key={levelKey(l)}
                data-expired={expired || undefined}
                className={cn(bodyRow, expired && 'opacity-50')}
              >
                <td className="py-2 font-semibold">{levelName(l)}</td>
                <td className="py-2 text-right whitespace-nowrap tabular-nums">
                  {decimal(l.price)}
                </td>
                <td className={cn('py-2 font-semibold', STATE_TONE[l.state])}>
                  {STATE_LABEL[l.state]}
                  {expired && <span className="font-normal"> · vencido</span>}
                </td>
                <td className="py-2 whitespace-nowrap text-muted-foreground tabular-nums">
                  {dateTimeUtc(l.exists_from)}
                </td>
                <td className="py-2 whitespace-nowrap text-muted-foreground tabular-nums">
                  {l.state_changed_at
                    ? dateTimeUtc(l.state_changed_at)
                    : expired
                      ? `venció ${dateTimeUtc(l.expired_at!)}`
                      : '—'}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </ScrollRegion>
  )
}

const point = (m: FvgPoint) => (m === null ? '—' : `${m.bars} ${m.bars === 1 ? 'vela' : 'velas'}`)
const atrText = (s: string | null) => (s === null ? '—' : `${signed(s)} ATR`)

/** Cuántos FVG se ven antes de "Mostrar todos": la lista entera alarga mucho el Panel (#357). */
const FVG_PREVIEW = 8

export function FvgTable({
  fvgs,
  preview = FVG_PREVIEW,
  label = 'FVG',
}: {
  fvgs: FvgEvent[]
  preview?: number
  /** nombre de la tabla: con dos en la misma vista, cada una el suyo */
  label?: string
}) {
  const [all, setAll] = useState(false)
  const horizons = fvgs[0]?.reactions.map((r) => r.bars) ?? []
  const rows = [...fvgs].reverse()
  const shown = all ? rows : rows.slice(0, preview)
  const hidden = rows.length - shown.length
  return (
    <div className="grid gap-2">
      <ScrollRegion label={label}>
        <table className={cn(table, 'min-w-[720px] [&_td]:whitespace-nowrap')}>
          <caption className="sr-only">FVG, del más nuevo al más viejo</caption>
          <thead>
            <tr className={headRow}>
              <th className={th}>Creado</th>
              <th className={th}>Dirección</th>
              <th className={cn(th, 'text-right')}>Zona</th>
              <th className={cn(th, 'text-right')}>Tamaño</th>
              <th className={cn(th, 'text-right')}>Toque</th>
              <th className={cn(th, 'text-right')}>Mitad</th>
              <th className={cn(th, 'text-right')}>Llenado</th>
              <th className={cn(th, 'text-right')}>Reacción {horizons.join(' · ')}</th>
              <th className={th}>Estado</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((f) => (
              <tr key={fvgKey(f)} className={bodyRow}>
                <td className="py-2 text-muted-foreground tabular-nums">
                  {dateTimeUtc(f.created_at)}
                </td>
                <td
                  className={cn(
                    'py-2 font-semibold',
                    f.direction === 'bullish' ? 'text-up' : 'text-down',
                  )}
                >
                  {DIRECTION[f.direction]}
                </td>
                <td className="py-2 text-right tabular-nums">
                  {decimal(f.bottom)} – {decimal(f.top)}
                </td>
                <td className="py-2 text-right tabular-nums">
                  {decimal(roundedText(f.size_atr))} ATR
                </td>
                <td className="py-2 text-right tabular-nums">{point(f.touch)}</td>
                <td className="py-2 text-right tabular-nums">{point(f.mid)}</td>
                <td className="py-2 text-right tabular-nums">{point(f.fill)}</td>
                <td className="py-2 text-right tabular-nums">
                  {f.reactions.map((r) => atrText(r.return_atr)).join(' · ')}
                </td>
                <td
                  className={cn(
                    'py-2 font-semibold',
                    f.status !== 'open' && 'text-muted-foreground',
                  )}
                >
                  {FVG_STATUS[f.status]}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </ScrollRegion>
      {rows.length > preview && (
        <button
          type="button"
          aria-expanded={all}
          onClick={() => setAll((v) => !v)}
          className="justify-self-start rounded-full px-2 py-1 text-[12px] font-bold text-muted-foreground underline-offset-4 hover:text-foreground hover:underline focus-visible:outline-2 focus-visible:outline-ring"
        >
          {all ? `Mostrar solo los ${preview} más nuevos` : `Mostrar los ${hidden} restantes`}
        </button>
      )}
    </div>
  )
}

/** Una tasa con su IC, o "datos insuficientes". El número viene del motor; acá solo se escribe. */
export function Rate({ stat, minCases }: { stat: StatsRate | undefined; minCases: number }) {
  if (!stat) return <span className="text-muted-foreground">—</span>
  if (stat.insufficient || stat.rate === null || stat.ci_low === null || stat.ci_high === null)
    return (
      <span className="text-muted-foreground">
        Datos insuficientes
        <span className="block text-[10.5px]">
          n = {stat.n} de {minCases}
        </span>
      </span>
    )
  return (
    <span>
      <strong className="font-bold">{percent(stat.rate)} %</strong>
      <span className="block text-[10.5px] text-muted-foreground">
        IC 95 %: {percent(stat.ci_low)}–{percent(stat.ci_high)} · n = {stat.n}
      </span>
    </span>
  )
}

const rateOf = (s: LiquidityStats, metric: StatsRate['metric']) =>
  s.rates.find((r) => r.metric === metric)
const distOf = (s: LiquidityStats, metric: StatsDistribution['metric'], horizon?: number) =>
  s.distributions.find(
    (d) => d.metric === metric && (horizon === undefined || d.horizon_bars === horizon),
  )
const median = (d: StatsDistribution | undefined) =>
  d && !d.insufficient && d.median !== null ? decimal(d.median) : '—'

function Reaction({ d }: { d: StatsDistribution | undefined }) {
  if (!d || d.insufficient || d.median === null) return <>—</>
  return (
    <>
      {signed(d.median)} ATR
      {d.p25 !== null && d.p75 !== null && (
        <span className="block text-[10.5px] text-muted-foreground">
          p25 {signed(d.p25)} · p75 {signed(d.p75)}
        </span>
      )}
    </>
  )
}

const groupKey = (s: LiquidityStats) =>
  `${s.subject}:${s.timeframe}:${s.direction}:${s.size_tercile}`

export function StatsPanel({ stats }: { stats: LiquidityStats[] }) {
  const fvg = stats.filter((s) => s.subject === 'fvg')
  const gaps = stats.filter((s) => s.subject === 'opening_gap')
  const horizon = fvg
    .flatMap((s) => s.distributions)
    .reduce<number | null>((h, d) => (d.metric === 'reaction' ? d.horizon_bars : h), null)
  return (
    <div className="grid gap-4">
      <p className="text-[11.5px] text-muted-foreground">
        Solo eventos ya cerrados
        {stats[0] ? `, hasta el ${dateUtc(stats[0].as_of)}` : ''}. Con menos de{' '}
        {stats[0]?.min_cases ?? '—'} casos no se muestra la tasa. Son frecuencias pasadas, no una
        predicción.
      </p>
      <ScrollRegion label="FVG por timeframe, dirección y tamaño">
        <table className={cn(table, 'min-w-[640px]')}>
          <caption className="mb-2 text-left text-[12px] font-bold">
            <Term id="fvg">FVG</Term> por timeframe, dirección y tamaño
          </caption>
          <thead>
            <tr className={headRow}>
              <th className={th}>Grupo</th>
              <th className={th}>Toque</th>
              <th className={th}>Mitad</th>
              <th className={th}>Llenado</th>
              <th className={cn(th, 'text-right')}>Mediana de velas</th>
              <th className={cn(th, 'text-right')}>
                Reacción{horizon !== null ? ` a ${horizon} velas` : ''}
              </th>
            </tr>
          </thead>
          <tbody>
            {fvg.map((s) => (
              <tr key={groupKey(s)} className={cn(bodyRow, 'align-top')}>
                <td className="py-2 font-semibold">
                  {s.timeframe} · {DIRECTION[s.direction]} · {TERCILE[s.size_tercile]}
                  <span className="block text-[10.5px] font-normal text-muted-foreground">
                    {s.n} casos
                  </span>
                </td>
                <td className="py-2">
                  <Rate stat={rateOf(s, 'touch')} minCases={s.min_cases} />
                </td>
                <td className="py-2">
                  <Rate stat={rateOf(s, 'mid')} minCases={s.min_cases} />
                </td>
                <td className="py-2">
                  <Rate stat={rateOf(s, 'fill')} minCases={s.min_cases} />
                </td>
                <td className="py-2 text-right tabular-nums">
                  {median(distOf(s, 'bars_to_touch'))} · {median(distOf(s, 'bars_to_mid'))} ·{' '}
                  {median(distOf(s, 'bars_to_fill'))}
                </td>
                <td className="py-2 text-right tabular-nums">
                  <Reaction d={distOf(s, 'reaction', horizon ?? undefined)} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </ScrollRegion>

      <ScrollRegion label="Gaps de apertura (diario)">
        <table className={cn(table, 'min-w-[520px]')}>
          <caption className="mb-2 text-left text-[12px] font-bold">
            <Term id="gap">Gaps</Term> de apertura (diario)
          </caption>
          <thead>
            <tr className={headRow}>
              <th className={th}>Dirección</th>
              <th className={th}>Se llena en la sesión</th>
              <th className={th}>Se llena antes de vencer</th>
              <th className={cn(th, 'text-right')}>Mediana de sesiones</th>
            </tr>
          </thead>
          <tbody>
            {gaps.map((s) => (
              <tr key={groupKey(s)} className={cn(bodyRow, 'align-top')}>
                <td className="py-2 font-semibold">
                  {s.direction === 'bullish' ? 'Gap al alza' : 'Gap a la baja'}
                  <span className="block text-[10.5px] font-normal text-muted-foreground">
                    {s.n} casos
                  </span>
                </td>
                <td className="py-2">
                  <Rate stat={rateOf(s, 'same_session_fill')} minCases={s.min_cases} />
                </td>
                <td className="py-2">
                  <Rate stat={rateOf(s, 'fill')} minCases={s.min_cases} />
                </td>
                <td className="py-2 text-right tabular-nums">
                  {median(distOf(s, 'sessions_to_fill'))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </ScrollRegion>
    </div>
  )
}

/** Índice de −1 a +1 tal como lo manda el motor, con su rótulo. La posición es solo dibujo. */
export function ImbalanceGauge({ imbalance }: { imbalance: Imbalance }) {
  const pos = ((Math.max(-1, Math.min(1, Number(imbalance.index))) + 1) / 2) * 100
  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <strong className="text-[clamp(22px,2.4vw,30px)] leading-none font-extrabold tracking-[-0.04em] tabular-nums">
          {signed(roundedText(imbalance.index))}
        </strong>
        <span className="rounded-full border border-border px-2 py-0.5 text-[10.5px] font-bold text-muted-foreground">
          {imbalance.label}
        </span>
      </div>
      <div
        role="meter"
        aria-label={`Índice de desequilibrio, ${imbalance.label}`}
        aria-valuemin={-1}
        aria-valuemax={1}
        aria-valuenow={Number(imbalance.index)}
        aria-valuetext={signed(roundedText(imbalance.index))}
        className="relative h-2 rounded-full bg-gradient-to-r from-down/50 via-border to-up/50"
      >
        <span
          className="absolute top-1/2 size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-background bg-foreground"
          style={{ left: `${pos}%` }}
        />
      </div>
      <div className="flex justify-between text-[10.5px] text-muted-foreground">
        <span>−1 bajista</span>
        <span>0</span>
        <span>+1 alcista</span>
      </div>
      <p className="text-[11.5px] text-muted-foreground">
        Últimas {imbalance.window_bars} velas cerradas de {imbalance.timeframe}, al{' '}
        {dateTimeUtc(imbalance.at)}. Sale solo de las velas de desplazamiento:{' '}
        <strong>no es flujo de órdenes</strong> (IG no da volumen ni libro de órdenes).
      </p>
    </div>
  )
}
