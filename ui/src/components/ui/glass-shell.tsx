/**
 * Armazón de la app: barra superior, navegación (lateral en escritorio, inferior en el celular),
 * escenario, paleta de comandos (⌘K), avisos y ajustes (vidrio del Inicio y del Fondo, ver la intro).
 *
 * Une dos componentes de referencia que hacían lo mismo (#164): "Glassy Dashboard" (barra,
 * docks y búsqueda) y "Adjustable Glass Dashboard" (panel para ajustar el vidrio). Queda un solo
 * armazón; el contenido de cada vista lo decide la app.
 */
import {
  Bell,
  Clock3,
  Ellipsis,
  House,
  Layers,
  LineChart,
  Moon,
  Play,
  Search,
  SlidersHorizontal,
  Sun,
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
import { PRESETS, type GlassSettings } from '@/components/ui/glass-settings'
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

type Popover = 'notices' | 'settings' | null

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
  glass,
  onGlassChange,
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

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setSearching(true)
      }
      if (e.key === 'Escape') setPopover(null)
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
    <div
      className="relative mx-auto flex min-h-svh w-full max-w-[1400px] flex-col px-3 pt-[max(0.75rem,env(safe-area-inset-top))] pb-[calc(6rem+env(safe-area-inset-bottom))] sm:px-6 sm:pt-6 md:pb-6"
      style={
        {
          '--glass-blur': `${glass.blur}px`,
          '--glass-opacity': glass.opacity,
        } as CSSProperties
      }
    >
      <a
        href="#contenido"
        className="sr-only z-50 rounded-full bg-primary px-4 py-2 text-[13px] font-bold text-primary-foreground focus:not-sr-only focus:fixed focus:top-3 focus:left-3"
      >
        Saltar al contenido
      </a>
      <header className="glass relative z-20 grid grid-cols-[1fr_auto] items-center gap-3 rounded-[22px] px-3 py-2 md:grid-cols-[1fr_minmax(220px,380px)_1fr] md:rounded-full">
        <a
          href={`#${home}`}
          className="flex min-w-0 items-center gap-2.5"
          onClick={(e) => {
            if (e.metaKey || e.ctrlKey || e.shiftKey) return
            e.preventDefault()
            onSelect(home)
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
            {/* el nombre accesible sale del texto visible (WCAG 2.5.3, axe, #661) */}
            <span className="sr-only">: ir a {homeLabel}</span>
          </span>
        </a>

        <button
          type="button"
          onClick={() => setSearching(true)}
          className="col-span-2 row-start-2 flex min-h-10 items-center gap-2.5 rounded-full border border-border bg-[color-mix(in_srgb,var(--card)_40%,transparent)] px-3 text-left text-[12px] font-semibold text-muted-foreground hover:text-foreground md:col-span-1 md:row-start-auto"
        >
          <Search className="size-4" aria-hidden="true" />
          <span className="truncate">Buscar o elegir un comando</span>
          <kbd className="ml-auto hidden rounded border border-border px-1.5 text-[10px] sm:inline">
            {SHORTCUT}
          </kbd>
        </button>

        <div ref={popButtons} className="relative flex items-center justify-end gap-1.5">
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
            <Popover title="Avisos" panelRef={popPanel} onClose={() => setPopover(null)}>
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
          {popover === 'settings' && (
            <Popover title="Ajustes" panelRef={popPanel} onClose={() => setPopover(null)}>
              {/*
               * Un formulario por grupos (#647, patrón *Settings Sidebar Layout* / *Form layout* de
               * 21st.dev, reescrito): cada grupo con su título y para qué sirve, y los controles en
               * filas separadas por líneas finas. Sin cajas dentro del popover.
               */}
              <SettingsGroup
                title="Vidrio del Inicio y del Fondo"
                hint="Solo se ve en el Inicio, el ingreso y el Fondo. Las demás secciones van lisas."
              >
                <div
                  role="group"
                  aria-label="Vidrio predefinido"
                  className="flex gap-0.5 rounded-full border border-border p-0.5"
                >
                  {PRESETS.map((p) => (
                    <button
                      key={p.name}
                      type="button"
                      onClick={() => onGlassChange(p.value)}
                      className="min-h-7 flex-1 rounded-full text-[11px] font-semibold hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]"
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
              </SettingsGroup>
              {onReplayIntro && (
                <SettingsGroup title="Intro" hint="La animación del comienzo, una vez más.">
                  <button
                    type="button"
                    onClick={() => {
                      setPopover(null)
                      onReplayIntro()
                    }}
                    className="flex min-h-9 items-center gap-2 text-[12px] font-semibold text-foreground underline-offset-4 hover:underline"
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

      <div className="relative mt-3 flex flex-1 gap-3">
        <Dock
          views={nav}
          active={active}
          section={section}
          onSelect={onSelect}
          onMore={menu ? openMore : undefined}
          side
        />
        <main
          id="contenido"
          tabIndex={-1}
          className="glass min-w-0 flex-1 rounded-[28px] p-4 outline-none sm:p-7"
        >
          <header className="mb-5 flex flex-wrap items-end justify-between gap-3">
            <div>
              <h1 className="text-[clamp(26px,3vw,42px)] leading-none font-bold tracking-[-0.05em]">
                {current.label}
              </h1>
              <p className="mt-2 text-[13px] text-muted-foreground">{current.description}</p>
            </div>
            {tabs.length > 0 && (
              <Tabs tabs={tabs} active={active} section={section} onSelect={onSelect} />
            )}
          </header>
          {children}
          <p
            role="note"
            className="mt-8 rounded-xl border border-border px-3 py-2 text-[11px] text-muted-foreground"
          >
            Prototipo en <strong className="text-foreground">simulación (paper)</strong>: muestra lo
            que guardó el motor. Lo que todavía no tiene datos del motor lleva el sello EJEMPLO. La
            interfaz no calcula nada ni envía órdenes reales. Nada de lo que se ve acá es una señal
            real ni una promesa de ganancia.
          </p>
        </main>
      </div>
      <Dock views={bar} active={active} section={section} onSelect={onSelect} onMore={openMore} />

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

/**
 * Las pestañas principales. Son enlaces con su hash (entran en el historial); la marca de la
 * pestaña activa se desliza con `transform` (barata, sin animar el layout).
 */
function Tabs({
  tabs,
  active,
  section,
  onSelect,
}: {
  tabs: ShellView[]
  active: string
  section: string
  onSelect: (id: string) => void
}) {
  const at = tabs.findIndex((t) => t.id === section)
  return (
    // en el celular las cuatro no entran iguales: cada una toma el ancho de su nombre (sin íconos)
    // y la activa lleva su propio fondo; desde `sm`, columnas iguales con la marca que se desliza
    <nav
      aria-label="Secciones"
      className="relative flex w-full rounded-full border border-border p-1 sm:grid sm:w-auto sm:grid-cols-[repeat(var(--tabs),minmax(0,1fr))]"
      style={{ '--tabs': tabs.length } as CSSProperties}
    >
      {at >= 0 && (
        <span
          aria-hidden
          className="tab-thumb absolute inset-y-1 left-1 hidden rounded-full bg-primary sm:block"
          style={{
            width: `calc((100% - 8px) / ${tabs.length})`,
            transform: `translateX(${at * 100}%)`,
          }}
        />
      )}
      {tabs.map(({ id, label, icon: Icon }) => {
        const on = id === section
        return (
          <a
            key={id}
            href={`#${id}`}
            aria-current={id === active ? 'page' : on ? 'true' : undefined}
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey || e.shiftKey) return
              e.preventDefault()
              onSelect(id)
            }}
            className={cn(
              'relative z-10 flex min-h-9 flex-auto items-center justify-center gap-1.5 rounded-full px-2 text-[12px] font-bold whitespace-nowrap sm:px-4',
              on
                ? 'bg-primary text-primary-foreground sm:bg-transparent'
                : 'text-muted-foreground hover:text-foreground',
            )}
          >
            <Icon className="hidden size-[15px] sm:block" aria-hidden="true" />
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
      className="relative grid size-10 place-items-center rounded-full border border-border hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] [&_svg]:size-[17px]"
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
      className="glass-strong absolute top-[calc(100%+10px)] right-0 z-30 w-[min(300px,calc(100vw-24px))] rounded-2xl p-4"
    >
      <header className="mb-3 flex items-center justify-between">
        <h2 id={id} className="text-[14px] font-bold">
          {title}
        </h2>
        <button
          type="button"
          aria-label={`Cerrar ${title.toLowerCase()}`}
          onClick={onClose}
          className="rounded-full p-1"
        >
          <X className="size-4" />
        </button>
      </header>
      {children}
    </section>
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
    <section className="grid gap-3 border-t border-border py-3 first:border-t-0 first:pt-0 last:pb-0">
      <div className="grid gap-0.5">
        <h3 className="text-[12px] font-bold">{title}</h3>
        <p className="text-[11px] leading-snug text-muted-foreground">{hint}</p>
      </div>
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
    <label className="block text-[11px] font-semibold">
      <span className="flex justify-between">
        {label}
        <output className="text-muted-foreground">{text}</output>
      </span>
      <input
        type="range"
        aria-label={label}
        aria-valuetext={text}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="mt-2 block h-6 w-full accent-[var(--primary)]"
      />
    </label>
  )
}

function Dock({
  views,
  active,
  section = active,
  onSelect,
  onMore,
  side = false,
}: {
  views: ShellView[]
  active: string
  /** la sección de la vista abierta: se marca aunque la vista sea una de adentro */
  section?: string
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
        'flex min-h-11 shrink-0 items-center justify-center gap-1 rounded-full text-[10px] font-bold transition-colors',
        side ? 'size-11' : 'min-w-[56px] flex-col px-2 py-1',
        elsewhere
          ? 'bg-primary text-primary-foreground'
          : 'hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]',
      )}
    >
      <Ellipsis className="size-[17px]" aria-hidden="true" />
      {!side && <span>Más</span>}
    </button>
  )

  const links = (
    <nav
      aria-label={side ? 'Accesos' : 'Vistas'}
      className={cn(
        'flex gap-1',
        side
          ? 'glass sticky top-6 z-20 hidden h-fit flex-col self-start rounded-full p-1.5 md:flex'
          : 'min-w-0 flex-1 justify-between',
      )}
    >
      {views.map(({ id, label, short, icon: Icon }) => {
        const on = id === section
        return (
          <a
            key={id}
            href={`#${id}`}
            aria-current={id === active ? 'page' : on ? 'true' : undefined}
            aria-label={side ? label : undefined}
            title={side ? label : undefined}
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey || e.shiftKey) return
              e.preventDefault()
              onSelect(id)
            }}
            className={cn(
              'flex min-h-11 items-center justify-center gap-1 rounded-full text-[10px] font-bold transition-colors',
              side ? 'size-11' : 'min-w-0 flex-1 flex-col px-1 py-1 whitespace-nowrap',
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
      {side && more && <span aria-hidden className="mx-2.5 h-px bg-border" />}
      {side && more}
    </nav>
  )
  if (side) return links
  // en el celular: las pestañas principales y "Más", sin desplazarse (#618)
  return (
    <div className="glass fixed inset-x-3 bottom-[max(0.75rem,env(safe-area-inset-bottom))] z-20 flex items-stretch gap-1 rounded-full p-1.5 md:hidden">
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
  { id: 'inicio-secciones', label: 'Secciones' },
  { id: 'inicio-resultados', label: 'Resultados recientes' },
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
  active: view,
  theme,
  onThemeChange,
  onReplayIntro,
  onClose,
  onSelect,
}: {
  views: ShellView[]
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
        'flex cursor-pointer items-center gap-3 rounded-xl px-2.5 py-2 text-left select-none',
        i === active && 'bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]',
      )}
    >
      <item.icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <span className="min-w-0">
        <strong className="block text-[13px]">{item.label}</strong>
        <small className="block truncate text-[11px] text-muted-foreground">{item.detail}</small>
      </span>
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
      className="glass-strong m-auto w-[min(520px,calc(100%-24px))] rounded-3xl p-4 text-foreground backdrop:bg-black/40 backdrop:backdrop-blur-sm"
    >
      <div className="flex items-center gap-2">
        <label className="flex min-w-0 flex-1 items-center gap-2 rounded-xl border border-border px-3">
          <Search className="size-4 text-muted-foreground" aria-hidden="true" />
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
            className="min-h-11 min-w-0 flex-1 bg-transparent text-[14px] outline-none"
          />
        </label>
        <button
          type="button"
          aria-label="Cerrar la paleta"
          onClick={onClose}
          className="grid size-11 shrink-0 place-items-center rounded-full border border-border hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)]"
        >
          <X className="size-4" aria-hidden="true" />
        </button>
      </div>
      <div
        id={listId}
        role="listbox"
        aria-label="Resultados"
        className="mt-3 grid max-h-[min(60svh,440px)] grid-cols-[minmax(0,1fr)] gap-2 overflow-y-auto"
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
                className="px-2.5 pb-1 text-[10.5px] font-bold tracking-[0.1em] text-muted-foreground uppercase"
              >
                {g}
              </div>
              {inGroup.map((item) => option(item, items.indexOf(item)))}
            </div>
          )
        })}
      </div>
      {items.length === 0 && (
        <p className="p-2 text-[12px] text-muted-foreground">
          No hay vistas, instrumentos ni comandos con ese nombre.
        </p>
      )}
      {catalog.status === 'loading' && instruments.length === 0 && (
        <p className="px-2 pt-2 text-[11px] text-muted-foreground">Pidiendo los instrumentos…</p>
      )}
      {catalog.status === 'error' && (
        <p className="px-2 pt-2 text-[11px] text-muted-foreground">
          Sin instrumentos: el gateway no respondió. Las vistas y los comandos andan igual.
        </p>
      )}
      <p className="hidden px-2 pt-2 text-[10.5px] text-muted-foreground sm:block">
        <kbd>↑</kbd> <kbd>↓</kbd> para moverse, <kbd>Enter</kbd> para elegir, <kbd>Esc</kbd> para
        cerrar.
      </p>
    </dialog>
  )
}
