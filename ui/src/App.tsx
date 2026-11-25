/**
 * Prototipo de la interfaz (#164, sigue al #30): carga → ingreso → armazón con las vistas.
 * Todo con datos de ejemplo: no se conecta al motor ni a IG y no calcula nada (CLAUDE.md §1).
 *
 * Tres pestañas principales (#286): Performance, Panel y Terminal. Cada vista tiene su hash y entra
 * en el historial, así atrás/adelante del navegador cambia de pestaña. El deslizamiento horizontal
 * del trackpad y los atajos de teclado pasan a la pestaña anterior o siguiente.
 */
import { useEffect, useState } from 'react'

import { LoadingScreen } from '@/components/ui/dot-loader'
import { DEFAULT_GLASS } from '@/components/ui/glass-settings'
import { GlassShell } from '@/components/ui/glass-shell'
import { HalftoneWallpaper } from '@/components/ui/halftone-wallpaper'
import { NotFound } from '@/components/ui/not-found'
import { PortalTransition } from '@/components/ui/portal-transition'
import { SignIn } from '@/components/ui/sign-in'
import { CURRENT_MEMBER, NOTICES } from '@/mocks'
import { ChartsView, CompetitionView, JournalView, NewsView } from '@/views'
import { useTabSwipe } from '@/lib/use-tab-swipe'
import { LiquidityView } from '@/views/liquidity'
import { MultichartView } from '@/views/multichart'
import { PanelView } from '@/views/panel'
import { PerformanceView } from '@/views/performance'
import { TerminalView } from '@/views/terminal'
import { HOME_VIEW, MAIN_TABS, VIEWS } from '@/views/nav'
import { FundView, GlossaryView, ProposalsView } from '@/views/phase10'

const NOT_FOUND = 'no-encontrada'
const viewFromHash = () => {
  const id = typeof location === 'undefined' ? '' : location.hash.slice(1)
  if (!id) return HOME_VIEW
  return VIEWS.some((v) => v.id === id) ? id : NOT_FOUND
}

type Stage = 'loading' | 'signin' | 'app'
type Theme = 'dark' | 'light'

/**
 * Tema (#579): el que eligió la persona (guardado en este navegador) o, si nunca eligió, el del
 * sistema, y lo sigue si cambia. Sin `localStorage` (navegación privada) usa el del sistema.
 */
const THEME_KEY = 'cfds.theme'
const savedTheme = (): Theme | null => {
  try {
    const v = localStorage.getItem(THEME_KEY)
    return v === 'dark' || v === 'light' ? v : null
  } catch {
    return null
  }
}
const LIGHT_QUERY = '(prefers-color-scheme: light)'
const systemTheme = (): Theme =>
  typeof window !== 'undefined' && window.matchMedia?.(LIGHT_QUERY).matches ? 'light' : 'dark'

/** Hacia dónde entra la vista nueva: por el orden de la navegación (para la transición). */
const order = (id: string) => VIEWS.findIndex((v) => v.id === id)
const TABS = MAIN_TABS.map((id) => VIEWS.find((v) => v.id === id)!)
const navTo = (id: string) => (prev: { view: string; dir: number }) => ({
  view: id,
  dir: Math.sign(order(id) - order(prev.view)),
})

/**
 * El ingreso es una maqueta (no se conecta a nada). `infra/scripts/ui_liquidez.sh` lo saltea con
 * `VITE_SKIP_SIGNIN=1` para abrir directo en la vista pedida (#231). Solo en desarrollo.
 */
const FIRST_STAGE: Stage =
  import.meta.env.DEV && import.meta.env.VITE_SKIP_SIGNIN === '1' ? 'app' : 'loading'

/**
 * La transición de entrada (#414) sale solo al aceptarse el ingreso: no al recargar ni al cambiar
 * de vista, y tampoco cuando se saltea el ingreso. Para probarla igual sin pasar por el ingreso,
 * en desarrollo: `VITE_PORTAL_PREVIEW=1`.
 */
const PORTAL_ON_START = import.meta.env.DEV && import.meta.env.VITE_PORTAL_PREVIEW === '1'

export default function App({ initialStage = FIRST_STAGE }: { initialStage?: Stage }) {
  const [stage, setStage] = useState<Stage>(initialStage)
  const [portal, setPortal] = useState(PORTAL_ON_START && initialStage === 'app')
  const [{ view, dir }, setNav] = useState(() => ({ view: viewFromHash(), dir: 0 }))
  const [theme, setThemeState] = useState<Theme>(() => savedTheme() ?? systemTheme())
  const setTheme = (t: Theme) => {
    setThemeState(t)
    try {
      localStorage.setItem(THEME_KEY, t)
    } catch {
      // sin almacenamiento: vale para esta visita
    }
  }

  useEffect(() => {
    const mq = window.matchMedia?.(LIGHT_QUERY)
    if (!mq) return
    const onChange = (e: MediaQueryListEvent) => {
      if (!savedTheme()) setThemeState(e.matches ? 'light' : 'dark')
    }
    mq.addEventListener?.('change', onChange)
    return () => mq.removeEventListener?.('change', onChange)
  }, [])
  const [glass, setGlass] = useState(DEFAULT_GLASS)

  useEffect(() => {
    document.documentElement.dataset.theme = theme
    // barra del sistema en el celular (PWA, #307), del color del fondo de cada tema
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute('content', theme === 'dark' ? '#101113' : '#e7e9ec')
  }, [theme])

  useEffect(() => {
    if (stage !== 'loading') return
    const t = window.setTimeout(() => setStage('signin'), 900)
    return () => window.clearTimeout(t)
  }, [stage])

  useEffect(() => {
    const onHash = () => setNav(navTo(viewFromHash()))
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  }, [])

  const select = (id: string) => {
    if (location.hash !== `#${id}`) history.pushState(null, '', `#${id}`)
    setNav(navTo(id))
    window.scrollTo({ top: 0 })
  }

  const tab = (MAIN_TABS as readonly string[]).indexOf(view)
  useTabSwipe(
    (step) => {
      const next = MAIN_TABS[tab + step]
      if (next) select(next)
    },
    stage === 'app' && tab >= 0,
  )

  return (
    <div data-theme={theme} className="relative isolate min-h-svh text-foreground">
      <HalftoneWallpaper light={theme === 'light'} />
      <div className="relative z-10">
        {stage === 'loading' && <LoadingScreen />}
        {stage === 'signin' && (
          <SignIn
            onSignIn={() => {
              setStage('app')
              setPortal(true)
            }}
          />
        )}
        {stage === 'app' && (
          <GlassShell
            views={VIEWS}
            home={HOME_VIEW}
            tabs={TABS}
            active={view}
            heading={
              view === NOT_FOUND
                ? {
                    label: 'Página no encontrada',
                    description: 'Esa dirección no existe en el prototipo.',
                  }
                : undefined
            }
            onSelect={select}
            theme={theme}
            onThemeChange={setTheme}
            glass={glass}
            onGlassChange={setGlass}
            notices={NOTICES}
            user={CURRENT_MEMBER}
          >
            <div key={view} data-dir={dir} className="view-enter">
              {view === 'performance' && <PerformanceView onOpen={select} />}
              {view === 'panel' && <PanelView onOpen={select} />}
              {view === 'propuestas' && <ProposalsView member={CURRENT_MEMBER} />}
              {view === 'terminal' && <TerminalView member={CURRENT_MEMBER} />}
              {view === 'graficos' && <ChartsView />}
              {view === 'multigrafico' && <MultichartView />}
              {view === 'liquidez' && <LiquidityView />}
              {view === 'noticias' && <NewsView />}
              {view === 'operaciones' && <JournalView />}
              {view === 'fondo' && <FundView />}
              {view === 'competencia' && <CompetitionView />}
              {view === 'glosario' && <GlossaryView />}
              {view === NOT_FOUND && <NotFound onHome={() => select('panel')} />}
            </div>
          </GlassShell>
        )}
      </div>
      {portal && <PortalTransition onDone={() => setPortal(false)} />}
    </div>
  )
}
