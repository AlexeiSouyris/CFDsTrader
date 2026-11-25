/**
 * Inicio (#581, etapa 2 de docs/design/2026-11-rediseno-ui.md): la vista de entrada y la única con
 * vidrio. Arriba, la cuenta de simulación tal como la manda el motor; en el centro, las puertas a
 * cada sección; abajo, los resultados recientes del registro y de las corridas del motor, tal como
 * llegan. El selector de estrategia solo filtra lo que se ve: la estrategia la elige y la valida el
 * motor. La UI no calcula nada (CLAUDE.md §1) y ningún texto promete ganancias.
 */
import { useEffect } from 'react'

import { ExampleBadge, ExampleNotice } from '@/components/ui/data-honesty'
import { SectionDoor } from '@/components/ui/section-door'
import { AccountStrip, KillSwitchBanner } from '@/components/ui/trading-terminal'
import { decimal } from '@/lib/format'
import { currencySign, useFund } from '@/lib/fund-api'
import { useBacktestRuns, usePaperAccount } from '@/lib/paper-api'
import { dayYear } from '@/lib/run-format'
import { BACKTEST_RUNS, PAPER_ACCOUNT } from '@/mocks'
import { DOORS } from '@/views/home-doors'
import { RecentResults } from '@/views/home-results'

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

  return (
    <div data-section="hub" className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-8">
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

      <section aria-labelledby="inicio-secciones" className="grid min-w-0 gap-3">
        <h2
          id="inicio-secciones"
          className="px-1 text-[clamp(20px,2vw,26px)] font-bold tracking-[-0.03em]"
        >
          Secciones
        </h2>
        <nav
          aria-label="Secciones del inicio"
          className="grid grid-cols-2 gap-3 lg:auto-rows-[minmax(9rem,auto)] lg:grid-cols-4"
        >
          {DOORS.map((d) => (
            <SectionDoor key={d.view} door={d} datum={datum[d.view]} onOpen={onOpen} />
          ))}
        </nav>
      </section>

      <RecentResults runs={runs} runsLive={runsLive} onOpen={onOpen} />
    </div>
  )
}
