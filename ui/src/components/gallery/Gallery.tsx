import type { ComponentType } from 'react'

/** Un componente de la galería: qué es, de dónde salió y cómo se ve con datos de ejemplo. */
export type GalleryEntry = {
  id: string
  title: string
  /** Para qué lo usaríamos en la app (spec §1.1 y fase 10). */
  purpose: string
  /** Link al componente original (por ejemplo en 21st.dev). */
  source?: string
  Demo: ComponentType
}

export function Gallery({ entries }: { entries: GalleryEntry[] }) {
  if (entries.length === 0) {
    return (
      <p className="rounded-xl border border-dashed border-zinc-700 p-8 text-center text-zinc-400">
        Todavía no hay componentes. Cada uno que se elija aparece acá.
      </p>
    )
  }
  return (
    <div className="grid gap-6">
      {entries.map(({ id, title, purpose, source, Demo }) => (
        <section
          key={id}
          aria-labelledby={`${id}-title`}
          className="rounded-xl border border-zinc-800 p-4"
        >
          <header className="mb-4 flex flex-wrap items-baseline justify-between gap-2">
            <div>
              <h2 id={`${id}-title`} className="text-lg font-semibold text-zinc-100">
                {title}
              </h2>
              <p className="text-sm text-zinc-400">{purpose}</p>
            </div>
            {source && (
              <a
                href={source}
                target="_blank"
                rel="noreferrer"
                className="text-sm text-sky-400 underline"
              >
                Ver original
              </a>
            )}
          </header>
          <Demo />
        </section>
      ))}
    </div>
  )
}
