/**
 * Capa flotante para los popovers de la UI (#570): menú "Herramientas", definiciones del glosario
 * (`Term`) y cualquier panel que se abra desde un botón dentro de una tarjeta.
 *
 * Por qué existe: una tarjeta con `transform`, `backdrop-filter` o `isolation` (como lo eran las
 * de vidrio antes de #708) crea su propio contexto de apilamiento. Un panel `absolute` con `z-40` dentro
 * de una tarjeta solo compite dentro de esa tarjeta: la tarjeta siguiente (por ejemplo, el gráfico
 * de abajo) se pinta encima. Por eso el panel se dibuja en un portal a `document.body`, con
 * posición fija calculada desde el botón y un z-index por encima de todas las tarjetas.
 *
 * Se cierra con Esc (y devuelve el foco al botón) o con un clic afuera. Un clic dentro de otra capa
 * flotante (por ejemplo, la definición de un término abierta desde el menú) no cierra la de abajo.
 */
import {
  useLayoutEffect,
  useState,
  type CSSProperties,
  type ReactNode,
  type RefObject,
} from 'react'
import { createPortal } from 'react-dom'

import { FLOATING_ATTR, FLOATING_Z, placeFloating, type Place } from '@/lib/floating'

/**
 * El panel flotante: se dibuja en `document.body`, con posición fija junto a `anchor`. Sigue al
 * botón si la página o un contenedor se desplazan, o si cambia el tamaño de la ventana.
 */
export function Floating({
  anchor,
  panelRef,
  align = 'start',
  as: Tag = 'div',
  className,
  style,
  children,
  ...rest
}: {
  anchor: RefObject<HTMLElement | null>
  panelRef: RefObject<HTMLElement | null>
  align?: 'start' | 'end'
  as?: 'div' | 'span'
  className?: string
  style?: CSSProperties
  children: ReactNode
  id?: string
  role?: string
  'aria-label'?: string
}) {
  const [place, setPlace] = useState<Place | null>(null)

  useLayoutEffect(() => {
    const update = () => {
      const a = anchor.current
      const p = panelRef.current
      if (!a || !p) return
      const next = placeFloating(
        a.getBoundingClientRect(),
        { width: p.offsetWidth, height: p.scrollHeight },
        { width: window.innerWidth, height: window.innerHeight },
        align,
      )
      setPlace((prev) =>
        prev &&
        prev.top === next.top &&
        prev.left === next.left &&
        prev.maxHeight === next.maxHeight
          ? prev
          : next,
      )
    }
    update()
    window.addEventListener('resize', update)
    window.addEventListener('scroll', update, true)
    return () => {
      window.removeEventListener('resize', update)
      window.removeEventListener('scroll', update, true)
    }
  }, [anchor, panelRef, align])

  return createPortal(
    <Tag
      {...{ [FLOATING_ATTR]: '' }}
      {...rest}
      ref={panelRef as RefObject<HTMLDivElement & HTMLSpanElement>}
      className={className}
      style={{
        position: 'fixed',
        zIndex: FLOATING_Z,
        top: place?.top ?? 0,
        left: place?.left ?? 0,
        maxHeight: place?.maxHeight,
        overflowY: 'auto',
        // hasta medir, invisible: así no parpadea en la esquina
        visibility: place ? undefined : 'hidden',
        ...style,
      }}
    >
      {children}
    </Tag>,
    document.body,
  )
}
