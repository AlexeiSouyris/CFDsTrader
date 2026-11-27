/**
 * Candados de las herramientas de los niveles 8–10 de la Academia (#846): cada uno tapa solo su
 * parte. En el Laboratorio, el constructor y el simulador de varianza por separado (la calculadora
 * de riesgo queda abierta; el simulador de cuenta fondeada tiene el suyo, del nivel 12, #851); en
 * Performance, la lista de corridas; en los informes, el PBO con el Sharpe deflactado y el
 * walk-forward; en el Inicio, el registro de resultados; en la Terminal, la pestaña Noticias.
 *
 * Catálogo y progreso: los fixtures e2e (el catálogo real, todo bloqueado); «abierto» los abre a
 * mano, como desde Ajustes.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { LabReportView } from '@/components/ui/lab-report'
import { RunDetailView } from '@/components/ui/run-detail'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import type { LabReport, StrategyCatalog } from '@/lib/lab-contracts'
import { BACKTEST_RUN_DETAIL, BACKTEST_RUNS, CURRENT_MEMBER } from '@/mocks'
import labCatalogJson from '@/mocks/lab-catalog.json'
import labReportJson from '@/mocks/lab-report.json'
import { RecentResults } from '@/views/home-results'
import { LabView } from '@/views/lab'
import { PerformanceView } from '@/views/performance'
import { TerminalView } from '@/views/terminal'

const CATALOG = catalogFixture as AcademyCatalog
const LAB_CATALOG = labCatalogJson as unknown as StrategyCatalog
const REPORT = labReportJson as unknown as LabReport

/** Los ids de los niveles 8–10, tal como están en `config/academy.toml`. */
const IDS = [
  'tool.variance_simulator',
  'tool.backtest_runs',
  'tool.pbo',
  'tool.walk_forward',
  'tool.results_registry',
  'tool.lab_builder',
  'tool.terminal_news',
]
const nameOf = (id: string) => CATALOG.tools.find((t) => t.id === id)!.name

/** Todo bloqueado (el fixture) o los ids de 8–10 abiertos a mano. */
const progress = (open: boolean): AcademyProgress => {
  const base = progressFixture as AcademyProgress
  if (!open) return base
  return {
    ...base,
    tools: base.tools.map((t) =>
      IDS.includes(t.tool_id) ? { ...t, unlocked: true as const, by: 'override' as const } : t,
    ),
  }
}

/** Gateway de prueba: la Academia (si se pasa) y el catálogo del laboratorio; lo demás no existe. */
function gateway(p: AcademyProgress | null) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const path = new URL(String(input), 'http://ui.test').pathname
      if (p !== null && path === '/api/v1/academy/catalog') return Response.json(CATALOG)
      if (p !== null && path === '/api/v1/academy/progress') return Response.json(p)
      if (path === '/api/v1/lab/catalog') return Response.json(LAB_CATALOG)
      return new Response('sin ruta', { status: 404 })
    }),
  )
}

/** El candado de una herramienta: su grupo, con el enlace a la lección que la abre. */
async function findLock(id: string, lesson: string) {
  const group = await screen.findByRole('group', { name: `${nameOf(id)}, bloqueada` })
  expect(group).toHaveTextContent(`Se desbloquea en la lección ${lesson}`)
  expect(within(group).getByRole('link', { name: 'Ir a la lección' })).toHaveAttribute(
    'href',
    '#academia',
  )
  return group
}

beforeEach(() => resetAcademy())
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('Laboratorio (#846)', () => {
  it('bloqueado: el constructor y el simulador de varianza, cada uno con su candado', async () => {
    gateway(progress(false))
    render(<LabView />)
    await findLock('tool.lab_builder', '10.3 · Exploratorio y confirmatorio en el Laboratorio')
    await findLock('tool.variance_simulator', '8.3 · Monte Carlo')
    // el formulario queda gris y fuera del lector; sin constructor no se invita a correr
    expect(screen.queryByRole('form', { name: 'Constructor de estrategias' })).toBeNull()
    expect(screen.queryByText(/Sin corrida todavía/)).toBeNull()
    expect(screen.queryByRole('region', { name: 'Simulador de varianza' })).toBeNull()
    // la calculadora de riesgo no tiene candado (el simulador de cuenta fondeada, el suyo: #851)
    expect(screen.getByRole('region', { name: 'Calculadora de riesgo' })).toBeVisible()
  })

  it('abierto: el constructor y el simulador como hoy', async () => {
    gateway(progress(true))
    render(<LabView />)
    expect(await screen.findByRole('form', { name: 'Constructor de estrategias' })).toBeVisible()
    expect(screen.getByText(/Sin corrida todavía/)).toBeVisible()
    expect(screen.getByRole('region', { name: 'Simulador de varianza' })).toBeVisible()
    // el único candado que queda es el del simulador de cuenta fondeada (nivel 12, #851); la
    // Academia llega aparte (con los nombres del catálogo, #861, el orden de las respuestas cambia)
    await waitFor(() =>
      expect(
        [...document.querySelectorAll('[data-tool-lock]')].map((e) =>
          e.getAttribute('data-tool-lock'),
        ),
      ).toEqual(['tool.propsim']),
    )
  })

  it('sin la Academia del motor no se bloquea nada', async () => {
    gateway(null)
    render(<LabView />)
    expect(await screen.findByRole('form', { name: 'Constructor de estrategias' })).toBeVisible()
    expect(screen.getByRole('region', { name: 'Simulador de varianza' })).toBeVisible()
    expect(document.querySelector('[data-tool-lock]')).toBeNull()
  })
})

describe('informe del laboratorio (#846)', () => {
  it('bloqueado: el PBO y el Sharpe deflactado en una línea, y el walk-forward gris', async () => {
    gateway(progress(false))
    render(<LabReportView report={REPORT} />)
    await findLock('tool.walk_forward', '9.4 · Walk-forward y fuera de muestra')
    expect(screen.queryByRole('table', { name: 'Ventanas walk-forward del informe' })).toBeNull()
    const pbo = document.querySelector('[data-tool-lock="tool.pbo"]')!
    expect(pbo).toHaveTextContent(`${nameOf('tool.pbo')}: se desbloquea en la lección 9.3`)
    expect(within(pbo as HTMLElement).getByRole('link', { name: 'Ir a la lección' })).toBeVisible()
    // lo demás del bloque y del informe sigue: el Sharpe, el Sortino y el aviso del fuera de muestra
    const block = within(
      screen.getByRole('heading', { name: 'Cuánto se probó' }).closest('section')!,
    )
    expect(block.queryByText('Sharpe deflactado')).toBeNull()
    expect(block.queryByText('PBO')).toBeNull()
    expect(block.getByText('Sharpe mensual')).toBeVisible()
    expect(block.getByText('Sortino mensual')).toBeVisible()
    expect(screen.getByText(/no se cargó ni se evaluó/)).toBeVisible()
  })

  it('abierto: el informe entero', async () => {
    gateway(progress(true))
    render(<LabReportView report={REPORT} />)
    expect(
      await screen.findByRole('table', { name: 'Ventanas walk-forward del informe' }),
    ).toBeVisible()
    expect(screen.getByText('Sharpe deflactado')).toBeVisible()
    expect(screen.getByText('PBO')).toBeVisible()
  })
})

describe('Performance (#846)', () => {
  it('bloqueada: la lista de corridas con su candado; lo demás de la vista sigue', async () => {
    gateway(progress(false))
    render(<PerformanceView onOpen={() => {}} />)
    await findLock('tool.backtest_runs', '9.1 · Mirar el futuro y repintar')
    expect(screen.queryByRole('table', { name: 'Corridas de backtest' })).toBeNull()
    expect(
      screen.queryByRole('button', { name: `Ver el detalle de ${BACKTEST_RUNS[0].run_id}` }),
    ).toBeNull()
    expect(screen.getByRole('heading', { name: 'Estadísticas de la última corrida' })).toBeVisible()
  })

  it('abierta: la lista abre el detalle de una corrida', async () => {
    gateway(progress(true))
    render(<PerformanceView onOpen={() => {}} />)
    const id = BACKTEST_RUN_DETAIL.run.run_id
    fireEvent.click(await screen.findByRole('button', { name: `Ver el detalle de ${id}` }))
    expect(await screen.findByRole('region', { name: `Detalle de la corrida ${id}` })).toBeVisible()
  })

  it('el detalle de una corrida: el walk-forward con su candado, el resto a la vista', async () => {
    gateway(progress(false))
    render(<RunDetailView detail={BACKTEST_RUN_DETAIL} example onClose={() => {}} />)
    await findLock('tool.walk_forward', '9.4')
    expect(screen.getByRole('heading', { name: 'Walk-forward / fuera de muestra' })).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Kill switch' })).toBeVisible()
    expect(screen.getByRole('heading', { name: 'Operaciones' })).toBeVisible()
  })
})

describe('registro de resultados del Inicio (#846)', () => {
  const draw = () =>
    render(<RecentResults runs={BACKTEST_RUNS} runsLive={false} onOpen={() => {}} />)

  it('bloqueado: la lista del registro con su candado; las corridas del motor siguen', async () => {
    gateway(progress(false))
    draw()
    await findLock('tool.results_registry', '9.5 · Casos reales del registro')
    expect(screen.queryByRole('list', { name: 'Últimas pruebas del registro' })).toBeNull()
    expect(screen.getByRole('list', { name: 'Últimas corridas del motor' })).toBeVisible()
  })

  it('abierto: la lista como hoy', async () => {
    gateway(progress(true))
    draw()
    expect(await screen.findByRole('list', { name: 'Últimas pruebas del registro' })).toBeVisible()
    expect(document.querySelector('[data-tool-lock]')).toBeNull()
  })
})

describe('pestaña Noticias de la Terminal (#846)', () => {
  it('bloqueada: la pestaña se elige y muestra su candado; las demás pestañas siguen', async () => {
    gateway(progress(false))
    render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(screen.getByRole('tab', { name: /Noticias/ }))
    const panel = screen.getByRole('tabpanel')
    await findLock('tool.terminal_news', '10.6 · Noticias y macro')
    expect(within(panel).queryByRole('heading', { name: 'Últimas noticias' })).toBeNull()
    fireEvent.click(screen.getByRole('tab', { name: /Actividad/ }))
    expect(
      within(screen.getByRole('tabpanel')).queryByRole('group', { name: /bloqueada/ }),
    ).toBeNull()
  })

  it('abierta: las noticias como hoy', async () => {
    gateway(progress(true))
    render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(screen.getByRole('tab', { name: /Noticias/ }))
    expect(
      await within(screen.getByRole('tabpanel')).findByRole('heading', {
        name: 'Últimas noticias',
      }),
    ).toBeVisible()
    expect(screen.getByRole('tabpanel').querySelector('[data-tool-lock]')).toBeNull()
  })
})
