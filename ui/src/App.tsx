/**
 * Prototipo de la interfaz (#164, sigue al #30): carga → ingreso → armazón con las vistas.
 * Todo con datos de ejemplo: no se conecta al motor ni a IG y no calcula nada (CLAUDE.md §1).
 */
import { useEffect, useState } from 'react'

import { LoadingScreen } from '@/components/ui/dot-loader'
import { DEFAULT_GLASS } from '@/components/ui/glass-settings'
import { GlassShell } from '@/components/ui/glass-shell'
import { HalftoneWallpaper } from '@/components/ui/halftone-wallpaper'
import { NotFound } from '@/components/ui/not-found'
import { SignIn } from '@/components/ui/sign-in'
import { CURRENT_MEMBER, NOTICES } from '@/mocks'
import { ChartsView, CompetitionView, JournalView, NewsView, OverviewView } from '@/views'
import { LiquidityView } from '@/views/liquidity'
import { VIEWS } from '@/views/nav'
import { FundView, GlossaryView, ProposalsView } from '@/views/phase10'

const NOT_FOUND = 'no-encontrada'
const viewFromHash = () => {
  const id = typeof location === 'undefined' ? '' : location.hash.slice(1)
  if (!id) return VIEWS[0].id
  return VIEWS.some((v) => v.id === id) ? id : NOT_FOUND
}

type Stage = 'loading' | 'signin' | 'app'

export default function App({ initialStage = 'loading' as Stage }: { initialStage?: Stage }) {
  const [stage, setStage] = useState<Stage>(initialStage)
  const [view, setView] = useState(viewFromHash)
  const [theme, setTheme] = useState<'dark' | 'light'>('dark')
  const [glass, setGlass] = useState(DEFAULT_GLASS)

  useEffect(() => {
    document.documentElement.dataset.theme = theme
  }, [theme])

  useEffect(() => {
    if (stage !== 'loading') return
    const t = window.setTimeout(() => setStage('signin'), 900)
    return () => window.clearTimeout(t)
  }, [stage])

  useEffect(() => {
    const onHash = () => setView(viewFromHash())
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  }, [])

  const select = (id: string) => {
    if (location.hash !== `#${id}`) history.pushState(null, '', `#${id}`)
    setView(id)
    window.scrollTo({ top: 0 })
  }

  return (
    <div data-theme={theme} className="relative isolate min-h-svh text-foreground">
      <HalftoneWallpaper light={theme === 'light'} />
      <div className="relative z-10">
        {stage === 'loading' && <LoadingScreen />}
        {stage === 'signin' && <SignIn onSignIn={() => setStage('app')} />}
        {stage === 'app' && (
          <GlassShell
            views={VIEWS}
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
            {view === 'panel' && <OverviewView onOpen={select} />}
            {view === 'propuestas' && <ProposalsView member={CURRENT_MEMBER} />}
            {view === 'graficos' && <ChartsView />}
            {view === 'liquidez' && <LiquidityView />}
            {view === 'noticias' && <NewsView />}
            {view === 'operaciones' && <JournalView />}
            {view === 'fondo' && <FundView />}
            {view === 'competencia' && <CompetitionView />}
            {view === 'glosario' && <GlossaryView />}
            {view === NOT_FOUND && (
              <NotFound onHome={() => select('panel')} onExplore={() => select('graficos')} />
            )}
          </GlassShell>
        )}
      </div>
    </div>
  )
}
