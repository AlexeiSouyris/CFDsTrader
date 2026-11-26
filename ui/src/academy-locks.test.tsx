/**
 * Candados de la Academia (#771): lo que abre o cierra cada herramienta lo dice el motor
 * (`academy_progress.v1`); sin el motor, nada se bloquea. En Ajustes se abre y se cierra a mano con
 * eventos del motor. Datos: los fixtures del gateway simulado de las pruebas e2e.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { AcademySettings } from '@/components/ui/academy-settings'
import { ToolLock } from '@/components/ui/tool-lock'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import { takeFocusedLesson } from '@/lib/academy-locks'
import { GlossaryView } from '@/views/phase10'

const CATALOG = catalogFixture as AcademyCatalog
const PROGRESS = progressFixture as AcademyProgress

/** Gateway falso de prueba: catálogo, progreso y eventos (anotados en `posts`). */
function gateway(
  progress: AcademyProgress = PROGRESS,
  { down = false, catalog = CATALOG }: { down?: boolean; catalog?: AcademyCatalog } = {},
) {
  const posts: unknown[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      if (down) return new Response('sin gateway', { status: 503 })
      const path = new URL(String(input), 'http://ui.test').pathname
      if (init?.method === 'POST') {
        posts.push([path, JSON.parse(String(init.body))])
        return Response.json({})
      }
      if (path === '/api/v1/academy/catalog') return Response.json(catalog)
      if (path === '/api/v1/academy/progress') return Response.json(progress)
      return new Response('sin ruta', { status: 404 })
    }),
  )
  return posts
}

const tool = (id: string, patch: Partial<AcademyProgress['tools'][number]>): AcademyProgress => ({
  ...PROGRESS,
  tools: PROGRESS.tools.map((t) =>
    t.tool_id === id ? ({ ...t, ...patch } as AcademyProgress['tools'][number]) : t,
  ),
})

const Markets = () => (
  <ToolLock tool="tool.watchlist">
    <button type="button">Mercados</button>
  </ToolLock>
)

beforeEach(() => resetAcademy())
afterEach(() => vi.unstubAllGlobals())

describe('candado de una herramienta (#771)', () => {
  it('bloqueada: se ve gris, no se usa y dice qué lección la abre', async () => {
    gateway()
    render(<Markets />)
    const lock = await screen.findByRole('group', { name: 'Watchlist, bloqueada' })
    expect(lock).toHaveTextContent('Se desbloquea en la lección 0.1 · Qué es un mercado.')
    expect(lock).toHaveTextContent('También se abre a mano en Ajustes.')
    // la herramienta queda fuera del lector y del foco
    expect(screen.queryByRole('button', { name: 'Mercados' })).toBeNull()
    const link = within(lock).getByRole('link', { name: 'Ir a la lección' })
    expect(link).toHaveAttribute('href', '#academia')
    fireEvent.click(link)
    expect(takeFocusedLesson()).toBe('0.1')
  })

  it('abierta por su lección o a mano: la herramienta tal cual', async () => {
    gateway(tool('tool.watchlist', { unlocked: true, by: 'override' }))
    render(<Markets />)
    expect(await screen.findByRole('button', { name: 'Mercados' })).toBeEnabled()
    expect(screen.queryByRole('group')).toBeNull()
  })

  it('sin el motor no se bloquea nada (es una ayuda, no un control de seguridad)', async () => {
    gateway(PROGRESS, { down: true })
    render(<Markets />)
    expect(screen.getByRole('button', { name: 'Mercados' })).toBeEnabled()
    await vi.waitFor(() => expect(fetch).toHaveBeenCalled())
    expect(screen.getByRole('button', { name: 'Mercados' })).toBeEnabled()
  })

  it('con locked_tools_visible = false se esconde', async () => {
    gateway(PROGRESS, {
      catalog: { ...CATALOG, thresholds: { ...CATALOG.thresholds, locked_tools_visible: false } },
    })
    const { container } = render(<Markets />)
    await vi.waitFor(() => expect(container).toBeEmptyDOMElement())
  })

  it('un control chico se reemplaza por una línea', async () => {
    gateway()
    render(
      <ToolLock tool="tool.source_selector" compact>
        <button type="button">Fuente</button>
      </ToolLock>,
    )
    expect(
      await screen.findByText(
        'Selector de fuente (CFD / futuros): se desbloquea en la lección 1.1 · Contado, futuros y CFD.',
      ),
    ).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Fuente' })).toBeNull()
  })

  it('el glosario lleva su candado', async () => {
    gateway()
    render(<GlossaryView />)
    expect(
      await screen.findByRole('group', { name: 'Glosario interactivo, bloqueada' }),
    ).toBeVisible()
    expect(screen.queryByRole('searchbox', { name: 'Buscar en el glosario' })).toBeNull()
  })
})

describe('Ajustes → Academia (#771)', () => {
  it('desbloquear todo y una por una son eventos del motor', async () => {
    const posts = gateway(tool('tool.glossary', { unlocked: true, by: 'lesson' }))
    render(<AcademySettings />)
    const list = await screen.findByRole('list', { name: 'Herramientas con candado' })
    expect(list).toHaveTextContent('Watchlistbloqueada · lección 0.1')
    expect(list).toHaveTextContent('Glosario interactivoabierta por la lección 0.2')
    // abierta por su lección: nada que hacer a mano
    expect(within(list).queryByRole('button', { name: /Glosario/ })).toBeNull()
    fireEvent.click(within(list).getByRole('button', { name: 'Abrir Watchlist' }))
    await vi.waitFor(() => expect(posts).toHaveLength(1))
    // mientras el motor guarda, los botones esperan
    await vi.waitFor(() =>
      expect(screen.getByRole('button', { name: 'Desbloquear todo' })).toBeEnabled(),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Desbloquear todo' }))
    await vi.waitFor(() => expect(posts).toHaveLength(2))
    expect(posts).toEqual([
      [
        '/api/v1/academy/events',
        { kind: 'tool_override', tool_id: 'tool.watchlist', action: 'unlock' },
      ],
      ['/api/v1/academy/events', { kind: 'tool_override', tool_id: '*', action: 'unlock' }],
    ])
  })

  it('con todo abierto a mano ofrece volver a bloquear', async () => {
    const all: AcademyProgress = {
      ...PROGRESS,
      unlock_all: true,
      tools: PROGRESS.tools.map((t) => ({ ...t, unlocked: true, by: 'override' })),
    }
    const posts = gateway(all)
    render(<AcademySettings />)
    const list = await screen.findByRole('list', { name: 'Herramientas con candado' })
    expect(list).toHaveTextContent('Watchlistabierta a mano')
    expect(within(list).queryAllByRole('button')).toHaveLength(0)
    fireEvent.click(screen.getByRole('button', { name: 'Volver a bloquear todo' }))
    await vi.waitFor(() =>
      expect(posts).toEqual([
        ['/api/v1/academy/events', { kind: 'tool_override', tool_id: '*', action: 'relock' }],
      ]),
    )
  })

  it('sin el motor lo dice', async () => {
    gateway(PROGRESS, { down: true })
    render(<AcademySettings />)
    expect(
      await screen.findByText('Sin conexión con el motor: los candados no se aplican.'),
    ).toBeInTheDocument()
  })
})
