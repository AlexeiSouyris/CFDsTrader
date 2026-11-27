/**
 * "Todas las vistas" (#618): cada sección del mapa con las vistas que viven adentro (Panel en
 * Inicio, Multigráfico en Gráficos, Performance, Propuestas, Liquidez y Glosario en Investigación).
 * Lo abre "Más" en la barra de abajo del celular y en la barra lateral: así cualquier vista queda a
 * dos toques. Es un diálogo modal (`showModal`): Esc o un toque afuera lo cierran y el foco vuelve
 * al botón. No sube el teclado, a diferencia de la paleta (que se abre desde acá con "Buscar").
 */
import { Search, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import type { ShellView } from '@/components/ui/app-shell'
import { cn } from '@/lib/utils'

export type MenuGroup = { view: ShellView; children: ShellView[] }

export function ViewsMenu({
  groups,
  active,
  section,
  onSelect,
  onSearch,
  onClose,
}: {
  groups: MenuGroup[]
  active: string
  section: string
  onSelect: (id: string) => void
  /** cierra el menú y abre la paleta de comandos */
  onSearch: () => void
  onClose: () => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  // el foco vuelve a "Más" al cerrar; se toma al renderizar, antes de que se mueva
  const [back] = useState(() =>
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  )
  useEffect(() => {
    const d = ref.current
    // jsdom (tests) no implementa showModal: ahí se abre sin modo modal
    if (d?.showModal && !d.open) d.showModal()
    else d?.setAttribute('open', '')
    return () => back?.focus?.()
  }, [back])

  const link = (v: ShellView, sub: boolean) => {
    const on = v.id === active || (!sub && v.id === section)
    return (
      <a
        href={`#${v.id}`}
        aria-current={v.id === active ? 'page' : on ? 'true' : undefined}
        onClick={(e) => {
          if (e.metaKey || e.ctrlKey || e.shiftKey) return
          e.preventDefault()
          onSelect(v.id)
        }}
        className={cn(
          'flex min-h-11 items-center gap-2.5 rounded-xl px-2.5 transition-colors',
          sub ? 'ml-7 text-[13px] text-muted-foreground' : 'text-[14px] font-bold',
          on
            ? 'bg-primary text-primary-foreground'
            : 'hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:text-foreground',
        )}
      >
        {!sub && <v.icon className="size-[17px] shrink-0" aria-hidden="true" />}
        {v.label}
      </a>
    )
  }

  return (
    <dialog
      ref={ref}
      aria-labelledby="views-menu-title"
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault()
        onClose()
      }}
      onClick={(e) => {
        // un toque en el fondo (fuera del rectángulo del diálogo) lo cierra
        if (e.target !== e.currentTarget) return
        const r = e.currentTarget.getBoundingClientRect()
        if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom)
          onClose()
      }}
      className="panel-raised mx-auto mt-auto mb-[max(0.75rem,env(safe-area-inset-bottom))] max-h-[min(85svh,640px)] w-[min(420px,calc(100%-24px))] overflow-y-auto rounded-sm p-3 text-foreground backdrop:bg-black/60 md:my-auto"
    >
      <header className="mb-3 flex items-center justify-between gap-2">
        <h2 id="views-menu-title" className="text-[16px] font-bold">
          Todas las vistas
        </h2>
        <button
          type="button"
          aria-label="Cerrar"
          onClick={onClose}
          className="grid size-11 place-items-center rounded-sm border border-border hover:bg-surface"
        >
          <X className="size-4" aria-hidden="true" />
        </button>
      </header>
      <nav aria-label="Todas las vistas">
        <ul className="grid gap-1">
          {groups.map((g) => (
            <li key={g.view.id}>
              {link(g.view, false)}
              {g.children.length > 0 && (
                <ul className="mt-0.5 grid gap-0.5">
                  {g.children.map((c) => (
                    <li key={c.id}>{link(c, true)}</li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      </nav>
      <button
        type="button"
        onClick={onSearch}
        className="mt-3 flex min-h-11 w-full items-center gap-2.5 rounded-xl border border-border px-3 text-[13px] font-semibold text-muted-foreground hover:text-foreground"
      >
        <Search className="size-4" aria-hidden="true" />
        Buscar un instrumento o un comando
      </button>
    </dialog>
  )
}
