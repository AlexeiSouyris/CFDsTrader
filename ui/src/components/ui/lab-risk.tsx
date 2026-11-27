/**
 * Calculadora de riesgo del laboratorio (#795, `docs/design/2026-11-laboratorio.md` §4.1): con un
 * capital hipotético, qué tamaño daría el módulo de riesgo del motor para un stop, cuánto se
 * pierde si salta y qué límite recorta o rechaza (`POST /api/v1/lab/risk_preview`,
 * `risk_preview.v1`). "En vivo": pide de nuevo al cambiar un campo, con una espera corta.
 *
 * **La UI no hace cuentas** (CLAUDE.md §1): el tamaño, el margen, la pérdida y los costos los
 * calcula el mismo `cfds_risk::check` del broker simulado. Es simulación: no manda nada.
 */
import { useEffect, useId, useState } from 'react'

import { Segmented } from '@/components/ui/segmented'
import { dateTimeUtc, decimal } from '@/lib/format'
import { previewRisk } from '@/lib/lab-api'
import { instrumentLabel, useInstrumentNames } from '@/lib/liquidity-api'
import type { RiskPreview, RiskPreviewRequest, StrategyCatalog } from '@/lib/lab-contracts'
import { equityText, schemaOf } from '@/lib/lab-spec'
import { REJECTION_LABEL } from '@/lib/paper-contracts'
import { cn } from '@/lib/utils'

/** Espera desde el último cambio antes de pedir. */
const RISK_DEBOUNCE_MS = 400

const field =
  'mt-1 block min-h-10 w-full min-w-0 rounded-sm border border-border bg-surface px-2.5 py-1.5 text-[13px] tabular-nums outline-none transition-colors hover:border-foreground/30 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40'
const label =
  'block min-w-0 text-[11px] font-bold tracking-[0.06em] text-muted-foreground uppercase'

/** En el laboratorio las velas son las guardadas de futuros, no las de IG en vivo. */
const LABEL = {
  ...REJECTION_LABEL,
  no_recent_candles: 'Pocas velas guardadas para calcular el ATR (calentamiento).',
  missing_fx_rate: 'Falta el tipo de cambio publicado para un contrato en otra moneda.',
}

const isDecimal = (s: string) => /^\d+(\.\d+)?$/.test(s)
const dec = (s: string) => s.trim().replace(',', '.')

type Form = {
  instrument_id: string
  timeframe: RiskPreviewRequest['timeframe']
  direction: RiskPreviewRequest['direction']
  equity: string
  risk_pct: string
  stop: 'atr' | 'price'
  k: string
  price: string
  atr_period: string
}

/** El pedido, o `null` mientras falte algo por escribir (los rangos los juzga el motor). */
function toRequest(f: Form): RiskPreviewRequest | null {
  const equity = equityText(f.equity)
  const period = /^\d{1,3}$/.test(f.atr_period.trim()) ? Number(f.atr_period) : NaN
  const stop =
    f.stop === 'atr'
      ? isDecimal(dec(f.k)) && { kind: 'atr' as const, k: dec(f.k) }
      : isDecimal(dec(f.price)) && { kind: 'price' as const, price: dec(f.price) }
  if (!equity || !isDecimal(dec(f.risk_pct)) || !stop || !(period >= 1)) return null
  return {
    instrument_id: f.instrument_id,
    timeframe: f.timeframe,
    direction: f.direction,
    equity,
    risk_pct: dec(f.risk_pct),
    atr_period: period,
    stop,
  }
}

function Result({ r }: { r: RiskPreview }) {
  const money = (v: string | null, cur: string) => (v == null ? '—' : `${decimal(v, 2)} ${cur}`)
  const items: [string, string, string?][] = r.accepted
    ? [
        ['Tamaño', `${decimal(r.size!, 4)} ${r.instrument_currency}/punto`],
        ['Nocional', money(r.notional, r.instrument_currency)],
        [
          'Apalancamiento',
          `${decimal(r.leverage!, 2)} ×${r.leverage_capped ? ' (tope)' : ''}`,
          r.leverage_capped ? 'text-info' : undefined,
        ],
        ['Margen', money(r.margin, r.instrument_currency)],
        [
          'Pérdida si salta el stop',
          `${money(r.risk, r.account_currency)} (${decimal(r.risk_pct!, 2)} %)`,
          'text-down',
        ],
        ['Costo de ida y vuelta', money(r.round_trip_cost, r.account_currency)],
      ]
    : []
  return (
    <div className="grid gap-2" data-risk-accepted={r.accepted}>
      {r.accepted ? (
        <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[12.5px]">
          {items.map(([k, v, cls]) => (
            <div key={k} className="min-w-0">
              <dt className="text-[11px] text-muted-foreground">{k}</dt>
              <dd className={cn('font-semibold tabular-nums', cls)}>{v}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <p role="status" className="text-[13px] text-down">
          El módulo de riesgo lo rechaza: {r.rejection ? LABEL[r.rejection] : 'sin motivo'}
        </p>
      )}
      <p className="text-[11.5px] text-muted-foreground tabular-nums">
        Referencia {r.reference ? decimal(r.reference, 2) : '—'}
        {r.as_of && ` (cierre ${dateTimeUtc(r.as_of)})`} · ATR {r.atr ? decimal(r.atr, 2) : '—'} ·
        stop {r.stop ? decimal(r.stop, 2) : '—'}
        {r.stop_atr &&
          ` (${decimal(r.stop_atr, 2)} ATR; permitido ${r.limits.min_stop_atr} a ${r.limits.max_stop_atr})`}{' '}
        · riesgo aplicado {r.risk_pct_applied} % (máximo {r.limits.max_risk_pct} %)
      </p>
      <ul className="grid gap-0.5 text-[11.5px] text-muted-foreground">
        {r.notes.map((n) => (
          <li key={n}>· {n}</li>
        ))}
      </ul>
    </div>
  )
}

export function RiskCalculator({ cat }: { cat: StrategyCatalog }) {
  const id = useId()
  const names = useInstrumentNames()
  const atr = schemaOf(cat, 'atr')?.params
  const pick = (name: string) => {
    const p = atr?.find((x) => x.name === name)
    return p && p.default != null ? String(p.default) : ''
  }
  const [form, setForm] = useState<Form>(() => ({
    instrument_id: cat.markets[0]?.instrument_id ?? '',
    timeframe: cat.markets[0]?.timeframes[0] ?? '1h',
    direction: 'long',
    equity: '',
    risk_pct: cat.limits.max_risk_pct,
    stop: 'atr',
    k: pick('k'),
    price: '',
    atr_period: pick('period'),
  }))
  const [result, setResult] = useState<{
    key: string
    r: RiskPreview | null
    error: string | null
  }>({ key: '', r: null, error: null })
  const req = toRequest(form)
  const key = req ? JSON.stringify(req) : ''
  useEffect(() => {
    if (!req) return
    const ctrl = new AbortController()
    const timer = window.setTimeout(() => {
      previewRisk(req, ctrl.signal).then(
        (r) => setResult({ key, r, error: null }),
        (e: unknown) => {
          if (!ctrl.signal.aborted)
            setResult({ key, r: null, error: e instanceof Error ? e.message : String(e) })
        },
      )
    }, RISK_DEBOUNCE_MS)
    return () => {
      window.clearTimeout(timer)
      ctrl.abort()
    }
    // `req` cambia con `key`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])
  const set = (patch: Partial<Form>) => setForm((f) => ({ ...f, ...patch }))
  const current = result.key === key ? result : null
  const market = cat.markets.find((m) => m.instrument_id === form.instrument_id)
  return (
    <section
      aria-labelledby={`${id}-title`}
      className="grid min-w-0 gap-3 rounded-sm border border-border bg-card p-3"
    >
      <header>
        <p className="text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase">
          Sin efectos · cuenta hipotética
        </p>
        <h3 id={`${id}-title`} className="text-[15px] font-semibold">
          Calculadora de riesgo
        </h3>
      </header>
      <div className="grid gap-3 sm:grid-cols-2">
        <label className={label}>
          Instrumento
          <select
            className={field}
            value={form.instrument_id}
            onChange={(e) => set({ instrument_id: e.target.value })}
          >
            {cat.markets.map((m) => (
              <option key={m.instrument_id} value={m.instrument_id}>
                {instrumentLabel(names, m.instrument_id)}
              </option>
            ))}
          </select>
        </label>
        <label className={label}>
          Capital (EUR, hipotético)
          <input
            className={field}
            inputMode="decimal"
            placeholder="por ejemplo 100000"
            value={form.equity}
            onChange={(e) => set({ equity: e.target.value })}
          />
        </label>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <Segmented
          label="Timeframe del ATR"
          options={market?.timeframes ?? ['1h']}
          value={form.timeframe}
          onChange={(v) => set({ timeframe: v as Form['timeframe'] })}
        />
        <Segmented
          label="Dirección de la calculadora"
          options={['long', 'short']}
          value={form.direction}
          format={(o) => (o === 'long' ? 'Largo' : 'Corto')}
          onChange={(v) => set({ direction: v as Form['direction'] })}
        />
        <Segmented
          label="Tipo de stop"
          options={['atr', 'price']}
          value={form.stop}
          format={(o) => (o === 'atr' ? 'k × ATR' : 'Precio')}
          onChange={(v) => set({ stop: v as Form['stop'] })}
        />
      </div>
      <div className="grid grid-cols-3 gap-3">
        <label className={label}>
          Riesgo (%)
          <input
            className={field}
            inputMode="decimal"
            value={form.risk_pct}
            onChange={(e) => set({ risk_pct: e.target.value })}
          />
        </label>
        {form.stop === 'atr' ? (
          <label className={label}>
            k (ATR)
            <input
              className={field}
              inputMode="decimal"
              value={form.k}
              onChange={(e) => set({ k: e.target.value })}
            />
          </label>
        ) : (
          <label className={label}>
            Precio del stop
            <input
              className={field}
              inputMode="decimal"
              value={form.price}
              onChange={(e) => set({ price: e.target.value })}
            />
          </label>
        )}
        <label className={label}>
          Período del ATR
          <input
            className={field}
            inputMode="numeric"
            value={form.atr_period}
            onChange={(e) => set({ atr_period: e.target.value })}
          />
        </label>
      </div>
      <div aria-live="polite" className="min-h-6">
        {!req && (
          <p className="text-[12px] text-muted-foreground">
            Escribí el capital, el riesgo y el stop: el motor responde al momento.
          </p>
        )}
        {req && !current && <p className="text-[12px] text-muted-foreground">Calculando…</p>}
        {current?.error && <p className="text-[12.5px] text-down">{current.error}</p>}
        {current?.r && <Result r={current.r} />}
      </div>
    </section>
  )
}
