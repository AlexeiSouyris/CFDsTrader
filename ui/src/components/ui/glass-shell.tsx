/**
 * Armazón de la app: barra superior con la línea de comandos, navegación (lateral en escritorio,
 * inferior en el celular), escenario, barra de estado, paleta de comandos (⌘K o `/`), atajos de
 * función (F1–F9, `?` los muestra), avisos y ajustes.
 *
 * Nació de dos componentes de vidrio (#164). Desde #708 es una terminal profesional: superficies
 * lisas, reglas de 1 px y teclado primero. El contenido de cada vista lo decide la app.
 */
import {
  Bell,
  Clock3,
  Ellipsis,
  House,
  Keyboard,
  Layers,
  LineChart,
  Moon,
  Play,
  SlidersHorizontal,
  Sun,
  Terminal,
  X,
  type LucideIcon,
} from 'lucide-react'
import {
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type RefObject,
} from 'react'

import { CandlestickIcon } from '@/components/ui/animated-icons'
import { StatusBar } from '@/components/ui/status-bar'
import { TOOLS } from '@/lib/analysis-layers'
import { useInstruments } from '@/lib/liquidity-api'
import type { InstrumentInfo } from '@/lib/liquidity-contracts'
import { useDismiss } from '@/lib/floating'
import { focusInstrument, focusTimeframe, OPEN_SEARCH_EVENT } from '@/lib/terminal-focus'
import { useChartTools } from '@/lib/use-chart-tools'
import { cn } from '@/lib/utils'
import { ViewsMenu, type MenuGroup } from '@/components/ui/views-menu'

export type ShellView = {
  id: string
  label: string
  /** etiqueta corta para el dock del celular */
  short: string
  description: string
  icon: LucideIcon
}

type Popover = 'notices' | 'settings' | 'keys' | null

export function GlassShell({
  brand = 'CFDsTrader',
  views,
  nav = views,
  bar = nav,
  menu,
  home = views[0].id,
  tabs = [],
  active,
  section = active,
  onSelect,
  onReplayIntro,
  theme,
  onThemeChange,
  fkeys = {},
  notices,
  user,
  heading,
  children,
}: {
  brand?: string
  views: ShellView[]
  /** las secciones de la barra lateral y la del celular (las demás vistas, en la paleta) */
  nav?: ShellView[]
  /** las de la barra de abajo del celular (#618): pocas, para que entren sin desplazarse */
  bar?: ShellView[]
  /** "Todas las vistas" (#618): cada sección con las vistas de adentro; lo abre "Más" */
  menu?: MenuGroup[]
  /** la sección de la vista abierta: la que se marca en la navegación */
  section?: string
  /** la vista de inicio: a la que lleva el logo */
  home?: string
  /** pestañas principales (#286): van arriba de cada vista, con su hash */
  tabs?: ShellView[]
  active: string
  onSelect: (id: string) => void
  /** vuelve a mostrar la intro "El mercado espera" (#581) */
  onReplayIntro?: () => void
  theme: 'dark' | 'light'
  onThemeChange: (t: 'dark' | 'light') => void
  /** atajo de función de cada sección (#708): `{ inicio: 'F1', ... }` */
  fkeys?: Readonly<Record<string, string>>
  notices: { id: string; title: string; detail: string }[]
  user: { name: string; initials: string }
  /** encabezado propio para una página que no es una vista (por ejemplo, el 404) */
  heading?: { label: string; description: string }
  children: ReactNode
}) {
  const [popover, setPopover] = useState<Popover>(null)
  const [searching, setSearching] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  // sin menú, "Más" abre la paleta (que también lista las vistas)
  const openMore = () => (menu ? setMenuOpen(true) : setSearching(true))
  const [seen, setSeen] = useState(false)
  const current = heading ?? views.find((v) => v.id === active) ?? views[0]
  const homeLabel = views.find((v) => v.id === home)?.label ?? home
  const popButtons = useRef<HTMLDivElement>(null)
  const popPanel = useRef<HTMLElement>(null)
  // los popovers del encabezado se cierran también con un toque afuera (#579)
  useDismiss(popover !== null, () => setPopover(null), [popButtons, popPanel])

  // el último `onSelect` y los atajos, sin volver a registrar el oyente en cada render
  const keys = useRef({ onSelect, fkeys })
  useEffect(() => {
    keys.current = { onSelect, fkeys }
  })

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setSearching(true)
        return
      }
      if (e.key === 'Escape') setPopover(null)
      if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return
      // F1–F9 (#708): a la sección, también mientras se escribe (las teclas de función no escriben)
      const target = Object.entries(keys.current.fkeys).find(([, k]) => k === e.key)?.[0]
      if (target && !e.shiftKey) {
        e.preventDefault()
        keys.current.onSelect(target)
        return
      }
      if (typing(e.target) || document.querySelector('dialog[open]')) return
      // `/` abre la línea de comandos y `?` la lista de atajos, como en una terminal
      if (e.key === '/') {
        e.preventDefault()
        setSearching(true)
      } else if (e.key === '?') {
        e.preventDefault()
        setPopover('keys')
      }
    }
    const open = () => setSearching(true)
    window.addEventListener('keydown', onKey)
    window.addEventListener(OPEN_SEARCH_EVENT, open)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener(OPEN_SEARCH_EVENT, open)
    }
  }, [])

  return (
    <div className="relative flex min-h-svh w-full flex-col pb-[calc(4.25rem+env(safe-area-inset-bottom))] md:pb-0">
      <a
        href="#contenido"
        className="sr-only z-50 bg-primary px-4 py-2 text-[13px] font-bold text-primary-foreground focus:not-sr-only focus:fixed focus:top-2 focus:left-2"
      >
        Saltar al contenido
      </a>
      <header className="sticky top-0 z-20 grid grid-cols-[1fr_auto] items-center gap-x-3 gap-y-1.5 border-b border-rule bg-card px-3 pt-[max(0.375rem,env(safe-area-inset-top))] pb-1.5 md:grid-cols-[auto_minmax(240px,560px)_1fr]">
        <a
          href={`#${home}`}
          className="flex min-w-0 items-center gap-2"
          onClick={(e) => {
            if (e.metaKey || e.ctrlKey || e.shiftKey) return
            e.preventDefault()
            onSelect(home)
          }}
        >
          <span className="grid size-7 place-items-center rounded-sm bg-primary text-primary-foreground">
            <CandlestickIcon size={16} playOnMount duration={1.4} />
          </span>
          <span className="flex min-w-0 items-baseline gap-2">
            <strong className="text-[12px] font-extrabold tracking-[0.16em] uppercase">
              {brand}
            </strong>
            <small className="hidden font-mono text-[10px] tracking-[0.08em] text-primary uppercase sm:inline">
              Simulación · paper
            </small>
            {/* el nombre accesible sale del texto visible (WCAG 2.5.3, axe, #661) */}
            <span className="sr-only">: ir a {homeLabel}</span>
          </span>
        </a>

        {/* la línea de comandos (#708): abre la paleta, también con ⌘K o `/` */}
        <button
          type="button"
          onClick={() => setSearching(true)}
          aria-keyshortcuts="Meta+K Control+K /"
          className="col-span-2 row-start-2 flex min-h-8 items-center gap-2 rounded-sm border border-border bg-background px-2.5 text-left font-mono text-[12px] text-muted-foreground hover:border-rule hover:text-foreground md:col-span-1 md:row-start-auto"
        >
          <span aria-hidden className="font-bold text-primary">
            &gt;
          </span>
          <span className="truncate">Buscar o elegir un comando</span>
          <kbd className="ml-auto hidden rounded-sm border border-border px-1.5 text-[10px] sm:inline">
            {SHORTCUT}
          </kbd>
          <kbd className="hidden rounded-sm border border-border px-1.5 text-[10px] sm:inline">
            /
          </kbd>
        </button>

        <div ref={popButtons} className="relative flex items-center justify-end gap-1">
          <IconButton
            label="Atajos de teclado"
            expanded={popover === 'keys'}
            onClick={() => setPopover(popover === 'keys' ? null : 'keys')}
          >
            <Keyboard />
          </IconButton>
          <IconButton
            label="Ajustes"
            expanded={popover === 'settings'}
            onClick={() => setPopover(popover === 'settings' ? null : 'settings')}
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
              <i className="absolute top-1.5 right-1.5 size-1.5 rounded-full bg-down" />
            )}
          </IconButton>
          <span
            title={user.name}
            className="hidden h-8 place-items-center rounded-sm border border-border px-2 font-mono text-[11px] font-bold sm:grid"
          >
            {user.initials}
          </span>

          {popover === 'notices' && (
            <Popover title="Avisos" panelRef={popPanel} onClose={() => setPopover(null)}>
              {notices.length === 0 ? (
                <p className="text-[12px] text-muted-foreground">No hay avisos nuevos.</p>
              ) : (
                <ul className="grid divide-y divide-border">
                  {notices.map((n) => (
                    <li key={n.id} className="py-2 first:pt-0 last:pb-0">
                      <strong className="block text-[12px]">{n.title}</strong>
                      <span className="text-[11px] text-muted-foreground">{n.detail}</span>
                    </li>
                  ))}
                </ul>
              )}
            </Popover>
          )}
          {popover === 'keys' && (
            <Popover title="Atajos de teclado" panelRef={popPanel} onClose={() => setPopover(null)}>
              <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-[12px]">
                {Object.entries(fkeys).map(([id, key]) => (
                  <KeyRow key={id} keys={key}>
                    {views.find((v) => v.id === id)?.label ?? id}
                  </KeyRow>
                ))}
                <KeyRow keys={`${SHORTCUT} · /`}>Línea de comandos</KeyRow>
                <KeyRow keys="?">Esta lista</KeyRow>
                <KeyRow keys="Esc">Cerrar</KeyRow>
              </dl>
            </Popover>
          )}
          {popover === 'settings' && (
            <Popover title="Ajustes" panelRef={popPanel} onClose={() => setPopover(null)}>
              {/*
               * Un formulario por grupos (#647, patrón *Settings Sidebar Layout* / *Form layout* de
               * 21st.dev, reescrito): cada grupo con su título y para qué sirve, y los controles en
               * filas separadas por líneas finas. Sin cajas dentro del popover.
               */}
              <SettingsGroup
                title="Tema"
                hint="Claro u oscuro para toda la app. La Terminal es siempre oscura."
              >
                <button
                  type="button"
                  onClick={() => onThemeChange(theme === 'dark' ? 'light' : 'dark')}
                  className="flex min-h-8 items-center gap-2 text-[12px] font-semibold text-foreground underline-offset-4 hover:underline"
                >
                  {theme === 'dark' ? (
                    <Sun className="size-4" aria-hidden="true" />
                  ) : (
                    <Moon className="size-4" aria-hidden="true" />
                  )}
                  {theme === 'dark' ? 'Pasar al tema claro' : 'Pasar al tema oscuro'}
                </button>
              </SettingsGroup>
              {onReplayIntro && (
                <SettingsGroup title="Intro" hint="La animación del comienzo, una vez más.">
                  <button
                    type="button"
                    onClick={() => {
                      setPopover(null)
                      onReplayIntro()
                    }}
                    className="flex min-h-8 items-center gap-2 text-[12px] font-semibold text-foreground underline-offset-4 hover:underline"
                  >
                    <Play className="size-4" aria-hidden="true" />
                    Ver la intro otra vez
                  </button>
                </SettingsGroup>
              )}
            </Popover>
          )}
        </div>
      </header>

      <div className="relative flex min-w-0 flex-1">
        <Dock
          views={nav}
          active={active}
          section={section}
          fkeys={fkeys}
          onSelect={onSelect}
          onMore={menu ? openMore : undefined}
          side
        />
        <main
          id="contenido"
          tabIndex={-1}
          className="min-w-0 flex-1 px-3 py-3 outline-none sm:px-4"
        >
          <header className="mb-3 flex flex-wrap items-end justify-between gap-x-4 gap-y-2 border-b border-border pb-2">
            <div className="min-w-0">
              <h1 className="text-[20px] leading-tight font-bold tracking-[-0.01em]">
                {current.label}
              </h1>
              <p className="mt-0.5 text-[12px] text-muted-foreground">{current.description}</p>
            </div>
            {tabs.length > 0 && (
              <Tabs
                tabs={tabs}
                active={active}
                section={section}
                fkeys={fkeys}
                onSelect={onSelect}
              />
            )}
          </header>
          {children}
          <p
            role="note"
            className="mt-6 border-t border-border pt-2 text-[11px] text-muted-foreground"
          >
            Prototipo en <strong className="text-foreground">simulación (paper)</strong>: muestra lo
            que guardó el motor. Lo que todavía no tiene datos del motor lleva el sello EJEMPLO. La
            interfaz no calcula nada ni envía órdenes reales. Nada de lo que se ve acá es una señal
            real ni una promesa de ganancia.
          </p>
        </main>
      </div>
      <StatusBar className="md:sticky md:bottom-0 md:z-20" />
      <Dock
        views={bar}
        active={active}
        section={section}
        fkeys={fkeys}
        onSelect={onSelect}
        onMore={openMore}
      />

      {menuOpen && menu && (
        <ViewsMenu
          groups={menu}
          active={active}
          section={section}
          onClose={() => setMenuOpen(false)}
          onSelect={(id) => {
            setMenuOpen(false)
            onSelect(id)
          }}
          onSearch={() => {
            setMenuOpen(false)
            // después de que el menú devuelve el foco a "Más": la paleta lo toma desde ahí
            window.setTimeout(() => setSearching(true), 0)
          }}
        />
      )}

      {searching && (
        <CommandPalette
          views={views}
          fkeys={fkeys}
          active={active}
          theme={theme}
          onThemeChange={onThemeChange}
          onReplayIntro={onReplayIntro}
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

/** Un atajo de una tecla no debe saltar mientras se escribe o se usa un control. */
const typing = (t: EventTarget | null) =>
  t instanceof HTMLElement &&
  (t.isContentEditable || ['INPUT', 'SELECT', 'TEXTAREA'].includes(t.tagName))

/**
 * Las pestañas principales. Son enlaces con su hash (entran en el historial). Desde #708, una línea
 * fina debajo de la activa (ámbar), como las pestañas de una terminal; cada una con su tecla.
 */
function Tabs({
  tabs,
  active,
  section,
  fkeys,
  onSelect,
}: {
  tabs: ShellView[]
  active: string
  section: string
  fkeys: Readonly<Record<string, string>>
  onSelect: (id: string) => void
}) {
  return (
    // en el celular cada una toma el ancho de su nombre (sin íconos ni teclas)
    <nav
      aria-label="Secciones"
      className="flex w-full border-b border-border sm:grid sm:w-auto sm:grid-cols-[repeat(var(--tabs),minmax(0,1fr))] sm:border-b-0"
      style={{ '--tabs': tabs.length } as CSSProperties}
    >
      {tabs.map(({ id, label, icon: Icon }) => {
        const on = id === section
        const key = fkeys[id]
        return (
          <a
            key={id}
            href={`#${id}`}
            aria-current={id === active ? 'page' : on ? 'true' : undefined}
            aria-keyshortcuts={key}
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey || e.shiftKey) return
              e.preventDefault()
              onSelect(id)
            }}
            className={cn(
              '-mb-px flex min-h-8 flex-auto items-center justify-center gap-1.5 border-b-2 px-2 text-[12px] font-bold whitespace-nowrap sm:px-3',
              on
                ? 'border-primary text-foreground'
                : 'border-transparent text-muted-foreground hover:text-foreground',
            )}
          >
            <Icon className="hidden size-[14px] sm:block" aria-hidden="true" />
            {label}
          </a>
        )
      })}
    </nav>
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
      className={cn(
        'relative grid size-8 place-items-center rounded-sm border border-transparent hover:border-border hover:bg-surface [&_svg]:size-[15px]',
        expanded && 'border-border bg-surface text-primary',
      )}
    >
      {children}
    </button>
  )
}

function Popover({
  title,
  panelRef,
  onClose,
  children,
}: {
  title: string
  panelRef: RefObject<HTMLElement | null>
  onClose: () => void
  children: ReactNode
}) {
  const id = useId()
  return (
    <section
      ref={panelRef}
      aria-labelledby={id}
      className="panel-raised absolute top-[calc(100%+6px)] right-0 z-30 w-[min(300px,calc(100vw-24px))] rounded-sm p-3"
    >
      <header className="mb-2 flex items-center justify-between border-b border-border pb-2">
        <h2 id={id} className="text-[11px] font-bold tracking-[0.1em] uppercase">
          {title}
        </h2>
        <button
          type="button"
          aria-label={`Cerrar ${title.toLowerCase()}`}
          onClick={onClose}
          className="rounded-sm p-1 hover:bg-surface"
        >
          <X className="size-4" />
        </button>
      </header>
      {children}
    </section>
  )
}

/** Una fila de la lista de atajos: la tecla y lo que hace. */
function KeyRow({ keys, children }: { keys: string; children: ReactNode }) {
  return (
    <>
      <dt>
        <kbd className="rounded-sm border border-border px-1.5 text-[11px]">{keys}</kbd>
      </dt>
      <dd className="text-muted-foreground">{children}</dd>
    </>
  )
}

/** Un grupo de ajustes: título, para qué sirve y sus controles, con una línea fina arriba. */
function SettingsGroup({
  title,
  hint,
  children,
}: {
  title: string
  hint: string
  children: ReactNode
}) {
  return (
    <section className="grid gap-2 border-t border-border py-2.5 first:border-t-0 first:pt-0 last:pb-0">
      <div className="grid gap-0.5">
        <h3 className="text-[12px] font-bold">{title}</h3>
        <p className="text-[11px] leading-snug text-muted-foreground">{hint}</p>
      </div>
      {children}
    </section>
  )
}

function Dock({
  views,
  active,
  section = active,
  fkeys = {},
  onSelect,
  onMore,
  side = false,
}: {
  views: ShellView[]
  active: string
  /** la sección de la vista abierta: se marca aunque la vista sea una de adentro */
  section?: string
  fkeys?: Readonly<Record<string, string>>
  onSelect: (id: string) => void
  /** abre "Todas las vistas" (#618) */
  onMore?: () => void
  side?: boolean
}) {
  // la vista abierta no está en esta barra (por ejemplo Noticias en la de abajo): se marca "Más"
  const elsewhere = !views.some((v) => v.id === section)
  const more = onMore && (
    <button
      type="button"
      aria-label={elsewhere ? 'Más vistas (la abierta está ahí)' : 'Más vistas'}
      title="Todas las vistas"
      onClick={onMore}
      className={cn(
        'flex min-h-9 shrink-0 items-center gap-2 text-[11px] font-bold',
        side
          ? 'w-full rounded-sm px-2.5 text-muted-foreground hover:bg-surface hover:text-foreground'
          : 'min-w-[56px] flex-col justify-center gap-0.5 px-2 py-1',
        elsewhere && (side ? 'text-primary' : 'border-t-2 border-primary text-primary'),
      )}
    >
      <Ellipsis className="size-[16px]" aria-hidden="true" />
      <span className={cn(side && 'hidden lg:inline')}>{side ? 'Más vistas' : 'Más'}</span>
    </button>
  )

  const links = (
    <nav
      aria-label={side ? 'Accesos' : 'Vistas'}
      className={cn(
        'flex',
        side
          ? 'sticky top-[45px] hidden h-[calc(100svh-45px-29px)] w-12 shrink-0 flex-col gap-0.5 self-start overflow-y-auto border-r border-border bg-card p-1 md:flex lg:w-44'
          : 'min-w-0 flex-1 justify-between',
      )}
    >
      {views.map(({ id, label, short, icon: Icon }) => {
        const on = id === section
        const key = fkeys[id]
        return (
          <a
            key={id}
            href={`#${id}`}
            aria-current={id === active ? 'page' : on ? 'true' : undefined}
            aria-keyshortcuts={key}
            title={side ? (key ? `${label} (${key})` : label) : undefined}
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey || e.shiftKey) return
              e.preventDefault()
              onSelect(id)
            }}
            className={cn(
              'flex min-h-9 items-center text-[11px] font-bold',
              side
                ? 'gap-2.5 rounded-sm border-l-2 px-2 lg:px-2.5'
                : 'min-w-0 flex-1 flex-col justify-center gap-0.5 border-t-2 px-1 py-1 whitespace-nowrap',
              on
                ? 'border-primary bg-surface text-foreground'
                : 'border-transparent text-muted-foreground hover:bg-surface hover:text-foreground',
            )}
          >
            <Icon className={cn('size-[16px] shrink-0', on && 'text-primary')} aria-hidden="true" />
            {side ? (
              // el nombre es el texto visible (WCAG 2.5.3); entre 768 y 1024 px, solo para el lector
              <span className="min-w-0 flex-1 items-center lg:flex">
                <span className="sr-only truncate lg:not-sr-only">{label}</span>
                {key && (
                  <kbd
                    aria-hidden
                    className="ml-auto hidden text-[10px] font-normal text-muted-foreground lg:inline"
                  >
                    {key}
                  </kbd>
                )}
              </span>
            ) : (
              <span>{short}</span>
            )}
          </a>
        )
      })}
      {side && more && <span aria-hidden className="mx-1 my-1 h-px bg-border" />}
      {side && more}
    </nav>
  )
  if (side) return links
  // en el celular: las pestañas principales y "Más", sin desplazarse (#618)
  return (
    <div className="fixed inset-x-0 bottom-0 z-20 flex items-stretch border-t border-rule bg-card px-1 pb-[env(safe-area-inset-bottom)] md:hidden">
      {links}
      {more}
    </div>
  )
}

/** El último catálogo que llegó: la próxima vez que se abre la paleta se ve al instante. */
let lastCatalog: { instruments: InstrumentInfo[]; timeframes: readonly string[] } = {
  instruments: [],
  timeframes: [],
}

/** Las partes del Inicio a las que se puede saltar (ids de `views/home.tsx`). */
const HOME_PARTS = [
  { id: 'inicio-cuenta', label: 'Cuenta' },
  { id: 'inicio-mercados', label: 'Mercados' },
  { id: 'inicio-noticias', label: 'Noticias y calendario' },
  { id: 'inicio-resultados', label: 'Resultados recientes' },
  { id: 'inicio-secciones', label: 'Secciones' },
] as const

/**
 * Después de abrir el Inicio, lleva la vista a una de sus partes y le pasa el foco. Espera hasta
 * 2 s a que la vista esté montada; si no aparece, no hace nada.
 */
function jumpTo(id: string, tries = 40) {
  const el = document.getElementById(id)
  if (!el) {
    if (tries > 0) window.setTimeout(() => jumpTo(id, tries - 1), 50)
    return
  }
  if (!el.hasAttribute('tabindex')) el.setAttribute('tabindex', '-1')
  el.scrollIntoView?.({ block: 'start' })
  el.focus({ preventScroll: true })
}

const GROUPS = [
  'Vistas',
  'Instrumentos',
  'Timeframe',
  'Capas',
  'Tema',
  'Ajustes',
  'Inicio',
] as const
type Group = (typeof GROUPS)[number]

type Command = {
  key: string
  group: Group
  label: string
  detail: string
  /** el texto con el que se busca */
  text: string
  icon: LucideIcon
  /** la tecla de función de la vista, si tiene (#708) */
  hotkey?: string
  run: () => void
}

/** "⌘ K" en Mac y en el iPad o el iPhone; "Ctrl K" en lo demás. */
const SHORTCUT =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.userAgent) ? '⌘ K' : 'Ctrl K'

/**
 * Paleta de comandos (⌘K / Ctrl K, #581; antes el buscador de #362). En un solo campo: ir a una
 * vista, abrir un instrumento del catálogo (`GET /api/v1/instruments`) en el gráfico del Terminal,
 * cambiar el timeframe del gráfico activo del Terminal, prender o apagar capas de análisis,
 * cambiar el tema y saltar a una parte del Inicio. Solo navegación y preferencias de pantalla:
 * no calcula nada ni envía órdenes.
 *
 * - Es un `combobox` con su `listbox`, con los resultados en grupos (`role="group"`): ↑/↓ marcan
 *   (con `aria-activedescendant`, el foco queda en el campo), Enter elige la marcada (o la
 *   primera) y Esc cierra.
 * - Con el puntero se elige en `pointerup` sobre la misma opción donde empezó (`pointerdown`), con
 *   `click` de respaldo: si el navegador o una extensión se come el `click`, igual navega. No se
 *   navega en `pointerdown` porque el diálogo desaparecería bajo el dedo y el `pointerup`/`click`
 *   caería sobre lo que hay debajo. En el dedo, desplazar la lista da `pointercancel`, no elige.
 * - Se cierra por el fondo solo si el clic cae fuera del rectángulo del diálogo: un clic en su
 *   borde o en su relleno (que también tiene al diálogo como `target`) no lo cierra.
 */
/** Las vistas con su propio gráfico activo, donde el timeframe de la paleta se queda. */
const CHART_VIEWS = ['graficos', 'multigrafico']

function CommandPalette({
  views,
  fkeys,
  active: view,
  theme,
  onThemeChange,
  onReplayIntro,
  onClose,
  onSelect,
}: {
  views: ShellView[]
  fkeys: Readonly<Record<string, string>>
  /** la vista abierta: en Gráficos y Multigráfico el timeframe se aplica ahí mismo */
  active: string
  theme: 'dark' | 'light'
  onThemeChange: (t: 'dark' | 'light') => void
  onReplayIntro?: () => void
  onClose: () => void
  onSelect: (id: string) => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  // al cerrar, el foco vuelve a donde estaba (el botón de la paleta o "Más vistas"). Se toma al
  // renderizar, antes de que `autoFocus` lo pase al campo.
  const [back] = useState(() =>
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  )
  const base = useId()
  const listId = `${base}-list`
  const optionId = (i: number) => `${base}-opt-${i}`
  const [q, setQ] = useState('')
  // la opción marcada; `null`: la que mejor coincide con lo escrito
  const [mark, setMark] = useState<number | null>(null)
  const pressed = useRef<string | null>(null)
  const chosen = useRef(false)
  const catalog = useInstruments()
  const { instruments, timeframes } =
    catalog.status === 'ok'
      ? { instruments: catalog.data.instruments, timeframes: catalog.data.timeframes }
      : lastCatalog
  useEffect(() => {
    if (catalog.status === 'ok')
      lastCatalog = { instruments: catalog.data.instruments, timeframes: catalog.data.timeframes }
  }, [catalog])
  const [layers, toggleLayer] = useChartTools()
  // Enter antes de que llegue el catálogo: se elige apenas llega (o falla)
  const waiting = useRef(false)

  useEffect(() => {
    const d = ref.current
    // jsdom (tests) no implementa showModal: ahí se abre sin modo modal
    if (d?.showModal && !d.open) d.showModal()
    else d?.setAttribute('open', '')
    return () => back?.focus?.()
  }, [back])

  // trampa de foco (#579): Tab y Mayús+Tab dan la vuelta dentro del diálogo. `showModal` ya deja
  // inerte lo de atrás; esto lo asegura también donde no hay modo modal.
  const trapTab = (e: ReactKeyboardEvent<HTMLDialogElement>) => {
    if (e.key !== 'Tab' || !ref.current) return
    const items = [
      ...ref.current.querySelectorAll<HTMLElement>(
        'input, button, [href], select, textarea, [tabindex]:not([tabindex="-1"])',
      ),
    ].filter((el) => !el.hasAttribute('disabled'))
    if (items.length === 0) return
    const first = items[0]
    const last = items[items.length - 1]
    const at = document.activeElement
    if (e.shiftKey && (at === first || !ref.current.contains(at))) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && (at === last || !ref.current.contains(at))) {
      e.preventDefault()
      first.focus()
    }
  }

  const other = theme === 'dark' ? 'light' : 'dark'
  // Gráficos y Multigráfico (#581, etapa 5) toman el timeframe sin ir al Terminal
  const inCharts = CHART_VIEWS.includes(view)
  const all: Command[] = [
    ...views.map((v) => ({
      key: `view:${v.id}`,
      group: 'Vistas' as const,
      label: v.label,
      detail: v.description,
      text: `${v.label} ${v.description}`,
      icon: v.icon,
      hotkey: fkeys[v.id],
      run: () => onSelect(v.id),
    })),
    ...instruments.map((i) => ({
      key: `instrument:${i.id}`,
      group: 'Instrumentos' as const,
      label: `${i.id} · ${i.name}`,
      detail: `Instrumento (${i.currency}): abre el Terminal con su gráfico.`,
      text: `${i.id} ${i.name} ${i.asset_class}`,
      icon: LineChart,
      run: () => {
        focusInstrument(i.id)
        onSelect('terminal')
      },
    })),
    ...timeframes.map((tf) => ({
      key: `timeframe:${tf}`,
      group: 'Timeframe' as const,
      label: `Timeframe ${tf}`,
      detail: inCharts
        ? 'Cambia el gráfico activo de esta vista.'
        : 'Cambia el gráfico activo del Terminal.',
      text: `timeframe ${tf} temporalidad intervalo`,
      icon: Clock3,
      run: () => {
        focusTimeframe(tf)
        if (inCharts) onClose()
        else onSelect('terminal')
      },
    })),
    ...TOOLS.map((t) => ({
      key: `layer:${t.id}`,
      group: 'Capas' as const,
      label: `${layers[t.id] ? 'Apagar' : 'Prender'} ${t.label}`,
      detail: `Capa de análisis de los gráficos: ahora ${layers[t.id] ? 'prendida' : 'apagada'}.`,
      text: `capa herramienta ${layers[t.id] ? 'apagar' : 'prender'} ${t.label}`,
      icon: Layers,
      run: () => {
        toggleLayer(t.id, !layers[t.id])
        onClose()
      },
    })),
    {
      key: 'theme',
      group: 'Tema',
      label: `Tema ${other === 'light' ? 'claro' : 'oscuro'}`,
      detail: 'Cambia el tema de la app. La Terminal es siempre oscura.',
      text: `tema modo ${other === 'light' ? 'claro' : 'oscuro'}`,
      icon: other === 'light' ? Sun : Moon,
      run: () => {
        onThemeChange(other)
        onClose()
      },
    },
    ...(onReplayIntro
      ? [
          {
            key: 'intro',
            group: 'Ajustes' as const,
            label: 'Ver la intro otra vez',
            detail: 'Muestra de nuevo la introducción "El mercado espera".',
            text: 'ver la intro otra vez introducción ajustes animación',
            icon: Play,
            run: () => {
              onClose()
              onReplayIntro()
            },
          },
        ]
      : []),
    ...HOME_PARTS.map((p) => ({
      key: `home:${p.id}`,
      group: 'Inicio' as const,
      label: `Inicio: ${p.label}`,
      detail: 'Salta a esa parte del Inicio.',
      text: `inicio ${p.label}`,
      icon: House,
      run: () => {
        onSelect('inicio')
        // después de que se cierra la paleta (que devuelve el foco) y se monta el Inicio
        window.setTimeout(() => jumpTo(p.id), 50)
      },
    })),
  ]
  const words = q.toLowerCase().split(/\s+/).filter(Boolean)
  const matches = (text: string) => words.every((w) => text.toLowerCase().includes(w))
  // en el orden de los grupos: el índice de cada opción es el de esta lista
  const items = GROUPS.flatMap((g) => all.filter((c) => c.group === g && matches(c.text)))
  // sin elegir con las flechas se marca la primera, salvo que no tenga la frase escrita tal cual
  // y otra la tenga en su nombre: "inicio resultados" → "Inicio: Resultados recientes"
  const plain = (t: string) =>
    t
      .toLowerCase()
      .replace(/[^\p{L}\p{N}]+/gu, ' ')
      .trim()
  const phrase = plain(q)
  const best =
    phrase && items.length && !plain(items[0].text).includes(phrase)
      ? items.findIndex((c) => plain(c.label).includes(phrase))
      : -1
  const active = items.length ? Math.min(mark ?? Math.max(best, 0), items.length - 1) : -1

  useEffect(() => {
    if (active >= 0)
      document.getElementById(optionId(active))?.scrollIntoView?.({ block: 'nearest' })
    // optionId depende solo de `base`, que no cambia
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active])

  const choose = (item: Command | undefined) => {
    if (!item || chosen.current) return
    chosen.current = true
    item.run()
  }

  useEffect(() => {
    if (!waiting.current || catalog.status === 'loading') return
    waiting.current = false
    choose(items[0])
    // solo cuando llega el catálogo
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [catalog.status])

  const onKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      if (!items.length) return
      const step = e.key === 'ArrowDown' ? 1 : -1
      setMark((active + step + items.length) % items.length)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (items.length === 0 && catalog.status === 'loading') waiting.current = true
      else choose(items[Math.max(active, 0)])
    } else if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }

  const option = (item: Command, i: number) => (
    <div
      key={item.key}
      id={optionId(i)}
      role="option"
      aria-selected={i === active}
      data-kind={item.group}
      onPointerDown={(e) => {
        if (e.button !== 0) return
        pressed.current = item.key
        // el foco queda en el campo (sin blur ni re-render entre el down y el up)
        e.preventDefault()
      }}
      onPointerUp={(e) => {
        if (e.button === 0 && pressed.current === item.key) choose(item)
        pressed.current = null
      }}
      onPointerCancel={() => (pressed.current = null)}
      onClick={() => choose(item)}
      onMouseMove={() => i !== active && setMark(i)}
      className={cn(
        'flex cursor-pointer items-center gap-2.5 rounded-sm border-l-2 border-transparent px-2 py-1.5 text-left select-none',
        i === active && 'border-primary bg-surface',
      )}
    >
      <item.icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span className="min-w-0 flex-1">
        <strong className="block text-[13px]">{item.label}</strong>
        <small className="block truncate text-[11px] text-muted-foreground">{item.detail}</small>
      </span>
      {item.hotkey && (
        <kbd aria-hidden className="shrink-0 text-[10px] text-muted-foreground">
          {item.hotkey}
        </kbd>
      )}
    </div>
  )

  return (
    <dialog
      ref={ref}
      role="dialog"
      aria-modal="true"
      aria-label="Paleta de comandos"
      onKeyDown={trapTab}
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault()
        onClose()
      }}
      onClick={(e) => {
        if (e.target !== e.currentTarget) return
        const r = e.currentTarget.getBoundingClientRect()
        const outside =
          e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom
        if (outside) onClose()
      }}
      className="panel-raised m-auto mt-[12svh] w-[min(600px,calc(100%-24px))] rounded-sm p-0 text-foreground backdrop:bg-black/60"
    >
      <div className="flex items-center gap-2 border-b border-rule p-2">
        <label className="flex min-w-0 flex-1 items-center gap-2 px-1.5">
          <Terminal className="size-4 text-primary" aria-hidden="true" />
          <input
            autoFocus
            role="combobox"
            aria-label="Buscar"
            aria-expanded={items.length > 0}
            aria-controls={listId}
            aria-autocomplete="list"
            aria-activedescendant={active >= 0 ? optionId(active) : undefined}
            placeholder="Vista, instrumento o comando: US100, 1h, tema…"
            value={q}
            onChange={(e) => {
              setQ(e.target.value)
              setMark(null)
              waiting.current = false
            }}
            onKeyDown={onKeyDown}
            className="min-h-10 min-w-0 flex-1 bg-transparent font-mono text-[14px] outline-none placeholder:text-muted-foreground"
          />
        </label>
        <button
          type="button"
          aria-label="Cerrar la paleta"
          onClick={onClose}
          className="grid size-9 shrink-0 place-items-center rounded-sm border border-border hover:bg-surface"
        >
          <X className="size-4" aria-hidden="true" />
        </button>
      </div>
      <div
        id={listId}
        role="listbox"
        aria-label="Resultados"
        className="grid max-h-[min(60svh,460px)] grid-cols-[minmax(0,1fr)] gap-1 overflow-y-auto p-1.5"
      >
        {GROUPS.map((g) => {
          const inGroup = items.filter((c) => c.group === g)
          if (inGroup.length === 0) return null
          const headId = `${base}-group-${g}`
          return (
            <div key={g} role="group" aria-labelledby={headId} data-group={g}>
              <div
                id={headId}
                role="presentation"
                className="px-2 pt-1 pb-0.5 font-mono text-[10px] font-bold tracking-[0.12em] text-info uppercase"
              >
                {g}
              </div>
              {inGroup.map((item) => option(item, items.indexOf(item)))}
            </div>
          )
        })}
      </div>
      {items.length === 0 && (
        <p className="px-3 pb-2 text-[12px] text-muted-foreground">
          No hay vistas, instrumentos ni comandos con ese nombre.
        </p>
      )}
      {catalog.status === 'loading' && instruments.length === 0 && (
        <p className="px-3 pb-2 text-[11px] text-muted-foreground">Pidiendo los instrumentos…</p>
      )}
      {catalog.status === 'error' && (
        <p className="px-3 pb-2 text-[11px] text-muted-foreground">
          Sin instrumentos: el gateway no respondió. Las vistas y los comandos andan igual.
        </p>
      )}
      <p className="hidden border-t border-border px-3 py-1.5 text-[10.5px] text-muted-foreground sm:block">
        <kbd>↑</kbd> <kbd>↓</kbd> para moverse, <kbd>Enter</kbd> para elegir, <kbd>Esc</kbd> para
        cerrar.
      </p>
    </dialog>
  )
}
