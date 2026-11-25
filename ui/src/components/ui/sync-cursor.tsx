/**
 * La línea del cursor sincronizado (#544, ver `lib/cursor-sync.ts`): en la vela que contiene la
 * hora del cursor de otro gráfico. Es lo único que se vuelve a dibujar al moverse ese cursor.
 */
import { useSyncExternalStore } from 'react'

import { barAt, type CursorSync } from '@/lib/cursor-sync'

export function SyncCursor({
  hub,
  self,
  starts,
  ends,
  x,
  top,
  bottom,
}: {
  hub: CursorSync
  /** este gráfico: su propio cursor no se marca */
  self: string
  starts: readonly number[]
  ends: readonly number[]
  /** x del centro de la vela `i`, con la escala con que se dibujó la capa */
  x: (i: number) => number
  top: number
  bottom: number
}) {
  const at = useSyncExternalStore(hub.subscribe, hub.get, hub.get)
  if (!at || at.from === self) return null
  const i = barAt(starts, ends, at.t)
  if (i === null) return null
  const cx = x(i)
  return (
    <line
      data-sync-cursor={new Date(starts[i]).toISOString()}
      x1={cx}
      x2={cx}
      y1={top}
      y2={bottom}
      stroke="var(--foreground)"
      strokeOpacity={0.65}
      strokeDasharray="3 3"
      vectorEffect="non-scaling-stroke"
      pointerEvents="none"
    />
  )
}
