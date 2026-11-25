/**
 * Página 404. Inspirada en "Not Found 2" (21st.dev), reescrita sin dependencias de shadcn
 * (#164): el resto de la app no usa Button/Empty, así que no se suman solo por esto.
 * «Buscar una vista o un instrumento» abre el buscador del encabezado (#362).
 */
import { Compass, Home } from 'lucide-react'

import { openSearch } from '@/lib/terminal-focus'

export function NotFound({
  onHome,
  onExplore = openSearch,
}: {
  onHome: () => void
  onExplore?: () => void
}) {
  return (
    <section className="grid min-h-[60svh] place-items-center text-center">
      <div>
        <h2 className="bg-gradient-to-b from-foreground to-transparent bg-clip-text text-[clamp(96px,18vw,180px)] leading-none font-extrabold tracking-[-0.06em] text-transparent">
          404
        </h2>
        <p className="mt-2 text-[14px] text-muted-foreground">
          La página que buscás no existe o se movió.
        </p>
        <div className="mt-6 flex flex-wrap justify-center gap-2">
          <button
            type="button"
            onClick={onHome}
            className="inline-flex items-center gap-2 rounded-full whitespace-nowrap bg-primary px-4 py-2.5 text-[13px] font-bold text-primary-foreground"
          >
            <Home className="size-4" aria-hidden="true" />
            Ir al inicio
          </button>
          <button
            type="button"
            onClick={onExplore}
            className="inline-flex items-center gap-2 rounded-full whitespace-nowrap border border-border px-4 py-2.5 text-[13px] font-bold"
          >
            <Compass className="size-4" aria-hidden="true" />
            Buscar una vista o un instrumento
          </button>
        </div>
      </div>
    </section>
  )
}
