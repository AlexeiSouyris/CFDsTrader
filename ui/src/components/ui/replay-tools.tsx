/**
 * Marcas del replay (#773): un nivel o un pivote va al motor con el cursor que pone él
 * (`replay_mark.v1`); lo que vuelve se lista y se dibuja tal cual.
 */
import { useState } from 'react'

import { Segmented } from '@/components/ui/segmented'
import { decimal, parseDecimalInput } from '@/lib/format'
import { barLabel } from '@/lib/replay-api'
import type { MarkKind, MarkRequest, ReplayBar, ReplayMark } from '@/lib/replay-contracts'

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

/** Lo que el motor guardó: las marcas, con la vela en que se hicieron. */
export function SessionLog({
  marks,
  bars,
  digits,
}: {
  marks: ReplayMark[]
  bars: ReplayBar[]
  digits: number
}) {
  if (marks.length === 0) return null
  const at = (i: number) => {
    const b = bars.find((x) => x.index === i)
    return b ? barLabel(b) : `vela ${i}`
  }
  const px = (p: string) => decimal(p, digits)
  return (
    <ul aria-label="Guardado por el motor" className="grid gap-1 text-[12px]">
      {marks.map((m) => (
        <li key={`m${m.mark_id}`} className="font-mono">
          M{m.mark_id} · {kindLabel(m.kind)} {px(m.price)}
          {m.bar_index !== null && ` en ${at(m.bar_index)}`} · marcado en {at(m.cursor)}
        </li>
      ))}
    </ul>
  )
}
