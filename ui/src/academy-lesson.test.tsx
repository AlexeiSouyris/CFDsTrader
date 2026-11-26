/**
 * La página de la lección (#770). Las preguntas de acá son **de prueba** (el catálogo real todavía
 * no tiene contenido), con la forma de `academy_catalog.v1`.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import { AcademyView } from '@/views/academy'

const PROGRESS = progressFixture as AcademyProgress
/** El catálogo real con dos preguntas de prueba en la lección 2.2. */
const CATALOG: AcademyCatalog = {
  ...(catalogFixture as AcademyCatalog),
  levels: (catalogFixture as AcademyCatalog).levels.map((lv) => ({
    ...lv,
    lessons: lv.lessons.map((l) =>
      l.id === '2.2'
        ? {
            ...l,
            exam: {
              kind: 'quiz' as const,
              ready: true,
              questions: [
                {
                  id: '2.2.1',
                  kind: 'choice' as const,
                  prompt: 'Pregunta de prueba de opción',
                  options: [
                    { id: 'a', text: 'Opción A' },
                    { id: 'b', text: 'Opción B' },
                  ],
                  unit: null,
                },
                {
                  id: '2.2.2',
                  kind: 'number' as const,
                  prompt: 'Pregunta de prueba numérica',
                  options: null,
                  unit: 'pts',
                },
              ],
            },
          }
        : l,
    ),
  })),
}

/** Gateway falso de prueba: catálogo, progreso y los POST de la Academia. */
function gateway(post: (path: string, body: unknown) => Response) {
  const fetch = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
    const path = new URL(String(input), 'http://ui.test').pathname
    if (init?.method === 'POST') return post(path, JSON.parse(String(init.body)))
    if (path === '/api/v1/academy/catalog') return Response.json(CATALOG)
    if (path === '/api/v1/academy/progress') return Response.json(PROGRESS)
    return new Response('sin ruta', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

const open = async (name: RegExp) => {
  const map = await screen.findByRole('navigation', { name: 'Mapa de niveles' })
  fireEvent.click(within(map).getByRole('button', { name }))
  return screen.getByRole('article', { name })
}

beforeEach(() => resetAcademy())
afterEach(() => vi.unstubAllGlobals())

describe('página de la lección (#770)', () => {
  it('elegir una lección la abre y lleva el foco a su título', async () => {
    gateway(() => new Response('', { status: 500 }))
    render(<AcademyView />)
    const page = await open(/Cómo se usa la Academia/)
    const map = screen.getByRole('navigation', { name: 'Mapa de niveles' })
    expect(within(map).getByRole('button', { name: /0\.3/ })).toHaveAttribute(
      'aria-current',
      'true',
    )
    expect(within(page).getByRole('heading', { level: 2 })).toHaveFocus()
    expect(page).toHaveTextContent('No abre ninguna herramienta.')
  })

  it('un examen con preguntas dice cuántas y el umbral del motor', async () => {
    gateway(() => new Response('', { status: 500 }))
    render(<AcademyView />)
    const page = await open(/Financiación nocturna/)
    expect(page).toHaveTextContent(
      '2 preguntas; para aprobar, 80 % de respuestas correctas (umbral provisorio). Corrige el motor.',
    )
  })

  it('una lección sin examen se marca completada con un evento del motor', async () => {
    const posts: unknown[] = []
    const fetch = gateway((path, body) => {
      posts.push([path, body])
      return Response.json({})
    })
    render(<AcademyView />)
    const page = await open(/Cómo se usa la Academia/)
    fireEvent.click(within(page).getByRole('button', { name: 'Marcar como completada' }))
    await vi.waitFor(() => expect(posts).toHaveLength(1))
    expect(posts[0]).toEqual([
      '/api/v1/academy/events',
      { kind: 'lesson_completed', lesson_id: '0.3' },
    ])
    // después vuelve a pedir el progreso (el motor decide)
    await vi.waitFor(() =>
      expect(fetch.mock.calls.filter(([u]) => u === '/api/v1/academy/progress')).toHaveLength(2),
    )
  })
})

describe('lección completada (#770)', () => {
  it('si el motor no la guarda, lo dice y deja reintentar', async () => {
    gateway(() => new Response('sin token', { status: 403 }))
    render(<AcademyView />)
    const page = await open(/Cómo se usa la Academia/)
    fireEvent.click(within(page).getByRole('button', { name: 'Marcar como completada' }))
    expect(await within(page).findByRole('alert')).toHaveTextContent(
      'No se pudo guardar en el motor (el gateway respondió 403: sin token)',
    )
    expect(within(page).getByRole('button', { name: 'Marcar como completada' })).toBeEnabled()
  })
})
