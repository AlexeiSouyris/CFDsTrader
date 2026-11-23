/**
 * Paneles de la vista de liquidez (#231, spec §5.4.1): la tabla de niveles con su estado, los FVG
 * abiertos y llenos, las estadísticas (tasas con IC 95 % de Wilson) y el índice de desequilibrio.
 * Todo llega calculado por el motor; acá solo se da formato (CLAUDE.md §1).
 */
import { Term } from '@/components/ui/term'
import { dateTimeUtc, dateUtc, decimal, signed } from '@/lib/format'
import { levelName, STATE_LABEL } from '@/lib/liquidity-labels'
import { cn } from '@/lib/utils'
import type {
  FvgEvent,
  FvgMilestone,
  FvgStatsRow,
  Imbalance,
  LevelState,
  LiquidityLevel,
  LiquidityStats,
  RateStat,
} from '@/mocks'

const STATE_TONE: Record<LevelState, string> = {
  intact: 'text-foreground',
  swept_reversal: 'text-[var(--chart-signal)]',
  broken: 'text-muted-foreground',
}
const DIRECTION = { bullish: 'Alcista', bearish: 'Bajista' } as const
const TERCILE = { small: 'Chico', medium: 'Mediano', large: 'Grande' } as const
const FVG_STATUS = { open: 'Abierto', filled: 'Lleno', expired: 'Vencido' } as const

const th = 'py-2 font-bold'
const headRow =
  'border-b border-border text-left text-[10px] font-bold tracking-[0.1em] text-muted-foreground uppercase'
const bodyRow = 'border-b border-border last:border-b-0'

export function LevelsTable({ levels }: { levels: LiquidityLevel[] }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[560px] text-[12.5px] [&_td]:px-2 [&_th]:px-2">
        <caption className="sr-only">Niveles de liquidez y su estado</caption>
        <thead>
          <tr className={headRow}>
            <th className={th}>Nivel</th>
            <th className={cn(th, 'text-right')}>Precio</th>
            <th className={th}>Estado</th>
            <th className={th}>Existe desde</th>
            <th className={th}>Cambió en</th>
          </tr>
        </thead>
        <tbody>
          {levels.map((l) => (
            <tr key={l.id} className={bodyRow}>
              <td className="py-2 font-semibold">
                {levelName(l)}
                {l.touches !== null && (
                  <span className="ml-1 text-[11px] font-normal text-muted-foreground">
                    · {l.touches} pivotes
                  </span>
                )}
              </td>
              <td className="py-2 text-right tabular-nums">{decimal(l.price)}</td>
              <td className={cn('py-2 font-semibold', STATE_TONE[l.state])}>
                {STATE_LABEL[l.state]}
              </td>
              <td className="py-2 text-muted-foreground">{dateTimeUtc(l.exists_from)}</td>
              <td className="py-2 text-muted-foreground">
                {l.state_changed_at ? dateTimeUtc(l.state_changed_at) : '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

const milestone = (m: FvgMilestone) =>
  m === null ? '—' : `${m.bars} ${m.bars === 1 ? 'vela' : 'velas'}`
const atrText = (s: string | null) => (s === null ? '—' : `${signed(s)} ATR`)

export function FvgTable({ fvgs }: { fvgs: FvgEvent[] }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[640px] text-[12.5px] [&_td]:px-2 [&_th]:px-2">
        <caption className="sr-only">FVG abiertos y llenos</caption>
        <thead>
          <tr className={headRow}>
            <th className={th}>Creado</th>
            <th className={th}>Dirección</th>
            <th className={cn(th, 'text-right')}>Zona</th>
            <th className={cn(th, 'text-right')}>Tamaño</th>
            <th className={cn(th, 'text-right')}>Toque</th>
            <th className={cn(th, 'text-right')}>Mitad</th>
            <th className={cn(th, 'text-right')}>Llenado</th>
            <th className={cn(th, 'text-right')}>Reacción 4 · 8</th>
            <th className={th}>Estado</th>
          </tr>
        </thead>
        <tbody>
          {[...fvgs].reverse().map((f) => (
            <tr key={f.id} className={bodyRow}>
              <td className="py-2 text-muted-foreground">{dateTimeUtc(f.created_at)}</td>
              <td
                className={cn(
                  'py-2 font-semibold',
                  f.direction === 'bullish' ? 'text-up' : 'text-down',
                )}
              >
                {DIRECTION[f.direction]}
              </td>
              <td className="py-2 text-right tabular-nums">
                {decimal(f.zone_low)} – {decimal(f.zone_high)}
              </td>
              <td className="py-2 text-right tabular-nums">{decimal(f.size_atr)} ATR</td>
              <td className="py-2 text-right tabular-nums">{milestone(f.first_touch)}</td>
              <td className="py-2 text-right tabular-nums">{milestone(f.midpoint)}</td>
              <td className="py-2 text-right tabular-nums">{milestone(f.filled)}</td>
              <td className="py-2 text-right tabular-nums">
                {atrText(f.reaction_atr.bars_4)} · {atrText(f.reaction_atr.bars_8)}
              </td>
              <td
                className={cn('py-2 font-semibold', f.status !== 'open' && 'text-muted-foreground')}
              >
                {FVG_STATUS[f.status]}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

/** Una tasa con su IC, o "datos insuficientes". El número viene del motor; acá solo se escribe. */
export function Rate({ stat }: { stat: RateStat }) {
  if (stat.status === 'insufficient')
    return (
      <span className="text-muted-foreground">
        Datos insuficientes
        <span className="block text-[10.5px]">
          n = {stat.n} de {stat.min_cases}
        </span>
      </span>
    )
  return (
    <span>
      <strong className="font-bold">{decimal(stat.rate_pct)} %</strong>
      <span className="block text-[10.5px] text-muted-foreground">
        IC 95 %: {decimal(stat.ci95_low_pct)}–{decimal(stat.ci95_high_pct)} · n = {stat.n}
      </span>
    </span>
  )
}

const medianBars = (v: number | null) => (v === null ? '—' : String(v))

export function StatsPanel({ stats }: { stats: LiquidityStats }) {
  return (
    <div className="grid gap-4">
      <p className="text-[11.5px] text-muted-foreground">
        Histórico del {dateUtc(stats.sample_from)} al {dateUtc(stats.sample_to)}, solo con eventos
        ya cerrados. Con menos de {stats.min_cases} casos no se muestra la tasa. Son frecuencias
        pasadas, no una predicción.
      </p>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[640px] text-[12.5px] [&_td]:px-2 [&_th]:px-2">
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
              <th className={cn(th, 'text-right')}>Reacción a 8 velas</th>
            </tr>
          </thead>
          <tbody>
            {stats.fvg.map((r: FvgStatsRow) => (
              <tr
                key={`${r.timeframe}-${r.direction}-${r.size_tercile}`}
                className={cn(bodyRow, 'align-top')}
              >
                <td className="py-2 font-semibold">
                  {r.timeframe} · {DIRECTION[r.direction]} · {TERCILE[r.size_tercile]}
                  <span className="block text-[10.5px] font-normal text-muted-foreground">
                    {r.cases} casos
                  </span>
                </td>
                <td className="py-2">
                  <Rate stat={r.touch} />
                </td>
                <td className="py-2">
                  <Rate stat={r.midpoint} />
                </td>
                <td className="py-2">
                  <Rate stat={r.fill} />
                </td>
                <td className="py-2 text-right tabular-nums">
                  {medianBars(r.median_bars.touch)} · {medianBars(r.median_bars.midpoint)} ·{' '}
                  {medianBars(r.median_bars.fill)}
                </td>
                <td className="py-2 text-right tabular-nums">
                  {r.reaction_atr_8 === null ? (
                    '—'
                  ) : (
                    <>
                      {signed(r.reaction_atr_8.median)} ATR
                      <span className="block text-[10.5px] text-muted-foreground">
                        RIC {signed(r.reaction_atr_8.q1)} a {signed(r.reaction_atr_8.q3)}
                      </span>
                    </>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full min-w-[520px] text-[12.5px] [&_td]:px-2 [&_th]:px-2">
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
            {stats.gaps.map((g) => (
              <tr key={g.direction} className={cn(bodyRow, 'align-top')}>
                <td className="py-2 font-semibold">
                  {g.direction === 'up' ? 'Gap al alza' : 'Gap a la baja'}
                  <span className="block text-[10.5px] font-normal text-muted-foreground">
                    {g.cases} casos
                  </span>
                </td>
                <td className="py-2">
                  <Rate stat={g.fill_same_session} />
                </td>
                <td className="py-2">
                  <Rate stat={g.fill_within_max} />
                </td>
                <td className="py-2 text-right tabular-nums">{g.median_sessions ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

/** Índice de −1 a +1 tal como lo manda el motor. La posición del marcador es solo dibujo. */
export function ImbalanceGauge({ imbalance }: { imbalance: Imbalance }) {
  const pos = ((Math.max(-1, Math.min(1, Number(imbalance.index))) + 1) / 2) * 100
  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <strong className="text-[clamp(22px,2.4vw,30px)] leading-none font-extrabold tracking-[-0.04em]">
          {signed(imbalance.index)}
        </strong>
        <span className="rounded-full border border-border px-2 py-0.5 text-[10.5px] font-bold text-muted-foreground">
          aproximación desde el precio
        </span>
      </div>
      <div
        role="meter"
        aria-label="Índice de desequilibrio, aproximación desde el precio"
        aria-valuemin={-1}
        aria-valuemax={1}
        aria-valuenow={Number(imbalance.index)}
        aria-valuetext={signed(imbalance.index)}
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
        Últimas {imbalance.window_bars} velas cerradas de 15 m, al {dateTimeUtc(imbalance.as_of)}.
        Sale solo de las velas de desplazamiento: <strong>no es flujo de órdenes</strong> (IG no da
        volumen ni libro de órdenes).
      </p>
    </div>
  )
}
