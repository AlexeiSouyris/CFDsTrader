/**
 * Capas de React de los gráficos navegables (#401, ver `chart-nav.ts`): lo que sigue al encuadre
 * cuadro a cuadro (ejes) y los botones sobre el gráfico. Solo presentación (CLAUDE.md §1).
 */
import { useSyncExternalStore, type ReactNode } from 'react'
import { ArrowRightToLine, Ruler } from 'lucide-react'

import { useMapping, type HistoryState, type Viewport } from '@/components/ui/chart-nav'
import { atLatest, visibleRange, type Mapping } from '@/lib/chart-viewport'
import { cn } from '@/lib/utils'

/** Capa chica que sigue al encuadre vivo (ejes, rótulos): recibe la escala vigente. */
export function ViewportLive({
  vp,
  children,
}: {
  vp: Viewport
  children: (m: Mapping) => ReactNode
}) {
  const m = useMapping(vp)
  return <>{children(m)}</>
}

/**
 * Botones sobre el gráfico: la regla (medir arrastrando; en el celular es la forma de medir),
 * volver a la escala automática (si está en manual) e ir a la última vela (si no se ve).
 * Se dibujan solo cuando cambia lo que muestran, no en cada cuadro.
 */
export function ChartNavBar({
  vp,
  ruler,
  onRuler,
  right,
  bottom,
  history = 'idle',
  onRetry,
}: {
  vp: Viewport
  ruler: boolean
  onRuler: (on: boolean) => void
  /** distancia al borde derecho (el ancho del eje de precio) */
  right: number
  /** borde inferior de la fila de botones, desde arriba del gráfico (el pie del panel de precio) */
  bottom: number
  history?: HistoryState
  onRetry?: () => void
}) {
  const latest = useSyncExternalStore(
    vp.store.subscribe,
    () => atLatest(vp.store.get(), vp.store.count()),
    () => true,
  )
  const manual = useSyncExternalStore(
    vp.store.subscribe,
    () => vp.store.get().manual !== null,
    () => false,
  )
  const atFirst = useSyncExternalStore(
    vp.store.subscribe,
    () => visibleRange(vp.store.get(), vp.store.count())[0] === 0,
    () => false,
  )
  const pill =
    'pointer-events-auto inline-flex h-7 items-center gap-1.5 rounded-full border border-border bg-background/85 px-2.5 text-[11px] font-bold text-muted-foreground shadow-sm backdrop-blur transition-colors hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring'
  return (
    <>
      {(history === 'loading' || history === 'error' || (history === 'done' && atFirst)) && (
        <div
          className="pointer-events-none absolute left-2 top-2 flex"
          role="status"
          data-history={history}
        >
          {history === 'error' ? (
            <button type="button" className={pill} onClick={onRetry}>
              No se pudieron traer velas anteriores · reintentar
            </button>
          ) : (
            <span className={cn(pill, 'pointer-events-none')}>
              {history === 'loading'
                ? 'Trayendo velas anteriores…'
                : 'No hay velas más antiguas guardadas'}
            </span>
          )}
        </div>
      )}
      <div
        className="pointer-events-none absolute flex gap-1.5"
        style={{ right: right + 8, top: bottom - 6, transform: 'translateY(-100%)' }}
      >
        {manual && (
          <button
            type="button"
            className={pill}
            onClick={() => vp.store.set({ manual: null, user: true })}
            title="La escala de precio vuelve a ajustarse a lo visible (también con doble clic en el eje)"
          >
            Escala auto
          </button>
        )}
        <button
          type="button"
          aria-pressed={ruler}
          aria-label="Regla: medir arrastrando"
          title="Medir: activá la regla o usá Shift + arrastrar"
          onClick={() => onRuler(!ruler)}
          className={cn(
            pill,
            ruler &&
              'border-primary bg-primary text-primary-foreground hover:text-primary-foreground',
          )}
        >
          <Ruler className="size-3.5" aria-hidden />
        </button>
        {!latest && (
          <button
            type="button"
            className={pill}
            onClick={vp.goLatest}
            aria-label="Ir a la última vela"
            title="Ir a la última vela (tecla Fin)"
          >
            <ArrowRightToLine className="size-3.5" aria-hidden />
            <span className="hidden sm:inline">Última vela</span>
          </button>
        )}
      </div>
    </>
  )
}
