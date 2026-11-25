/**
 * Posición y cierre de las capas flotantes (#570). El componente está en
 * `components/ui/floating.tsx`; acá queda la parte sin JSX (y que se prueba sola).
 */
import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react'

/** z-index de la capa: por encima de tarjetas, barras pegajosas (z-20/z-30) y la cabecera */
export const FLOATING_Z = 60
/** margen mínimo contra los bordes de la ventana */
const GUTTER = 8
/** separación entre el botón y el panel */
const GAP = 8

/** Marca de las capas flotantes: un clic dentro de cualquiera no cierra las demás. */
export const FLOATING_ATTR = 'data-floating-layer'

export type Place = { top: number; left: number; maxHeight: number }

/**
 * Calcula dónde va el panel: debajo del botón (o arriba si abajo no entra y arriba hay más lugar),
 * alineado a su borde izquierdo o derecho y siempre dentro de la ventana.
 */
export function placeFloating(
  anchor: DOMRect,
  size: { width: number; height: number },
  view: { width: number; height: number },
  align: 'start' | 'end',
): Place {
  const width = Math.min(size.width, view.width - GUTTER * 2)
  let left = align === 'end' ? anchor.right - width : anchor.left
  if (align === 'start' && left + width > view.width - GUTTER) left = anchor.right - width
  if (align === 'end' && left < GUTTER) left = anchor.left
  left = Math.max(GUTTER, Math.min(left, view.width - GUTTER - width))

  const below = view.height - anchor.bottom - GAP - GUTTER
  const above = anchor.top - GAP - GUTTER
  const up = size.height > below && above > below
  const maxHeight = Math.max(120, up ? above : below)
  const top = up
    ? Math.max(GUTTER, anchor.top - GAP - Math.min(size.height, maxHeight))
    : anchor.bottom + GAP
  return { top, left, maxHeight }
}

/**
 * Cierra con Esc o con un clic fuera del ancla y del panel. Con Esc, el foco vuelve a `focusBack`.
 */
export function useDismiss(
  open: boolean,
  onClose: () => void,
  refs: RefObject<HTMLElement | null>[],
  focusBack?: RefObject<HTMLElement | null>,
) {
  const latest = useRef({ onClose, refs, focusBack })
  useLayoutEffect(() => {
    latest.current = { onClose, refs, focusBack }
  })
  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node | null
      if (!t) return
      const { refs: rs, onClose: close } = latest.current
      if (rs.some((r) => r.current?.contains(t))) return
      if (t instanceof Element && t.closest(`[${FLOATING_ATTR}]`)) return
      close()
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      const { onClose: close, focusBack: back } = latest.current
      close()
      back?.current?.focus()
    }
    document.addEventListener('pointerdown', onDown)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('pointerdown', onDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])
}
