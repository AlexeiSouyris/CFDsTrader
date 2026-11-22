/**
 * Armazón de la app: barra superior, navegación (lateral en escritorio, inferior en el celular),
 * escenario de vidrio, búsqueda de vistas, avisos y ajuste del vidrio.
 *
 * Une dos componentes de referencia que hacían lo mismo (#164): "Glassy Dashboard" (barra,
 * docks y búsqueda) y "Adjustable Glass Dashboard" (panel para ajustar el vidrio). Queda un solo
 * armazón; el contenido de cada vista lo decide la app.
 */
import { Bell, Moon, Search, SlidersHorizontal, Sun, X, type LucideIcon } from 'lucide-react'
import { useEffect, useId, useRef, useState, type CSSProperties, type ReactNode } from 'react'

import { CandlestickIcon } from '@/components/ui/animated-icons'
import { PRESETS, type GlassSettings } from '@/components/ui/glass-settings'
import { cn } from '@/lib/utils'

export type ShellView = {
  id: string
  label: string
  /** etiqueta corta para el dock del celular */
  short: string
  description: string
  icon: LucideIcon
}

type Popover = 'notices' | 'glass' | null

export function GlassShell({
  brand = 'CFDsTrader',
  views,
  active,
  onSelect,
  theme,
  onThemeChange,
  glass,
  onGlassChange,
  notices,
  user,
  heading,
  children,
}: {
  brand?: string
  views: ShellView[]
  active: string
  onSelect: (id: string) => void
  theme: 'dark' | 'light'
  onThemeChange: (t: 'dark' | 'light') => void
  glass: GlassSettings
  onGlassChange: (g: GlassSettings) => void
  notices: { id: string; title: string; detail: string }[]
  user: { name: string; initials: string }
  /** encabezado propio para una página que no es una vista (por ejemplo, el 404) */
  heading?: { label: string; description: string }
  children: ReactNode
}) {
  const [popover, setPopover] = useState<Popover>(null)
  const [searching, setSearching] = useState(false)
  const [seen, setSeen] = useState(false)
  const current = heading ?? views.find((v) => v.id === active) ?? views[0]

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setSearching(true)
      }
      if (e.key === 'Escape') setPopover(null)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  return (
    <div
      className="relative mx-auto flex min-h-svh w-full max-w-[1400px] flex-col px-3 pt-3 pb-24 sm:px-6 sm:pt-6 md:pb-6"
      style={
        {
          '--glass-blur': `${glass.blur}px`,
          '--glass-opacity': glass.opacity,
        } as CSSProperties
      }
    >
      <header className="glass relative z-20 grid grid-cols-[1fr_auto] items-center gap-3 rounded-[22px] px-3 py-2 md:grid-cols-[1fr_minmax(220px,380px)_1fr] md:rounded-full">
        <a
          href="#panel"
          className="flex min-w-0 items-center gap-2.5"
          onClick={(e) => {
            e.preventDefault()
            onSelect(views[0].id)
          }}
        >
          <span className="grid size-9 place-items-center rounded-full bg-primary text-primary-foreground">
            <CandlestickIcon size={20} playOnMount duration={1.4} />
          </span>
          <span className="min-w-0">
            <strong className="block text-[12px] font-extrabold tracking-[0.18em] uppercase">
              {brand}
            </strong>
            <small className="block text-[10px] text-muted-foreground">Simulación · paper</small>
          </span>
        </a>

        <button
          type="button"
          onClick={() => setSearching(true)}
          className="col-span-2 row-start-2 flex min-h-10 items-center gap-2.5 rounded-full border border-border bg-[color-mix(in_srgb,var(--card)_40%,transparent)] px-3 text-left text-[12px] font-semibold text-muted-foreground hover:text-foreground md:col-span-1 md:row-start-auto"
        >
          <Search className="size-4" aria-hidden="true" />
          <span className="truncate">Buscar una vista o un instrumento</span>
          <kbd className="ml-auto hidden rounded border border-border px-1.5 text-[10px] sm:inline">
            Ctrl K
          </kbd>
        </button>

        <div className="relative flex items-center justify-end gap-1.5">
          <IconButton
            label="Ajustar el vidrio"
            expanded={popover === 'glass'}
            onClick={() => setPopover(popover === 'glass' ? null : 'glass')}
          >
            <SlidersHorizontal />
          </IconButton>
          <IconButton
            label={theme === 'dark' ? 'Tema claro' : 'Tema oscuro'}
            onClick={() => onThemeChange(theme === 'dark' ? 'light' : 'dark')}
          >
            {theme === 'dark' ? <Sun /> : <Moon />}
          </IconButton>
          <IconButton
            label="Avisos"
            expanded={popover === 'notices'}
            onClick={() => {
              setSeen(true)
              setPopover(popover === 'notices' ? null : 'notices')
            }}
          >
            <Bell />
            {!seen && notices.length > 0 && (
              <i className="absolute top-2 right-2 size-1.5 rounded-full bg-down" />
            )}
          </IconButton>
          <span
            title={user.name}
            className="hidden size-9 place-items-center rounded-full border border-border text-[11px] font-bold sm:grid"
          >
            {user.initials}
          </span>

          {popover === 'notices' && (
            <Popover title="Avisos" onClose={() => setPopover(null)}>
              {notices.length === 0 ? (
                <p className="text-[12px] text-muted-foreground">No hay avisos nuevos.</p>
              ) : (
                <ul className="grid gap-2">
                  {notices.map((n) => (
                    <li key={n.id} className="rounded-xl border border-border p-2.5">
                      <strong className="block text-[12px]">{n.title}</strong>
                      <span className="text-[11px] text-muted-foreground">{n.detail}</span>
                    </li>
                  ))}
                </ul>
              )}
            </Popover>
          )}
          {popover === 'glass' && (
            <Popover title="Vidrio" onClose={() => setPopover(null)}>
              <div className="mb-3 flex gap-1.5">
                {PRESETS.map((p) => (
                  <button
                    key={p.name}
                    type="button"
                    onClick={() => onGlassChange(p.value)}
                    className="flex-1 rounded-lg border border-border py-1.5 text-[11px] font-semibold hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]"
                  >
                    {p.name}
                  </button>
                ))}
              </div>
              <Range
                label="Desenfoque"
                value={glass.blur}
                min={0}
                max={36}
                step={1}
                text={`${glass.blur} px`}
                onChange={(blur) => onGlassChange({ ...glass, blur })}
              />
              <Range
                label="Opacidad"
                value={glass.opacity}
                min={0.08}
                max={0.92}
                step={0.01}
                text={`${Math.round(glass.opacity * 100)} %`}
                onChange={(opacity) => onGlassChange({ ...glass, opacity })}
              />
            </Popover>
          )}
        </div>
      </header>

      <div className="relative mt-3 flex flex-1 gap-3">
        <Dock views={views} active={active} onSelect={onSelect} side />
        <main className="glass min-w-0 flex-1 rounded-[28px] p-4 sm:p-7">
          <header className="mb-5 flex flex-wrap items-end justify-between gap-3">
            <div>
              <h1 className="text-[clamp(26px,3vw,42px)] leading-none font-bold tracking-[-0.05em]">
                {current.label}
              </h1>
              <p className="mt-2 text-[13px] text-muted-foreground">{current.description}</p>
            </div>
          </header>
          {children}
          <p
            role="note"
            className="mt-8 rounded-xl border border-border px-3 py-2 text-[11px] text-muted-foreground"
          >
            Prototipo con <strong className="text-foreground">datos de ejemplo</strong>: no está
            conectado al motor ni a IG y no calcula nada. Nada de lo que se ve acá es una señal real
            ni una promesa de ganancia.
          </p>
        </main>
      </div>
      <Dock views={views} active={active} onSelect={onSelect} />

      {searching && (
        <ViewSearch
          views={views}
          onClose={() => setSearching(false)}
          onSelect={(id) => {
            setSearching(false)
            onSelect(id)
          }}
        />
      )}
    </div>
  )
}

function IconButton({
  label,
  expanded,
  onClick,
  children,
}: {
  label: string
  expanded?: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-expanded={expanded}
      onClick={onClick}
      className="relative grid size-10 place-items-center rounded-full border border-border hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] [&_svg]:size-[17px]"
    >
      {children}
    </button>
  )
}

function Popover({
  title,
  onClose,
  children,
}: {
  title: string
  onClose: () => void
  children: ReactNode
}) {
  const id = useId()
  return (
    <section
      aria-labelledby={id}
      className="glass-strong absolute top-[calc(100%+10px)] right-0 z-30 w-[min(300px,calc(100vw-24px))] rounded-2xl p-4"
    >
      <header className="mb-3 flex items-center justify-between">
        <h2 id={id} className="text-[14px] font-bold">
          {title}
        </h2>
        <button type="button" aria-label="Cerrar" onClick={onClose} className="rounded-full p-1">
          <X className="size-4" />
        </button>
      </header>
      {children}
    </section>
  )
}

function Range({
  label,
  value,
  min,
  max,
  step,
  text,
  onChange,
}: {
  label: string
  value: number
  min: number
  max: number
  step: number
  text: string
  onChange: (v: number) => void
}) {
  return (
    <label className="mb-3 block text-[11px] font-semibold">
      <span className="flex justify-between">
        {label}
        <output className="text-muted-foreground">{text}</output>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="mt-2 w-full accent-[var(--primary)]"
      />
    </label>
  )
}

function Dock({
  views,
  active,
  onSelect,
  side = false,
}: {
  views: ShellView[]
  active: string
  onSelect: (id: string) => void
  side?: boolean
}) {
  return (
    <nav
      aria-label={side ? 'Accesos' : 'Vistas'}
      className={cn(
        'glass z-20 flex gap-1 rounded-full p-1.5',
        side
          ? 'sticky top-6 hidden h-fit flex-col self-start md:flex'
          : 'fixed inset-x-3 bottom-3 justify-between overflow-x-auto [scrollbar-width:none] md:hidden',
      )}
    >
      {views.map(({ id, label, short, icon: Icon }) => {
        const on = id === active
        return (
          <a
            key={id}
            href={`#${id}`}
            aria-current={on ? 'page' : undefined}
            aria-label={side ? label : undefined}
            title={side ? label : undefined}
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey || e.shiftKey) return
              e.preventDefault()
              onSelect(id)
            }}
            className={cn(
              'flex min-h-11 items-center justify-center gap-1 rounded-full text-[10px] font-bold transition-colors',
              side ? 'size-11' : 'min-w-[60px] flex-1 shrink-0 flex-col px-1 py-1',
              on
                ? 'bg-primary text-primary-foreground'
                : 'hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]',
            )}
          >
            <Icon className="size-[17px]" aria-hidden="true" />
            {!side && <span>{short}</span>}
          </a>
        )
      })}
    </nav>
  )
}

function ViewSearch({
  views,
  onClose,
  onSelect,
}: {
  views: ShellView[]
  onClose: () => void
  onSelect: (id: string) => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const [q, setQ] = useState('')
  useEffect(() => {
    const d = ref.current
    // jsdom (tests) no implementa showModal: ahí se abre sin modo modal
    if (d?.showModal) d.showModal()
    else d?.setAttribute('open', '')
  }, [])
  const words = q.toLowerCase().split(/\s+/).filter(Boolean)
  const found = views.filter((v) =>
    words.every((w) => `${v.label} ${v.description}`.toLowerCase().includes(w)),
  )
  return (
    <dialog
      ref={ref}
      aria-label="Buscar una vista"
      onClose={onClose}
      onClick={(e) => e.target === e.currentTarget && ref.current?.close()}
      className="glass-strong m-auto w-[min(480px,calc(100%-24px))] rounded-3xl p-4 text-foreground backdrop:bg-black/40 backdrop:backdrop-blur-sm"
    >
      <label className="flex items-center gap-2 rounded-xl border border-border px-3">
        <Search className="size-4 text-muted-foreground" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Buscar"
          placeholder="Escribí una vista: gráficos, noticias…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          className="min-h-11 flex-1 bg-transparent text-[14px] outline-none"
        />
      </label>
      <ul className="mt-3 grid gap-1.5">
        {found.map((v) => (
          <li key={v.id}>
            <button
              type="button"
              onClick={() => onSelect(v.id)}
              className="flex w-full items-center gap-3 rounded-xl border border-border p-2.5 text-left hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]"
            >
              <v.icon className="size-4 text-muted-foreground" aria-hidden="true" />
              <span>
                <strong className="block text-[13px]">{v.label}</strong>
                <small className="text-[11px] text-muted-foreground">{v.description}</small>
              </span>
            </button>
          </li>
        ))}
        {found.length === 0 && (
          <li className="p-2 text-[12px] text-muted-foreground">No hay vistas con ese nombre.</li>
        )}
      </ul>
    </dialog>
  )
}
