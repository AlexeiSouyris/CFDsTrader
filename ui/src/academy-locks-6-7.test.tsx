/**
 * Candados de las herramientas de los niveles 6–7 de la Academia (#836): las capas del menú
 * Herramientas, el ticket de paper y la barra de estado de la operación. Lo que no se esconde
 * nunca: el gráfico de precio y los avisos de seguridad (kill switch, compuerta, reconciliación).
 * Los ids y los nombres son los de `config/academy.toml`; el progreso es **de prueba**.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { OpsStatusBar } from '@/components/ui/ops-status'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import { TOOLS_KEY } from '@/lib/analysis-layers'
import { chartBars } from '@/lib/liquidity-bars'
import type { OpsStatus } from '@/lib/ops-api'
import { CURRENT_MEMBER, LIQUIDITY_US100 as DATA } from '@/mocks'
import { TerminalView } from '@/views/terminal'

/** [id, nombre, lección que la abre] */
const TOOLS = [
  ['tool.atr', 'ATR', '6.1'],
  ['tool.moving_averages', 'EMA, SMA y cruces de medias', '6.2'],
  ['tool.fibonacci', 'Fibonacci y golden pocket', '6.4'],
  ['tool.floor_pivots', 'Puntos pivote diarios', '6.4'],
  ['tool.spring_upthrust', 'Spring y upthrust', '6.4'],
  ['tool.bottoms', 'Precios de fondo', '6.4'],
  ['tool.vix_regime', 'Régimen del VIX', '6.4'],
  ['tool.paper_ticket', 'Ticket de paper (preview y órdenes simuladas)', '7.3'],
  [
    'tool.trading_status_bar',
    'Barra de estado de la operación (kill switch y pérdida del día)',
    '7.4',
  ],
] as const
const IDS = new Set<string>(TOOLS.map(([id]) => id))

const baseCatalog = catalogFixture as AcademyCatalog
const CATALOG: AcademyCatalog = {
  ...baseCatalog,
  tools: [
    ...baseCatalog.tools.filter((t) => !IDS.has(t.id)),
    ...TOOLS.map(([id, name, lesson_id]) => ({ id, name, lesson_id })),
  ],
}

const progress = (open: boolean): AcademyProgress => ({
  ...(progressFixture as AcademyProgress),
  tools: [
    ...(progressFixture as AcademyProgress).tools.filter((t) => !IDS.has(t.tool_id)),
    ...TOOLS.map(([tool_id, , lesson_id]) =>
      open
        ? { tool_id, unlocked: true as const, by: 'override' as const, lesson_id }
        : { tool_id, unlocked: false as const, by: null, lesson_id },
    ),
  ],
})

/** Gateway de prueba: la Academia y, si se pasa, el estado de la operación; lo demás no existe. */
function gateway(p: AcademyProgress | null, ops: OpsStatus | null = null) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const path = new URL(String(input), 'http://ui.test').pathname
      if (p !== null && path === '/api/v1/academy/catalog') return Response.json(CATALOG)
      if (p !== null && path === '/api/v1/academy/progress') return Response.json(p)
      if (ops !== null && path === '/api/v1/ops/status') return Response.json(ops)
      return new Response('sin ruta', { status: 404 })
    }),
  )
}

const academyCalls = () =>
  vi.mocked(fetch).mock.calls.filter(([input]) => String(input).includes('/api/v1/academy/')).length

beforeEach(() => resetAcademy())
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('capas del menú Herramientas (#836)', () => {
  const draw = () =>
    render(
      <LiquidityChart
        instrument="US100"
        bars={chartBars(DATA)}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
        analysis={DATA.analysis}
      />,
    )
  /** prendidas en el navegador (el ejemplo de US100 trae tramos de Fibonacci y fondos) */
  const SAVED = { ema: true, fibonacci: true, bottoms: true, vix: true }
  const menu = () => {
    fireEvent.click(screen.getByRole('button', { name: /Herramientas/ }))
    return screen.getByRole('dialog', { name: 'Herramientas de análisis' })
  }

  it('bloqueadas: no se dibujan aunque estén prendidas, y cada una lleva a su lección', async () => {
    gateway(progress(false))
    localStorage.setItem(TOOLS_KEY, JSON.stringify(SAVED))
    const { container } = draw()
    const m = menu()
    const ema = await within(m).findByRole('link', {
      name: /^EMA diaria: se desbloquea en la lección 6\.2/,
    })
    expect(ema).toHaveAttribute('href', '#academia')
    for (const [label, lesson] of [
      ['Cruce dorado / de la muerte', '6\\.2'],
      ['Fibonacci y golden pocket', '6\\.4'],
      ['Spring y upthrust', '6\\.4'],
      ['Precio de fondo', '6\\.4'],
      ['Puntos pivote diarios', '6\\.4'],
      ['Régimen del VIX', '6\\.4'],
    ])
      expect(
        within(m).getByRole('link', {
          name: new RegExp(`^${label}: se desbloquea en la lección ${lesson}`),
        }),
      ).toBeInTheDocument()
    expect(within(m).queryAllByRole('checkbox')).toHaveLength(0)
    // el gráfico de precio sigue; sus capas, no
    expect(screen.getByRole('img', { name: /US100/ })).toBeVisible()
    expect(container.querySelector('[data-fib-leg]')).toBeNull()
    expect(container.querySelector('[data-marker]')).toBeNull()
    expect(screen.getByRole('button', { name: /Herramientas/ }).textContent).toBe('Herramientas')
    // la preferencia guardada no se toca: al abrirse vuelve como estaba
    expect(JSON.parse(localStorage.getItem(TOOLS_KEY)!)).toEqual(SAVED)
  })

  it('abiertas (por su lección o a mano), las casillas y las capas como siempre', async () => {
    gateway(progress(true))
    localStorage.setItem(TOOLS_KEY, JSON.stringify(SAVED))
    const { container } = draw()
    await vi.waitFor(() => expect(academyCalls()).toBe(2))
    await new Promise((r) => setTimeout(r, 0))
    const m = menu()
    expect(within(m).queryByRole('link')).toBeNull()
    expect(within(m).getByRole('checkbox', { name: 'Fibonacci y golden pocket' })).toBeChecked()
    expect(within(m).getByRole('checkbox', { name: 'Spring y upthrust' })).not.toBeChecked()
    expect(container.querySelector('[data-fib-leg]')).not.toBeNull()
    expect(container.querySelector('[data-marker="bottom"]')).not.toBeNull()
  })

  it('sin el motor no se bloquea nada', async () => {
    gateway(null)
    localStorage.setItem(TOOLS_KEY, JSON.stringify(SAVED))
    const { container } = draw()
    await vi.waitFor(() => expect(academyCalls()).toBe(2))
    await new Promise((r) => setTimeout(r, 0))
    expect(within(menu()).getAllByRole('checkbox')).toHaveLength(7)
    expect(container.querySelector('[data-fib-leg]')).not.toBeNull()
  })
})

describe('capas desde la paleta de comandos (#836)', () => {
  afterEach(() => history.replaceState(null, '', '#'))

  it('bloqueada, la capa lleva a su lección en vez de prenderse', async () => {
    gateway(progress(false))
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: 'k', metaKey: true })
    fireEvent.change(screen.getByRole('combobox', { name: 'Buscar' }), {
      target: { value: 'ema' },
    })
    const option = await screen.findByRole('option', { name: /^EMA diaria \(con candado\)/ })
    expect(option).toHaveTextContent('Se desbloquea en la lección 6.2')
    expect(screen.queryByRole('option', { name: /^(Apagar|Prender) EMA diaria/ })).toBeNull()
    fireEvent.click(option)
    expect(location.hash).toBe('#academia')
    expect(localStorage.getItem(TOOLS_KEY)).toBeNull()
  })
})

/** Estado de prueba con la forma de `ops_status.v1` (números del motor, no de la UI). */
const READY: OpsStatus = {
  as_of: '2026-11-23T10:00:00Z',
  mode: 'paper',
  gate: 'ready',
  accepting_orders: true,
  account: {
    kill_switch: { active: false },
    daily_loss_used_pct: '40.0',
    daily_loss_limit_pct: '3',
    leverage: '1.35',
    open_positions: 1,
    max_positions: 5,
  },
  last_reconcile: {
    kind: 'startup',
    at: '2026-11-23T07:00:00Z',
    clean: true,
    matched: 3,
    mismatches: 0,
    drifts: 0,
  },
}

describe('barra de estado de la operación (#836)', () => {
  const bar = () => screen.getByRole('region', { name: 'Estado de la operación' })
  const lessonLink = () => screen.queryByRole('link', { name: 'Ir a la lección' })

  it('bloqueada, sigue mostrando la compuerta y el kill switch; lo demás, en la lección 7.4', async () => {
    gateway(progress(false))
    render(<OpsStatusBar status={READY} academyLock />)
    expect(await screen.findByRole('link', { name: 'Ir a la lección' })).toHaveAttribute(
      'href',
      '#academia',
    )
    expect(bar()).toHaveTextContent('Acepta órdenes')
    expect(bar()).toHaveTextContent('Kill switch inactivo')
    expect(bar()).toHaveTextContent(/se desbloquea en la lección 7\.4/)
    expect(within(bar()).queryByRole('meter')).toBeNull()
    expect(bar()).not.toHaveTextContent('Apalancamiento')
  })

  it.each([
    [
      'el kill switch activo',
      {
        ...READY,
        accepting_orders: false,
        account: {
          ...READY.account!,
          kill_switch: { active: true, reason: 'Pérdida máxima diaria superada.', since: 'x' },
        },
      },
    ],
    ['el broker sin aceptar órdenes', { ...READY, gate: 'starting', accepting_orders: false }],
    ['la cuenta sin datos', { ...READY, account: null }],
    [
      'una reconciliación con diferencias',
      { ...READY, last_reconcile: { ...READY.last_reconcile!, clean: false, mismatches: 1 } },
    ],
  ] as [string, OpsStatus][])('con %s se muestra entera aunque esté bloqueada', async (_, s) => {
    gateway(progress(false))
    render(<OpsStatusBar status={s} academyLock />)
    await vi.waitFor(() => expect(academyCalls()).toBe(2))
    await new Promise((r) => setTimeout(r, 0))
    expect(lessonLink()).toBeNull()
    expect(bar()).not.toHaveAttribute('data-tool-lock')
  })

  it('fuera de la Terminal (el Panel) y abierta, entera', async () => {
    gateway(progress(false))
    const { unmount } = render(<OpsStatusBar status={READY} />)
    await vi.waitFor(() => expect(academyCalls()).toBe(2))
    await new Promise((r) => setTimeout(r, 0))
    expect(within(bar()).getByRole('meter')).toBeInTheDocument()
    unmount()
    resetAcademy()
    gateway(progress(true))
    render(<OpsStatusBar status={READY} academyLock />)
    await vi.waitFor(() => expect(academyCalls()).toBe(2))
    await new Promise((r) => setTimeout(r, 0))
    expect(within(bar()).getByRole('meter')).toBeInTheDocument()
    expect(lessonLink()).toBeNull()
  })
})

describe('Terminal (#836)', () => {
  it('el ticket de paper se abre en la lección 7.3; el aviso de simulación y la barra siguen', async () => {
    gateway(progress(false), READY)
    render(<TerminalView member={CURRENT_MEMBER} />)
    expect(
      await screen.findByRole('group', {
        name: 'Ticket de paper (preview y órdenes simuladas), bloqueada',
      }),
    ).toBeVisible()
    expect(await screen.findByRole('region', { name: 'Estado de la operación' })).toHaveTextContent(
      'Kill switch inactivo',
    )
    expect(screen.getByText(/se desbloquea en la lección 7\.4/)).toBeInTheDocument()
  })

  it('abierto, el ticket como siempre', async () => {
    gateway(progress(true), READY)
    render(<TerminalView member={CURRENT_MEMBER} />)
    await vi.waitFor(() => expect(academyCalls()).toBe(2))
    await new Promise((r) => setTimeout(r, 0))
    expect(screen.queryByRole('group', { name: /^Ticket de paper/ })).toBeNull()
    expect(screen.getByRole('region', { name: 'Estado de la operación' })).not.toHaveAttribute(
      'data-tool-lock',
    )
  })
})
