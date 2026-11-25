/**
 * Caja con desplazamiento horizontal para tablas anchas (#579). Una sola para toda la app:
 *
 * - Es una región con nombre (`role="region"` y `aria-label`) y, si de verdad se desplaza, entra
 *   en el orden del Tab: con el foco adentro, las flechas la mueven (WCAG 2.1.1, axe
 *   `scrollable-region-focusable`).
 * - Cuando hay más contenido a un costado, ese borde se desvanece y, en el celular, una línea
 *   avisa que se puede deslizar: sin eso, a 375 px las columnas de la derecha quedaban escondidas
 *   sin ninguna señal.
 */
import { useEffect, useRef, useState, type ReactNode } from 'react'

import { cn } from '@/lib/utils'

const FADE = '28px'

export function ScrollRegion({
  label,
  className,
  children,
}: {
  label: string
  className?: string
  children: ReactNode
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [edges, setEdges] = useState({ left: false, right: false })

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const update = () => {
      const left = el.scrollLeft > 1
      const right = el.scrollLeft + el.clientWidth < el.scrollWidth - 1
      setEdges((prev) => (prev.left === left && prev.right === right ? prev : { left, right }))
    }
    update()
    el.addEventListener('scroll', update, { passive: true })
    const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(update)
    ro?.observe(el)
    if (el.firstElementChild) ro?.observe(el.firstElementChild)
    return () => {
      el.removeEventListener('scroll', update)
      ro?.disconnect()
    }
  }, [])

  const scrolls = edges.left || edges.right
  const mask = scrolls
    ? `linear-gradient(to right, ${edges.left ? 'transparent' : '#000'}, #000 ${FADE}, #000 calc(100% - ${FADE}), ${edges.right ? 'transparent' : '#000'})`
    : undefined

  return (
    <>
      <div
        ref={ref}
        role="region"
        // "(tabla)": no repite el nombre de la sección que la contiene (axe landmark-unique)
        aria-label={`${label} (tabla)`}
        tabIndex={scrolls ? 0 : undefined}
        className={cn(
          'overflow-x-auto rounded-md focus-visible:outline-2 focus-visible:outline-ring',
          className,
        )}
        style={{ maskImage: mask, WebkitMaskImage: mask }}
      >
        {children}
      </div>
      {edges.right && (
        <p
          aria-hidden="true"
          className="mt-1 text-right text-[10px] text-muted-foreground md:hidden"
        >
          Deslizá para ver más →
        </p>
      )}
    </>
  )
}
