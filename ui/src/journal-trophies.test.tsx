import { render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { JournalEntry, JournalPage } from '@/lib/journal-api'
import { TROPHY_RULES } from '@/lib/trophy-rules'
import { JournalView } from '@/views/journal'
import { CompetitionView } from '@/views/trophies'

/** Un gateway falso de prueba (no son datos reales) que responde `/api/v1/journal`. */
function gateway(reply: () => Response) {
  const fetch = vi.fn(async (url: string) =>
    url === '/api/v1/journal' ? reply() : new Response('ruta desconocida', { status: 404 }),
  )
  vi.stubGlobal('fetch', fetch)
  return fetch
}

const page = (entries: JournalEntry[], limit = 100): JournalPage => ({ entries, limit })

/** Dos operaciones de paper del 27-11 y una del 26-11, como las manda el motor. */
function entries(): JournalEntry[] {
  const proposal = (id: string, signal_at: string, side: 'long' | 'short') => ({
    id,
    source: 'paper' as const,
    run_id: 'paper-runner',
    instrument_id: 'US500',
    side,
    signal_at,
    reference: '6712.4',
    stop: side === 'short' ? '6730.4' : '6694.4',
    tp1: side === 'short' ? '6676.4' : '6748.4',
    size: '0.5',
    leverage: '20',
    score: 4,
    breakdown: [{ component: 'trend' as const, points: 2 }],
    triggers: ['level_rejection' as const],
  })
  return [
    { proposal: proposal('c', '2026-11-27T19:00:00Z', 'long'), outcome: null },
    {
      proposal: proposal('b', '2026-11-27T16:30:00Z', 'short'),
      outcome: {
        proposal_id: 'b',
        kind: 'simulated',
        entry_at: '2026-11-27T16:45:00Z',
        entry_price: '6712.1',
        exits: [{ at: '2026-11-27T18:00:00Z', price: '6730.7', fraction: '1', reason: 'stop' }],
        commission: '0',
        funding: '0',
        gross_pnl: '-9.30',
        net_pnl: '-9.30',
        hit_tp1: false,
        hit_stop: true,
        exit_reason: 'stop',
      },
    },
    { proposal: proposal('a', '2026-11-26T10:15:00Z', 'long'), outcome: null },
  ]
}

describe('Diario con el diario de paper del motor (#616)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('agrupa por día UTC de la señal y muestra cada entrada tal como llega', async () => {
    const fetch = gateway(() => Response.json(page(entries())))
    const { container } = render(<JournalView />)
    expect(container.querySelector('[data-section="journal"]')).not.toBeNull()
    const log = screen.getByRole('region', { name: 'Diario de operaciones' })
    await within(log).findAllByRole('list')
    expect(fetch).toHaveBeenCalledWith('/api/v1/journal', expect.anything())
    // sin sello de ejemplo: son datos del motor
    expect(screen.queryByText('EJEMPLO')).toBeNull()
    expect(within(log).getAllByRole('list')).toHaveLength(2)
    expect(within(log).getAllByRole('listitem')).toHaveLength(3)
    const closed = within(log).getAllByRole('listitem')[1]
    expect(closed.querySelector('time')).toHaveAttribute('dateTime', '2026-11-27T16:30:00Z')
    expect(closed).toHaveTextContent('16:30 UTC')
    expect(closed).toHaveTextContent('corto')
    expect(closed).toHaveTextContent('−9,30 · salió por el stop')
    expect(closed).toHaveTextContent('Entrada6.712,1')
    expect(closed).toHaveTextContent('rechazo de un nivel de liquidez (puntaje 4)')
    const open = within(log).getAllByRole('listitem')[0]
    expect(open).toHaveTextContent('todavía sin resultado')
    expect(open).toHaveTextContent('Entrada6.712,4')
    expect(screen.getByText(/Resultados del pasado, no promesas/)).toBeInTheDocument()
    expect(screen.queryByText(/Se muestran las últimas/)).toBeNull()
  })

  it('avisa cuando llega al tope de la página', async () => {
    gateway(() => Response.json(page(entries(), 3)))
    render(<JournalView />)
    expect(await screen.findByText('Se muestran las últimas 3 operaciones.')).toBeInTheDocument()
  })

  it('sin operaciones, un estado vacío honesto y ninguna fila', async () => {
    gateway(() => Response.json(page([])))
    render(<JournalView />)
    expect(
      await screen.findByText(
        /Todavía no hay operaciones en simulación: el runner arranca el 27-11/,
      ),
    ).toHaveAttribute('role', 'status')
    expect(screen.queryAllByRole('listitem')).toHaveLength(0)
    expect(screen.queryByText('EJEMPLO')).toBeNull()
  })

  it('sin gateway, avisa el error y no inventa filas', async () => {
    gateway(() => new Response('ruta desconocida', { status: 404 }))
    render(<JournalView />)
    expect(await screen.findByRole('alert')).toHaveTextContent(
      /No se pudo leer el diario del motor \(el gateway respondió 404\)/,
    )
    expect(screen.queryAllByRole('listitem')).toHaveLength(0)
  })
})

describe('Trofeos de proceso sin datos todavía (#616)', () => {
  it('muestran los previstos sin estado, fecha ni miembro inventado, y nunca la ganancia', () => {
    const { container } = render(<CompetitionView />)
    expect(container.querySelector('[data-section="trophies"]')).not.toBeNull()
    expect(screen.queryByText('EJEMPLO')).toBeNull()
    expect(screen.getByRole('status')).toHaveTextContent(/Todavía no hay trofeos/)
    const badges = within(screen.getByRole('list', { name: 'Trofeos' })).getAllByRole('listitem')
    expect(badges.map((b) => b.dataset.state)).toEqual(TROPHY_RULES.map(() => 'sin datos'))
    expect(badges[0]).toHaveTextContent(TROPHY_RULES[0].rule)
    expect(container.querySelector('time')).toBeNull()
    expect(screen.queryByRole('table')).toBeNull()
    expect(screen.getByText(/nadie operó en simulación/)).toBeInTheDocument()
    const text = container.textContent ?? ''
    expect(text).not.toMatch(/€|\d R\b|ganador|ganancia (segura|garantizada)|logrado el/i)
  })
})
