/**
 * Marcas (#773) y ticket de estudio (#791) del replay. Una marca (nivel o pivote) y una entrada van
 * al motor con el cursor que pone él (`replay_mark.v1`, `replay_order.v1`); lo que vuelve se muestra
 * y se dibuja tal cual. La entrada se decide al cierre de la vela del cursor y entra en la apertura
 * de la siguiente: lo resuelve el motor. Sin stop o sin plan se puede mandar a propósito: el
 * proceso lo marca al calificar. Es simulación de estudio, no una orden del broker.
 */
import { useState } from 'react'

import { Segmented } from '@/components/ui/segmented'
import { decimal, parseDecimalInput } from '@/lib/format'
import { barLabel } from '@/lib/replay-api'
import type {
  MarkKind,
  MarkRequest,
  OrderRequest,
  ReplayBar,
  ReplayMark,
  ReplayOrder,
} from '@/lib/replay-contracts'

const KINDS: [MarkKind, string][] = [
  ['level', 'Nivel'],
  ['pivot_high', 'Pivote alto'],
  ['pivot_low', 'Pivote bajo'],
]
const kindLabel = (k: MarkKind) => KINDS.find(([id]) => id === k)![1]

const field =
  'min-h-9 w-full rounded-sm border border-border bg-surface px-2 font-mono text-[13px] aria-invalid:border-down'
const send =
  'min-h-9 justify-self-start rounded-sm bg-primary px-3 text-[12.5px] font-bold text-primary-foreground disabled:opacity-60'

/** El precio y la vela que propone tocar el gráfico (se pueden cambiar a mano). */
export type Picked = { price: string; bar: ReplayBar } | null

export function MarkForm({
  picked,
  busy,
  onMark,
}: {
  picked: Picked
  busy: boolean
  onMark: (req: MarkRequest) => void
}) {
  const [kind, setKind] = useState<MarkKind>('level')
  const [typed, setTyped] = useState<{ from: Picked; text: string } | null>(null)
  // tocar el gráfico propone el precio; en un pivote, el máximo o el mínimo de esa vela
  const proposed = !picked
    ? ''
    : kind === 'level'
      ? picked.price
      : kind === 'pivot_high'
        ? picked.bar.high
        : picked.bar.low
  const text = typed && typed.from === picked ? typed.text : proposed
  const price = parseDecimalInput(text)
  const pivot = kind !== 'level'
  const ready = price !== null && (!pivot || picked !== null)
  return (
    <form
      aria-label="Marcar en el gráfico"
      className="grid gap-2"
      onSubmit={(e) => {
        e.preventDefault()
        if (!ready || busy) return
        onMark({ kind, price, ...(pivot && { bar_index: picked!.bar.index }) })
      }}
    >
      <Segmented
        label="Qué marcar"
        options={KINDS.map(([, l]) => l)}
        value={kindLabel(kind)}
        onChange={(l) => setKind(KINDS.find(([, x]) => x === l)![0])}
      />
      <label className="grid gap-1 text-[12px]">
        <span className="font-semibold">Precio</span>
        <input
          inputMode="decimal"
          autoComplete="off"
          value={text}
          aria-invalid={text !== '' && price === null}
          onChange={(e) => setTyped({ from: picked, text: e.target.value })}
          className={field}
        />
      </label>
      <p className="text-[11.5px] text-muted-foreground">
        {pivot
          ? picked
            ? `Vela: ${barLabel(picked.bar)} UTC. Tocá otra para cambiarla.`
            : 'Tocá la vela del pivote en el gráfico.'
          : 'Tocá el gráfico a la altura del nivel o escribí el precio.'}
      </p>
      <button type="submit" disabled={!ready || busy} className={send}>
        Marcar
      </button>
    </form>
  )
}

export function StudyTicket({
  at,
  busy,
  onOrder,
}: {
  /** la vela de la decisión (la del cursor) */
  at: ReplayBar | undefined
  busy: boolean
  onOrder: (req: OrderRequest) => void
}) {
  const [side, setSide] = useState<'long' | 'short'>('long')
  const [stop, setStop] = useState('')
  const [target, setTarget] = useState('')
  const [risk, setRisk] = useState('')
  const [plan, setPlan] = useState('')
  const parsed = { stop: parseDecimalInput(stop), target: parseDecimalInput(target) }
  const riskPct = parseDecimalInput(risk)
  const bad = (t: string, v: string | null) => t.trim() !== '' && v === null
  const ready = riskPct !== null && !bad(stop, parsed.stop) && !bad(target, parsed.target)
  return (
    <form
      aria-label="Ticket de estudio"
      className="grid gap-2"
      onSubmit={(e) => {
        e.preventDefault()
        if (!ready || busy) return
        onOrder({
          side,
          risk_pct: riskPct,
          plan,
          ...(parsed.stop && { stop: parsed.stop }),
          ...(parsed.target && { target: parsed.target }),
        })
      }}
    >
      <Segmented
        label="Lado"
        options={['Largo', 'Corto']}
        value={side === 'long' ? 'Largo' : 'Corto'}
        onChange={(v) => setSide(v === 'Largo' ? 'long' : 'short')}
      />
      <div className="grid grid-cols-3 gap-2">
        {(
          [
            ['Stop', stop, setStop, bad(stop, parsed.stop)],
            ['Objetivo', target, setTarget, bad(target, parsed.target)],
            ['Riesgo %', risk, setRisk, bad(risk, riskPct)],
          ] as const
        ).map(([label, value, set, invalid]) => (
          <label key={label} className="grid gap-1 text-[12px]">
            <span className="font-semibold">{label}</span>
            <input
              inputMode="decimal"
              autoComplete="off"
              value={value}
              aria-invalid={invalid}
              onChange={(e) => set(e.target.value)}
              className={field}
            />
          </label>
        ))}
      </div>
      <label className="grid gap-1 text-[12px]">
        <span className="font-semibold">Plan escrito antes de entrar</span>
        <textarea
          value={plan}
          maxLength={2000}
          rows={3}
          onChange={(e) => setPlan(e.target.value)}
          className="rounded-sm border border-border bg-surface px-2 py-1.5 text-[13px]"
        />
      </label>
      <p className="text-[11.5px] text-muted-foreground">
        {at ? `Se decide al cierre de ${barLabel(at)} UTC` : 'Se decide al cierre de la vela'} y
        entra en la apertura de la siguiente (lo resuelve el motor). Simulación de estudio: no es
        una orden.
        {!parsed.stop && ' Sin stop: el proceso lo reprueba.'}
        {!plan.trim() && ' Sin plan escrito: el proceso lo reprueba.'}
      </p>
      <button type="submit" disabled={!ready || busy} className={send}>
        Mandar al motor
      </button>
    </form>
  )
}

/** Lo que el motor guardó: marcas y entradas, con la vela en que se decidieron. */
export function SessionLog({
  marks,
  orders,
  bars,
  digits,
}: {
  marks: ReplayMark[]
  orders: ReplayOrder[]
  bars: ReplayBar[]
  digits: number
}) {
  if (marks.length + orders.length === 0) return null
  const at = (i: number) => {
    const b = bars.find((x) => x.index === i)
    return b ? barLabel(b) : `vela ${i}`
  }
  const px = (p: string | null) => (p === null ? '—' : decimal(p, digits))
  return (
    <ul aria-label="Guardado por el motor" className="grid gap-1 text-[12px]">
      {marks.map((m) => (
        <li key={`m${m.mark_id}`} className="font-mono">
          M{m.mark_id} · {kindLabel(m.kind)} {px(m.price)}
          {m.bar_index !== null && ` en ${at(m.bar_index)}`} · marcado en {at(m.cursor)}
        </li>
      ))}
      {orders.map((o) => (
        <li key={`o${o.order_id}`} className="font-mono">
          E{o.order_id} · {o.side === 'long' ? 'largo' : 'corto'} · stop {px(o.stop)} · obj.{' '}
          {px(o.target)} · riesgo {decimal(o.risk_pct)} % · decidida en {at(o.cursor)}
        </li>
      ))}
    </ul>
  )
}
