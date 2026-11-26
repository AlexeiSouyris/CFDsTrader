/**
 * Candados de las herramientas de los niveles 3–5 de la Academia (#827): qué se bloquea y qué no
 * se bloquea nunca (el gráfico de precio). Los ids y los nombres son los de `config/academy.toml`;
 * el progreso es **de prueba** (todo bloqueado o todo abierto a mano).
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { ChartDrawBar } from '@/components/ui/chart-draw-bar'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import { NO_TOOLS } from '@/lib/chart-drawings'
import { chartBars } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA } from '@/mocks'
import { LiquidityContent } from '@/views/liquidity'
import { MultichartView } from '@/views/multichart'
import { ReplayView } from '@/views/replay'

/** [id, nombre, lección que la abre] */
const TOOLS = [
  ['tool.timeframe_selector', 'Selector de timeframe', '3.2'],
  ['tool.multichart', 'Multigráfico', '3.2'],
  ['tool.replay_practice', 'Replay libre', '3.3'],
  ['tool.ruler', 'Regla de medición', '3.3'],
  ['tool.drawing', 'Dibujo: líneas horizontales, de tendencia y rectángulos', '4.2'],
  ['tool.liquidity_map', 'Mapa de liquidez', '5.1'],
  ['tool.fvg_order_blocks', 'Capas de FVG y order blocks', '5.3'],
  ['tool.fvg_stats', 'Estadísticas de FVG', '5.3'],
] as const

const IDS = new Set<string>(TOOLS.map(([id]) => id))

// el catálogo de las pruebas ya trae estos ids (#837): se reemplazan por los de la tabla
const CATALOG: AcademyCatalog = {
  ...(catalogFixture as AcademyCatalog),
  tools: [
    ...(catalogFixture as AcademyCatalog).tools.filter((t) => !IDS.has(t.id)),
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

/** Gateway de prueba: catálogo y progreso de la Academia; lo demás no existe (datos de ejemplo). */
function gateway(p: AcademyProgress | null) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const path = new URL(String(input), 'http://ui.test').pathname
      if (p === null) return new Response('sin gateway', { status: 503 })
      if (path === '/api/v1/academy/catalog') return Response.json(CATALOG)
      if (path === '/api/v1/academy/progress') return Response.json(p)
      return new Response('sin ruta', { status: 404 })
    }),
  )
}

const lockLink = (name: string, lesson: string) =>
  screen.findAllByRole('link', {
    name: new RegExp(`^${name}: se desbloquea en la lección ${lesson.replace('.', '\\.')}`),
  })

beforeEach(() => resetAcademy())
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('barra de dibujo (#827)', () => {
  const bar = () => (
    <ChartDrawBar label="Dibujo" tools={NO_TOOLS} onChange={() => {}} instrument="US100" />
  )

  it('medir y dibujar bloqueados llevan a su lección; navegar, velas y línea siguen', async () => {
    gateway(progress(false))
    render(bar())
    const [ruler] = await lockLink('Regla de medición', '3.3')
    expect(ruler).toHaveAttribute('href', '#academia')
    expect(
      await lockLink('Dibujo: líneas horizontales, de tendencia y rectángulos', '4.2'),
    ).toHaveLength(3)
    const toolbar = screen.getByRole('toolbar', { name: 'Dibujo' })
    expect(within(toolbar).queryByRole('button', { name: 'Medir' })).toBeNull()
    for (const name of ['Navegar', 'Velas', 'Línea'])
      expect(within(toolbar).getByRole('button', { name })).toBeEnabled()
  })

  it('abiertos (por su lección o a mano), los botones tal cual', async () => {
    gateway(progress(true))
    render(bar())
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2))
    expect(screen.getByRole('button', { name: 'Medir' })).toBeEnabled()
    expect(screen.queryByRole('link')).toBeNull()
  })

  it('sin el motor no se bloquea nada', async () => {
    gateway(null)
    render(bar())
    await vi.waitFor(() => expect(fetch).toHaveBeenCalled())
    expect(screen.getByRole('button', { name: 'Medir' })).toBeEnabled()
  })
})

describe('regla bloqueada (#827)', () => {
  /** Shift + arrastrar sobre el gráfico (en jsdom el SVG mide 720 × 440). */
  async function shiftDrag(open: boolean) {
    gateway(progress(open))
    vi.spyOn(SVGElement.prototype, 'getBoundingClientRect').mockReturnValue({
      ...{ left: 0, top: 0, x: 0, y: 0, width: 720, height: 440, right: 720, bottom: 440 },
      toJSON: () => ({}),
    } as DOMRect)
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={chartBars(DATA)}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
      />,
    )
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2))
    // espera a que el estado de la Academia llegue al gráfico
    await (open
      ? screen.findByRole('button', { name: 'Regla: medir arrastrando' })
      : lockLink('Regla de medición', '3.3'))
    const svg = screen.getByRole('img', { name: /US100/ })
    fireEvent.pointerDown(svg, {
      clientX: 100,
      clientY: 200,
      button: 0,
      pointerId: 1,
      shiftKey: true,
    })
    fireEvent.pointerMove(svg, { clientX: 300, clientY: 150, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 300, clientY: 150, pointerId: 1 })
    return container.querySelector('[data-measure]')
  }

  it('Shift + arrastrar no mide con la regla bloqueada', async () => {
    expect(await shiftDrag(false)).toBeNull()
  })

  it('con la regla abierta, Shift + arrastrar mide', async () => {
    expect(await shiftDrag(true)).not.toBeNull()
  })
})

describe('liquidez (#827)', () => {
  it('niveles, FVG y estadísticas bloqueados; el gráfico de precio sigue, sin esas capas', async () => {
    gateway(progress(false))
    render(<LiquidityContent data={DATA} />)
    for (const name of ['Mapa de liquidez', 'Capas de FVG y order blocks', 'Estadísticas de FVG'])
      expect(await screen.findByRole('group', { name: `${name}, bloqueada` })).toBeVisible()
    // el gráfico se ve y se usa, sin niveles ni FVG, y dice por qué
    expect(screen.getByRole('img', { name: /con 0 niveles de liquidez y 0 FVG/ })).toBeVisible()
    expect(
      screen.getByText(/^Mapa de liquidez: se desbloquea en la lección 5\.1\b/),
    ).toBeInTheDocument()
    expect(screen.queryByRole('group', { name: 'Niveles' })).toBeNull()
    // la regla del gráfico también tiene candado
    expect(await lockLink('Regla de medición', '3.3')).toHaveLength(1)
  })

  it('abiertas, las tablas y las capas como siempre', async () => {
    gateway(progress(true))
    render(<LiquidityContent data={DATA} />)
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(2))
    expect(screen.queryByRole('group', { name: /bloqueada$/ })).toBeNull()
    expect(screen.getByRole('img', { name: /con [1-9]\d* niveles de liquidez/ })).toBeVisible()
    expect(screen.getByRole('group', { name: 'Niveles' })).toBeInTheDocument()
  })
})

describe('replay, Multigráfico y timeframe (#827)', () => {
  it('el replay libre se abre en la lección 3.3', async () => {
    gateway(progress(false))
    render(<ReplayView />)
    expect(await screen.findByRole('group', { name: 'Replay libre, bloqueada' })).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Empezar' })).toBeNull()
  })

  it('el Multigráfico se bloquea entero', async () => {
    gateway(progress(false))
    render(<MultichartView />)
    expect(await screen.findByRole('group', { name: 'Multigráfico, bloqueada' })).toBeVisible()
  })

  it('Gráficos (un solo gráfico) nunca se bloquea; el selector de timeframe sí', async () => {
    gateway(progress(false))
    render(<MultichartView single />)
    expect(
      await screen.findByText(/^Selector de timeframe: se desbloquea en la lección 3\.2\b/),
    ).toBeInTheDocument()
    expect(screen.queryByRole('group', { name: 'Multigráfico, bloqueada' })).toBeNull()
    expect(screen.queryByRole('group', { name: 'Timeframe del gráfico activo' })).toBeNull()
    expect(screen.getByRole('img', { name: /US100/ })).toBeVisible()
  })
})
