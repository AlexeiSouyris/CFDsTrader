/**
 * Academia (#769): el mapa de niveles y el progreso, tal como los manda el motor. Los datos son los
 * fixtures del gateway simulado de las pruebas e2e (el catálogo de `config/academy.toml`, sin
 * preguntas todavía).
 */
import { render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { reloadAcademy, resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import { AcademyView } from '@/views/academy'

const CATALOG = catalogFixture as AcademyCatalog
const PROGRESS = progressFixture as AcademyProgress

/** Gateway falso de prueba: catálogo y progreso, o 503 si está caído. */
function gateway(progress: AcademyProgress = PROGRESS, down = false) {
  const fetch = vi.fn(async (input: string | URL | Request) => {
    if (down) return new Response('sin gateway', { status: 503 })
    const path = new URL(String(input), 'http://ui.test').pathname
    if (path === '/api/v1/academy/catalog') return Response.json(CATALOG)
    if (path === '/api/v1/academy/progress') return Response.json(progress)
    return new Response('sin ruta', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

beforeEach(() => resetAcademy())
afterEach(() => vi.unstubAllGlobals())

describe('Academia (#769)', () => {
  it('mapa de niveles con cada lección y el progreso del motor', async () => {
    const fetch = gateway()
    render(<AcademyView />)
    const map = await screen.findByRole('region', { name: 'Mapa de niveles' })
    expect(fetch).toHaveBeenCalledWith('/api/v1/academy/catalog', expect.anything())
    expect(fetch).toHaveBeenCalledWith('/api/v1/academy/progress', expect.anything())
    // tres niveles (0 a 2) y sus diez lecciones, en el orden del catálogo
    expect(within(map).getAllByRole('heading', { level: 2 })).toHaveLength(3)
    expect(within(map).getByRole('heading', { name: /N0.*Orientación.*0\/3/ })).toBeVisible()
    const rows = within(map)
      .getAllByRole('listitem')
      .filter((li) => li.querySelector('h2') === null)
    expect(rows).toHaveLength(10)
    expect(rows[0]).toHaveTextContent('0.1Qué es un mercadoEn preparación')
    expect(rows[2]).toHaveTextContent('0.3Cómo se usa la AcademiaSin examen')
    const strip = screen.getByRole('region', { name: 'Progreso' })
    expect(strip).toHaveTextContent('Lecciones aprobadas0 / 10')
    expect(strip).toHaveTextContent('Herramientas abiertas0 / 6')
    expect(strip).toHaveTextContent('80 % de respuestasprovisorio')
  })

  it('sin contenido todavía: «Contenido en preparación», sin inventar lecciones', async () => {
    gateway()
    render(<AcademyView />)
    const note = await screen.findByRole('note')
    expect(note).toHaveTextContent('Contenido en preparación')
    expect(note).toHaveTextContent('Las lecciones todavía no tienen texto ni preguntas')
  })

  it('muestra lo aprobado, los intentos y lo abierto a mano tal como lo calculó el motor', async () => {
    const progress: AcademyProgress = {
      ...PROGRESS,
      lessons: PROGRESS.lessons.map((l) =>
        l.lesson_id === '0.1'
          ? { ...l, passed: true, passed_at: '2026-11-24T10:00:00Z', attempts: 2 }
          : l.lesson_id === '0.2'
            ? { ...l, attempts: 1, best_score_pct: '60' }
            : l,
      ),
      tools: PROGRESS.tools.map((t) =>
        t.tool_id === 'tool.watchlist' ? { ...t, unlocked: true, by: 'override' } : t,
      ),
      unlock_all: true,
    }
    gateway(progress)
    render(<AcademyView />)
    const map = await screen.findByRole('region', { name: 'Mapa de niveles' })
    expect(map).toHaveTextContent('0.1Qué es un mercadoAprobada el 24 nov, 10:00 UTC')
    expect(map).toHaveTextContent('0.2Cuántos pierden1 intento · mejor 60 %')
    expect(within(map).getByRole('heading', { name: /N0.*1\/3/ })).toBeVisible()
    const strip = screen.getByRole('region', { name: 'Progreso' })
    expect(strip).toHaveTextContent('Lecciones aprobadas1 / 10')
    expect(strip).toHaveTextContent('Herramientas abiertas1 / 6(todas a mano)')
  })

  it('sin gateway lo dice, sin lecciones inventadas', async () => {
    gateway(PROGRESS, true)
    render(<AcademyView />)
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'No se pudo leer la Academia del motor (el gateway respondió 503: sin gateway)',
    )
    expect(screen.queryByRole('region', { name: 'Mapa de niveles' })).toBeNull()
  })

  it('si el gateway falla después, se quedan los últimos datos del motor', async () => {
    const fetch = gateway()
    render(<AcademyView />)
    await screen.findByRole('region', { name: 'Mapa de niveles' })
    fetch.mockImplementation(async () => new Response('sin gateway', { status: 503 }))
    await reloadAcademy()
    expect(screen.getByRole('region', { name: 'Mapa de niveles' })).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
  })
})
