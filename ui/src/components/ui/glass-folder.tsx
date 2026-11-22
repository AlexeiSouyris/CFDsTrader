/**
 * Carpeta de noticias de un miembro: un panel trasero, las hojas adentro y un frente de vidrio
 * esmerilado que se inclina al pasar el puntero y se abre con un clic (las hojas se abren en
 * abanico).
 *
 * Inspirado en "Glass Folder Cards" (21st.dev), reescrito (#164): el arte del panel es un
 * degradé de CSS en grafito y plata (sin el pintor de ruido del original), y la carpeta sabe
 * de quién es. Todos ven todas las carpetas; solo el dueño agrega en la suya. **Ese permiso lo
 * tiene que imponer el servidor**: la UI solo lo refleja.
 */
import { Lock, Plus } from 'lucide-react'
import { useId, useState } from 'react'

import { cn } from '@/lib/utils'
import type { FolderFile, MemberFolder } from '@/mocks'

const SOURCE_ART: Record<FolderFile['source'], string> = {
  fed: 'bg-[linear-gradient(135deg,#3b4a63,#1b2230_60%,#8fa3c4)]',
  ecb: 'bg-[linear-gradient(135deg,#4a3f2a,#1f1b14_60%,#c9b27a)]',
  nota: 'bg-[linear-gradient(180deg,#f4f5f7,#dfe2e6)]',
}

export function GlassFolder({
  folder,
  editable,
  onAdd,
}: {
  folder: MemberFolder
  /** el miembro que mira es el dueño de la carpeta */
  editable: boolean
  onAdd?: (file: FolderFile) => void
}) {
  const [open, setOpen] = useState(false)
  const [files, setFiles] = useState(folder.files)
  const listId = useId()
  const visible = files.slice(-4)

  const add = () => {
    const file: FolderFile = {
      name: `Nota ${files.length + 1}`,
      meta: 'Nota · recién',
      source: 'nota',
    }
    setFiles((f) => [...f, file])
    onAdd?.(file)
  }

  return (
    <div
      className="group relative w-[min(320px,86vw)] pt-14 [perspective:1200px]"
      data-open={open || undefined}
    >
      {/* panel trasero */}
      <div className="absolute inset-x-0 top-14 bottom-0 overflow-hidden rounded-[22px] border border-[var(--glass-rim)] bg-[linear-gradient(160deg,#5b6068_0%,#2a2d32_45%,#15171a_100%)] [clip-path:path('M0_56A22_22_0_0_1_22_34L130_34C154_34_156_0_182_0L298_0A22_22_0_0_1_320_22L320_250A22_22_0_0_1_298_272L22_272A22_22_0_0_1_0_250Z')]">
        <div className="absolute inset-0 bg-[radial-gradient(120%_70%_at_80%_0%,rgba(255,255,255,0.28),transparent_60%)]" />
      </div>

      {/* hojas */}
      <ul
        id={listId}
        aria-label={`Noticias de ${folder.owner.name}`}
        className="relative z-10 mx-auto h-[272px] w-full"
      >
        {visible.map((file, i) => {
          const slot = visible.length === 1 ? 0 : (i / (visible.length - 1)) * 2 - 1
          const depth = visible.length - 1 - i
          return (
            <li
              key={`${file.name}-${i}`}
              className="absolute inset-x-6 top-16 h-40 transition-transform duration-700 ease-[cubic-bezier(.34,1.36,.5,1)]"
              style={{
                transform: open
                  ? `translate(${slot * 34}px, ${-84 - depth * 22}px) rotate(${slot * 5}deg)`
                  : `translate(0, ${-depth * 12}px)`,
                zIndex: i + 1,
              }}
            >
              <div
                tabIndex={open ? 0 : -1}
                className={cn(
                  'relative h-full overflow-hidden rounded-2xl shadow-[0_10px_26px_-8px_rgba(0,0,0,0.5)] group-hover:-translate-y-2',
                  SOURCE_ART[file.source],
                )}
              >
                <div
                  className={cn(
                    'flex items-baseline justify-between gap-2 px-3.5 pt-3 text-[12px] font-bold',
                    file.source === 'nota' ? 'text-zinc-900' : 'text-white',
                  )}
                >
                  <span className="truncate">{file.name}</span>
                  <span className="shrink-0 text-[10px] opacity-70">{file.meta}</span>
                </div>
              </div>
            </li>
          )
        })}
      </ul>

      {/* frente de vidrio */}
      <button
        type="button"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen((o) => !o)}
        className={cn(
          'glass absolute inset-x-0 bottom-0 z-20 h-[198px] origin-bottom rounded-[22px] text-left transition-transform duration-700 ease-[cubic-bezier(.34,1.36,.5,1)]',
          open ? '[transform:rotateX(-24deg)]' : 'group-hover:[transform:rotateX(-10deg)]',
        )}
      >
        <span className="absolute bottom-7 left-7">
          <span className="block text-[18px] font-bold tracking-tight">
            Carpeta de {folder.owner.name}
          </span>
          <span className="mt-1 flex items-center gap-1.5 text-[12px] text-muted-foreground">
            {files.length} {files.length === 1 ? 'noticia' : 'noticias'}
            {!editable && (
              <>
                {' · '}
                <Lock className="size-3" aria-hidden="true" />
                solo lectura
              </>
            )}
          </span>
        </span>
      </button>

      {editable && (
        <button
          type="button"
          onClick={add}
          aria-label={`Agregar una nota a la carpeta de ${folder.owner.name}`}
          className="absolute right-5 bottom-5 z-30 grid size-12 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-lg transition-transform hover:-translate-y-0.5 active:scale-95"
        >
          <Plus className="size-6" strokeWidth={1.8} />
        </button>
      )}
    </div>
  )
}
