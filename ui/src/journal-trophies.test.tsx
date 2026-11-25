import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { PROCESS_BADGES, TRADES } from '@/mocks'
import { JournalView } from '@/views/journal'
import { CompetitionView } from '@/views/trophies'

describe('Diario en línea de tiempo (#581, etapa 8)', () => {
  it('agrupa por día UTC y cada entrada tiene hora, motivo, resultado del motor y nota', () => {
    const { container } = render(<JournalView />)
    expect(container.querySelector('[data-section="journal"]')).not.toBeNull()
    expect(screen.getByRole('note')).toHaveTextContent('EJEMPLO')
    const log = screen.getByRole('region', { name: 'Diario de operaciones' })
    const days = new Set(TRADES.map((t) => t.at.slice(0, 10)))
    expect(within(log).getAllByRole('list')).toHaveLength(days.size)
    expect(within(log).getAllByRole('listitem')).toHaveLength(TRADES.length)
    const loss = TRADES.find((t) => t.id === 'p-0118')!
    const entry = within(log).getByRole('heading', { name: /US500/ }).closest('li')!
    expect(entry.querySelector('time')).toHaveAttribute('dateTime', loss.at)
    expect(entry).toHaveTextContent('16:30 UTC')
    expect(entry).toHaveTextContent('−18,40 € · perdida')
    expect(entry).toHaveTextContent(loss.reason!)
    expect(entry).toHaveTextContent(loss.note!)
    expect(screen.getByText(/Resultados del pasado, no promesas/)).toBeInTheDocument()
  })
})

describe('Trofeos de proceso (#581, etapa 8)', () => {
  it('premian reglas cumplidas con estados, nunca la ganancia', () => {
    const { container } = render(<CompetitionView />)
    expect(container.querySelector('[data-section="trophies"]')).not.toBeNull()
    expect(screen.getByRole('note')).toHaveTextContent('EJEMPLO')
    const badges = within(screen.getByRole('list', { name: 'Trofeos' })).getAllByRole('listitem')
    expect(badges).toHaveLength(PROCESS_BADGES.length)
    expect(badges.map((b) => b.dataset.state)).toEqual(PROCESS_BADGES.map((b) => b.state))
    // sin resultados en dinero ni en R, sin puestos
    const text = container.textContent ?? ''
    expect(text).not.toMatch(/€|\d R\b|ganador|ganancia (segura|garantizada)/i)
  })
})
