/**
 * La página de la lección (#770) y el examen de preguntas (#781): las respuestas van al motor y la
 * nota, el umbral y el aprobado vuelven de él. Las preguntas de acá son **de prueba** (el catálogo real todavía
 * no tiene contenido), con la forma de `academy_catalog.v1`.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import docFixture from '../e2e/fixtures/academy-lesson-doc.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress, QuizResult } from '@/lib/academy-contracts'
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

const RESULT: QuizResult = {
  lesson_id: '2.2',
  catalog_version: CATALOG.catalog_version,
  score_pct: '50',
  pass_pct: '80',
  passed: false,
  correct: 1,
  total: 2,
  questions: [
    { question_id: '2.2.1', correct: true },
    { question_id: '2.2.2', correct: false },
  ],
  graded_at: '2026-11-26T05:00:00Z',
}

/** Gateway falso de prueba: catálogo, progreso y los POST de la Academia. */
function gateway(post: (path: string, body: unknown) => Response) {
  const fetch = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
    const path = new URL(String(input), 'http://ui.test').pathname
    if (init?.method === 'POST') return post(path, JSON.parse(String(init.body)))
    if (path === '/api/v1/academy/catalog') return Response.json(CATALOG)
    if (path === '/api/v1/academy/progress') return Response.json(PROGRESS)
    // el texto de cualquier lección (#799): el de la 0.1, que es el que hay de prueba
    if (/^\/api\/v1\/academy\/lessons\/[\d.]+\/doc$/.test(path)) return Response.json(docFixture)
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

describe('examen de preguntas (#781)', () => {
  it('manda las respuestas al motor y muestra su corrección, sin decir la clave', async () => {
    let sent: unknown = null
    gateway((path, body) => {
      sent = [path, body]
      return Response.json(RESULT)
    })
    render(<AcademyView />)
    const page = await open(/Financiación nocturna/)
    expect(page).toHaveTextContent('2 preguntas; para aprobar, 80 % de respuestas correctas')
    const form = within(page).getByRole('form', { name: 'Examen de la lección 2.2' })
    fireEvent.click(within(form).getByRole('radio', { name: 'Opción B' }))
    fireEvent.change(within(form).getByRole('textbox', { name: /Respuesta/ }), {
      target: { value: '1,17' },
    })
    fireEvent.click(within(form).getByRole('button', { name: 'Enviar al motor' }))
    expect(await within(form).findByText('Todavía no')).toBeInTheDocument()
    expect(sent).toEqual([
      '/api/v1/academy/quiz/2.2',
      {
        lesson_id: '2.2',
        catalog_version: CATALOG.catalog_version,
        answers: [
          { question_id: '2.2.1', choice: 'b' },
          { question_id: '2.2.2', value: '1.17' },
        ],
      },
    ])
    expect(form).toHaveTextContent('1 de 2 bien · 50 % (para aprobar, 80 %)')
    expect(form).toHaveTextContent('El intento quedó guardado.')
    expect(within(form).getByText('Bien, según el motor')).toBeInTheDocument()
    expect(within(form).getByText('Mal, según el motor')).toBeInTheDocument()
    expect(within(form).getByRole('button', { name: 'Enviar otro intento' })).toBeEnabled()
  })

  it('un número mal escrito no se manda; las vacías avisan que cuentan como incorrectas', async () => {
    const post = vi.fn(() => Response.json(RESULT))
    gateway(post)
    render(<AcademyView />)
    const form = within(await open(/Financiación nocturna/)).getByRole('form')
    expect(form).toHaveTextContent('2 sin responder: cuentan como incorrectas.')
    const field = within(form).getByRole('textbox', { name: /Respuesta/ })
    fireEvent.change(field, { target: { value: 'uno' } })
    expect(field).toHaveAttribute('aria-invalid', 'true')
    expect(within(form).getByRole('alert')).toHaveTextContent('Escribí un número')
    fireEvent.submit(form)
    expect(post).not.toHaveBeenCalled()
  })

  it('catálogo viejo (409): lo dice y recarga', async () => {
    const fetch = gateway(() => new Response('catálogo viejo', { status: 409 }))
    render(<AcademyView />)
    const form = within(await open(/Financiación nocturna/)).getByRole('form')
    fireEvent.click(within(form).getByRole('radio', { name: 'Opción A' }))
    fireEvent.click(within(form).getByRole('button', { name: 'Enviar al motor' }))
    expect(await within(form).findByRole('alert')).toHaveTextContent(
      'El examen cambió en el motor mientras respondías',
    )
    await vi.waitFor(() =>
      expect(fetch.mock.calls.filter(([u]) => u === '/api/v1/academy/catalog')).toHaveLength(2),
    )
  })
})
