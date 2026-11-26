/**
 * Replay de la Academia (#772, diseño §5): práctica vela a vela con velas del pasado, con marcas
 * de niveles y pivotes (#773) y un ticket de estudio (#791). El motor
 * elige instrumento, timeframe y fecha (solo del período de investigación) y guarda el cursor: la
 * UI pide «avanzar N» y dibuja solo las velas que vuelven, nunca una posterior. La fecha real no
 * viaja hasta el final. La UI no calcula nada de trading.
 */
import { useState, type KeyboardEvent } from 'react'

import { ReplayChart } from '@/components/ui/replay-chart'
import { MarkForm, SessionLog, StudyTicket, type Picked } from '@/components/ui/replay-tools'
import { decimal, priceDigits } from '@/lib/format'
import { GatewayError } from '@/lib/paper-api'
import {
  kept,
  keep,
  markReplay,
  openReplay,
  orderReplay,
  stepReplay,
  withStep,
  type Live,
} from '@/lib/replay-api'
import type { MarkRequest, OrderRequest } from '@/lib/replay-contracts'

const STEPS = [1, 5, 20, 100] as const

const ERRORS: Record<number, string> = {
  503: 'El motor no pudo armar una sesión con velas completas ahora. Probá de nuevo.',
  404: 'La sesión ya no está en el gateway (se cierra la más vieja o se reinició). Empezá otra.',
  403: 'El gateway rechazó el pedido: falta el token local (CFDS_GATEWAY_TOKEN en el proxy).',
}
const errorText = (e: unknown) =>
  (e instanceof GatewayError && ERRORS[e.status]) ||
  `No se pudo hablar con el motor (${e instanceof Error ? e.message : String(e)}).`

const button =
  'min-h-9 rounded-sm border border-border px-3 text-[12.5px] font-semibold hover:border-rule hover:bg-surface disabled:opacity-60'

export function ReplayView() {
  const [live, setLiveState] = useState<Live | null>(kept)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [seed, setSeed] = useState('')
  const [picked, setPicked] = useState<Picked>(null)
  const setLive = (l: Live | null) => {
    keep(l)
    setLiveState(l)
  }
  const run = async (task: () => Promise<void>) => {
    setBusy(true)
    setError(null)
    try {
      await task()
    } catch (e) {
      setError(errorText(e))
    } finally {
      setBusy(false)
    }
  }
  const start = () =>
    run(async () => {
      const s = await openReplay({ kind: 'practice', ...(seed.trim() && { seed: seed.trim() }) })
      setLive({ session: s, bars: s.bars, cursor: s.cursor, done: false, marks: [], orders: [] })
    })
  const step = (n: number) =>
    live &&
    run(async () => {
      const st = await stepReplay(live.session.session_id, n)
      setLive({ ...live, bars: withStep(live.bars, st), cursor: st.cursor, done: st.done })
    })
  const mark = (req: MarkRequest) =>
    live &&
    run(async () => {
      const m = await markReplay(live.session.session_id, req)
      setLive({ ...live, marks: [...live.marks, m] })
    })
  const order = (req: OrderRequest) =>
    live &&
    run(async () => {
      const o = await orderReplay(live.session.session_id, req)
      setLive({ ...live, orders: [...live.orders, o] })
    })

  const alert = error && (
    <p role="alert" className="text-[12.5px] text-down">
      {error}
    </p>
  )
  if (!live) {
    return (
      <section aria-labelledby="replay-empezar" className="panel grid max-w-[72ch] gap-3 p-3">
        <h2 id="replay-empezar" className="text-[15px] font-bold">
          Práctica vela a vela
        </h2>
        <p className="text-[13px] leading-relaxed text-muted-foreground">
          El motor elige al azar el instrumento, el timeframe y la fecha, solo del período de
          investigación (nunca de los meses fuera de muestra). La fecha queda oculta hasta el final:
          el eje dice día y hora UTC. El cursor vive en el motor, así que nunca se ve una vela
          futura. Práctica sin nota; todo en simulación.
        </p>
        <label className="grid max-w-72 gap-1 text-[12px]">
          <span className="font-semibold">Semilla (opcional, para repetir una sesión)</span>
          <input
            inputMode="numeric"
            autoComplete="off"
            value={seed}
            onChange={(e) => setSeed(e.target.value.replace(/\D/g, ''))}
            className="min-h-9 rounded-sm border border-border bg-surface px-2 font-mono text-[13px]"
          />
        </label>
        <button
          type="button"
          disabled={busy}
          onClick={start}
          className="min-h-9 justify-self-start rounded-sm bg-primary px-3 text-[12.5px] font-bold text-primary-foreground disabled:opacity-60"
        >
          {busy ? 'Pidiendo la sesión…' : 'Empezar práctica'}
        </button>
        {alert}
      </section>
    )
  }

  const { session: s, bars, cursor, done, marks, orders } = live
  const total = s.warmup_bars + s.exam_bars
  const digits = priceDigits(bars.slice(0, 50).flatMap((b) => [b.open, b.high, b.low, b.close]))
  const r = s.rules
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'ArrowRight' && !busy && !done && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault()
      void step(1)
    }
  }
  return (
    <section aria-label="Sesión de replay" className="grid min-w-0 gap-3" onKeyDown={onKey}>
      <dl className="panel flex flex-wrap gap-x-6 gap-y-1 px-2.5 py-2 text-[12px]">
        {[
          ['Instrumento', `${s.instrument_id} · ${s.timeframe}`],
          ['Velas', s.source === 'futures' ? 'de futuros (costos del CFD)' : 'del CFD de IG'],
          ['Vela', `${cursor + 1} de ${total}`],
          ['Semilla', s.seed],
        ].map(([k, v]) => (
          <div key={k} className="flex gap-2">
            <dt className="text-muted-foreground">{k}</dt>
            <dd className="font-mono font-semibold">{v}</dd>
          </div>
        ))}
      </dl>
      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 xl:grid-cols-[minmax(0,1fr)_20rem]">
        <div className="grid min-w-0 content-start gap-3">
          <ReplayChart
            bars={bars}
            digits={digits}
            onPick={setPicked}
            lines={[
              ...marks
                .filter((m) => m.kind === 'level')
                .map((m) => ({ price: m.price, label: `M${m.mark_id}`, tone: 'mark' as const })),
              ...orders.flatMap((o) =>
                (
                  [
                    [o.stop, 'stop'],
                    [o.target, 'obj.'],
                  ] as const
                ).flatMap(([p, l]) =>
                  p ? [{ price: p, label: `E${o.order_id} ${l}`, tone: 'order' as const }] : [],
                ),
              ),
            ]}
            points={marks.flatMap((m) =>
              m.bar_index === null
                ? []
                : [{ index: m.bar_index, price: m.price, label: `M${m.mark_id}` }],
            )}
          />
          <div
            role="group"
            aria-label="Avanzar velas"
            aria-keyshortcuts="ArrowRight"
            className="flex flex-wrap items-center gap-2"
          >
            {STEPS.map((n) => (
              <button
                key={n}
                type="button"
                disabled={busy || done}
                onClick={() => void step(n)}
                className={button}
              >
                Avanzar {n}
              </button>
            ))}
            <button type="button" disabled={busy} onClick={() => setLive(null)} className={button}>
              Nueva práctica
            </button>
            <span className="text-[11.5px] text-muted-foreground">
              → avanza una vela. Las velas las manda el motor hasta el cursor.
            </span>
          </div>
        </div>
        <div className="panel grid content-start gap-4 p-3">
          <MarkForm picked={picked} busy={busy} onMark={(r) => void mark(r)} />
          <StudyTicket at={bars.at(-1)} busy={busy || done} onOrder={(r) => void order(r)} />
        </div>
      </div>
      {alert}
      {done && (
        <p role="status" className="text-[13px] font-semibold">
          Llegaste a la última vela de la práctica.
        </p>
      )}
      <SessionLog marks={marks} orders={orders} bars={bars} digits={digits} />
      <p className="text-[12px] text-muted-foreground">
        Reglas con las que se califica un examen
        {r.provisional ? ' (provisorias, del dueño)' : ''}: al menos {r.trades_per_exam}{' '}
        operaciones; stop entre {decimal(r.stop_min_atr)} y {decimal(r.stop_max_atr)} ATR; riesgo de
        hasta {decimal(r.max_risk_pct)} % por operación; niveles con tolerancia de{' '}
        {decimal(r.level_tolerance_atr)} ATR. En la práctica no hay nota.
      </p>
    </section>
  )
}
