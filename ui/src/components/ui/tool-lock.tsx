/**
 * Candado de una herramienta (#771, `docs/design/2026-11-academia.md` §4.1): mientras su lección no
 * esté aprobada (lo dice el motor), la herramienta se ve gris, no se puede usar y dice qué lección
 * la abre, con un enlace a esa lección. También se abre a mano en Ajustes. Con
 * `locked_tools_visible = false` en el catálogo, se esconde. Abierta, se muestra tal cual.
 */
import { Lock } from 'lucide-react'
import type { ReactNode } from 'react'

import { focusLesson, useToolLock } from '@/lib/academy-locks'
import { cn } from '@/lib/utils'

export function ToolLock({
  tool,
  compact = false,
  className,
  children,
}: {
  /** id estable de la herramienta en el catálogo (`tool.watchlist`, ...) */
  tool: string
  /** una línea en lugar de la herramienta (para controles chicos, como un selector) */
  compact?: boolean
  className?: string
  children: ReactNode
}) {
  const lock = useToolLock(tool)
  if (!lock.locked) return <>{children}</>
  if (!lock.visible) return null
  const lesson = `lección ${lock.lessonId}${lock.lessonTitle ? ` · ${lock.lessonTitle}` : ''}`
  const link = (
    <a
      href="#academia"
      onClick={() => focusLesson(lock.lessonId)}
      className="font-semibold text-foreground underline underline-offset-2"
    >
      Ir a la lección
    </a>
  )
  if (compact) {
    return (
      <p
        data-tool-lock={tool}
        className={cn('flex flex-wrap items-center gap-x-1.5 text-[12px]', className)}
      >
        <Lock aria-hidden="true" className="size-3.5 shrink-0" />
        <span className="text-muted-foreground">
          {lock.name}: se desbloquea en la {lesson}.
        </span>
        {link}
      </p>
    )
  }
  return (
    <div data-tool-lock={tool} className={cn('relative min-h-36 min-w-0', className)}>
      {/* la herramienta se ve, gris y sin poder usarse (fuera del orden de foco y del lector) */}
      <div inert aria-hidden="true" className="pointer-events-none opacity-30 select-none">
        {children}
      </div>
      <div className="absolute inset-0 flex justify-center p-2">
        <div
          role="group"
          aria-label={`${lock.name}, bloqueada`}
          className="panel-raised sticky top-20 grid h-fit max-w-[36ch] gap-1 rounded-section px-3 py-2.5 text-[12px]"
        >
          <p className="flex items-center gap-1.5 font-bold">
            <Lock aria-hidden="true" className="size-3.5" />
            {lock.name}
          </p>
          <p className="text-muted-foreground">
            Se desbloquea en la {lesson}. También se abre a mano en Ajustes.
          </p>
          {link}
        </div>
      </div>
    </div>
  )
}
