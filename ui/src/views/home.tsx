/**
 * Inicio (#581; espacio de trabajo desde #708, docs/design/2026-11-terminal-pro.md): la vista de
 * entrada como una terminal de varios paneles. Arriba, la cuenta de simulación tal como la manda el
 * motor; después, los mercados (la misma lista de la Terminal), las noticias y el calendario (los
 * mismos de la pestaña Noticias de la Terminal), los resultados de investigación (registro y
 * corridas del motor, tal como llegan) y las secciones con su atajo. Todo con las rutas que ya
 * existen. El selector de estrategia solo filtra lo que se ve. La UI no calcula nada (CLAUDE.md §1)
 * y ningún texto promete ganancias.
 */
import { useEffect } from 'react'

import { ExampleBadge, ExampleNotice } from '@/components/ui/data-honesty'
import { SectionDoor } from '@/components/ui/section-door'
import { TerminalNews } from '@/components/ui/terminal-news'
import { AccountStrip, KillSwitchBanner } from '@/components/ui/trading-terminal'
import { Watchlist } from '@/components/ui/watchlist'
import { WorkspacePanel } from '@/components/ui/workspace-panel'
import { useChartCatalog } from '@/lib/chart-slots'
import { decimal } from '@/lib/format'
import { useInstruments } from '@/lib/liquidity-api'
import { ibkrSummaries, useMarketSummaries } from '@/lib/market-summary'
import { focusInstrument } from '@/lib/terminal-focus'
import { currencySign, useFund } from '@/lib/fund-api'
import { useBacktestRuns, usePaperAccount } from '@/lib/paper-api'
import { dayYear } from '@/lib/run-format'
import { BACKTEST_RUNS, PAPER_ACCOUNT } from '@/mocks'
import { DOORS } from '@/views/home-doors'
import { RecentResults } from '@/views/home-results'
import { FKEYS } from '@/views/nav'

/** Un atajo de una tecla no debe saltar mientras se escribe o se usa un control. */
const typing = (t: EventTarget | null) =>
  t instanceof HTMLElement &&
  (t.isContentEditable || ['INPUT', 'SELECT', 'TEXTAREA'].includes(t.tagName))

export function HomeView({ onOpen }: { onOpen: (view: string) => void }) {
  const remoteAccount = usePaperAccount()
  const live = remoteAccount.status === 'ok'
  const account = live ? remoteAccount.data : PAPER_ACCOUNT
  const remoteRuns = useBacktestRuns()
  const runsLive = remoteRuns.status === 'ok'
  const runs = runsLive ? remoteRuns.data : BACKTEST_RUNS
  const fund = useFund()

  // 1 a 7 abren cada sección (solo en el Inicio y nunca mientras se escribe)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || typing(e.target)) return
      const door = DOORS.find((d) => d.shortcut === e.key)
      if (door) onOpen(door.view)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onOpen])

  const lastRun = runs[0]
  const datum: Record<string, string | undefined> = {
    terminal: live
      ? `${account.open_positions} de ${account.max_positions} posiciones abiertas`
      : undefined,
    performance:
      runsLive && lastRun ? `última corrida: ${dayYear(lastRun.finished_at)}` : undefined,
    fondo: fund.live
      ? `unidad: ${decimal(fund.summary.nav, 4)} ${currencySign(fund.summary.currency)}`
      : undefined,
  }

  // los mercados: la misma lista y las mismas fuentes que la Terminal (#552, #556)
  const { instruments } = useChartCatalog()
  const catalog = useInstruments()
  const names: Record<string, string> =
    catalog.status === 'ok'
      ? Object.fromEntries(catalog.data.instruments.map((i) => [i.id, i.name]))
      : {}
  const summaries = useMarketSummaries(instruments)
  const ibkr = useMarketSummaries([], ibkrSummaries)
  // elegir un mercado abre la Terminal con su gráfico (los de IBKR, que son solo datos, sin elegir)
  const pickMarket = (id: string, feed?: 'ibkr') => {
    if (feed !== 'ibkr') focusInstrument(id)
    onOpen('terminal')
  }
  const fullView = (label: string, view: string) => ({
    label,
    view,
    hotkey: FKEYS[view],
    onOpen,
  })

  return (
    <div data-section="hub" className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
      <section
        id="inicio-cuenta"
        aria-label="Cuenta"
        className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-2"
      >
        {!live && (
          <ExampleNotice>
            Sin conexión con el motor: la cuenta es de ejemplo. Con el gateway, sale del broker
            simulado.
          </ExampleNotice>
        )}
        <AccountStrip account={account} aside={!live && <ExampleBadge />} />
        <KillSwitchBanner account={account} />
      </section>

      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-[minmax(0,4fr)_minmax(0,8fr)]">
        <div id="inicio-mercados" className="grid min-w-0 content-start gap-1">
          <Watchlist
            groups={[
              {
                id: 'ibkr',
                title: 'IBKR',
                sourceLabel: 'solo datos, no operable',
                rows: ibkr.rows,
                feed: 'ibkr',
                badge: 'IBKR',
                loading: ibkr.status === 'loading',
                note:
                  ibkr.status === 'error'
                    ? 'Sin conexión con el gateway: no hay datos de IBKR.'
                    : ibkr.status === 'ok' && ibkr.rows.length === 0
                      ? 'El gateway no tiene instrumentos de IBKR configurados.'
                      : undefined,
              },
              {
                id: 'broker',
                title: 'Broker simulado',
                sourceLabel: summaries.label,
                rows: summaries.rows,
                loading: summaries.status === 'loading',
              },
            ]}
            names={names}
            selected=""
            onSelect={pickMarket}
          />
          <p className="px-1 text-[11px] text-muted-foreground">
            Precios del pasado, no promesas. Elegir uno abre la Terminal.
          </p>
        </div>
        <WorkspacePanel
          id="inicio-noticias"
          title="Noticias y calendario"
          open={fullView('Noticias', 'noticias')}
        >
          <TerminalNews />
        </WorkspacePanel>
      </div>

      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-[minmax(0,8fr)_minmax(0,4fr)]">
        <RecentResults runs={runs} runsLive={runsLive} onOpen={onOpen} />
        <WorkspacePanel id="inicio-secciones" title="Secciones" className="content-start">
          <nav aria-label="Secciones del inicio" className="-mx-1 grid">
            {DOORS.map((d) => (
              <SectionDoor key={d.view} door={d} datum={datum[d.view]} onOpen={onOpen} />
            ))}
          </nav>
          <p className="mt-1 px-1 text-[11px] text-muted-foreground">
            <kbd>1</kbd>–<kbd>7</kbd> desde el Inicio · <kbd>F1</kbd>–<kbd>F9</kbd> desde cualquier
            vista
          </p>
        </WorkspacePanel>
      </div>
    </div>
  )
}
