/**
 * Investigación (#581, etapa 6): `#investigacion` reúne Performance, Propuestas, Liquidez y
 * Glosario con un índice lateral, sin romper los hashes de siempre; las tablas se ordenan (solo
 * presentación, con decimales exactos).
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import App from '@/App'
import { compareDecimal, sortRows } from '@/lib/sort-rows'
import { PerformanceView } from '@/views/performance'

afterEach(() => {
  localStorage.clear()
  history.replaceState(null, '', '#')
})

const h1 = () => screen.getByRole('heading', { level: 1 })
const runIds = () =>
  within(screen.getByRole('table', { name: 'Corridas de backtest' }))
    .getAllByRole('button', { name: /Ver el detalle/ })
    .map((b) => b.textContent)

describe('Investigación (#581, etapa 6)', () => {
  it('#investigacion reúne las cuatro partes, con el índice y el estilo editorial', () => {
    history.replaceState(null, '', '#investigacion')
    render(<App initialStage="app" />)
    expect(h1()).toHaveTextContent('Investigación')
    const page = document.querySelector('[data-section="research"]')!
    expect(page).not.toBeNull()
    const titles = [...page.querySelectorAll('h2[data-research-title]')].map((h) => h.textContent)
    expect(titles).toEqual(['Performance', 'Propuestas', 'Liquidez', 'Glosario'])
    const index = screen.getByRole('navigation', { name: 'Índice de Investigación' })
    expect(
      within(index)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual([
      'Corridas',
      'Paper vs backtest',
      'Aprendizaje',
      'Modelo',
      'Propuestas',
      'Liquidez',
      'Glosario',
    ])
    expect(screen.getByRole('table', { name: 'Corridas de backtest' })).toBeInTheDocument()
  })

  it('el índice lleva a cada parte y le pasa el foco, sin cambiar el hash', () => {
    history.replaceState(null, '', '#investigacion')
    render(<App initialStage="app" />)
    const index = screen.getByRole('navigation', { name: 'Índice de Investigación' })
    fireEvent.click(within(index).getByRole('button', { name: 'Glosario' }))
    expect(document.activeElement).toBe(document.getElementById('inv-glosario')!.closest('section'))
    expect(location.hash).toBe('#investigacion')
  })

  it('los hashes de siempre siguen abriendo su vista sola', () => {
    for (const [hash, title] of [
      ['#performance', 'Performance'],
      ['#propuestas', 'Propuestas'],
      ['#liquidez', 'Liquidez'],
      ['#glosario', 'Glosario'],
    ]) {
      history.replaceState(null, '', hash)
      const { unmount } = render(<App initialStage="app" />)
      expect(h1()).toHaveTextContent(title)
      expect(document.querySelector('[data-section="research"]')).toBeNull()
      unmount()
    }
  })

  it('"Abrir sola" abre la vista de siempre', () => {
    history.replaceState(null, '', '#investigacion')
    render(<App initialStage="app" />)
    fireEvent.click(screen.getAllByRole('link', { name: 'Abrir sola →' })[1])
    expect(location.hash).toBe('#propuestas')
    expect(h1()).toHaveTextContent('Propuestas')
  })

  it('la puerta "Investigación" del Inicio lleva a #investigacion', () => {
    render(<App initialStage="app" />)
    const doors = screen.getByRole('navigation', { name: 'Secciones del inicio' })
    fireEvent.click(within(doors).getByRole('link', { name: /Investigación/ }))
    expect(location.hash).toBe('#investigacion')
  })
})

describe('tablas ordenables (#581, etapa 6)', () => {
  it('ordena las corridas por columna: mayor primero, luego menor, luego como llegaron', () => {
    render(<PerformanceView onOpen={() => {}} />)
    const original = runIds()
    const header = screen.getByRole('button', { name: 'Retorno' })
    fireEvent.click(header)
    expect(header.closest('th')).toHaveAttribute('aria-sort', 'descending')
    expect(runIds()).toEqual([
      'ejemplo-2026-11-22-us100-de40',
      'ejemplo-2026-11-20-sin-operaciones',
      'ejemplo-2026-11-21-indices-1h',
    ])
    fireEvent.click(header)
    expect(header.closest('th')).toHaveAttribute('aria-sort', 'ascending')
    expect(runIds()[0]).toBe('ejemplo-2026-11-21-indices-1h')
    fireEvent.click(header)
    expect(header.closest('th')).not.toHaveAttribute('aria-sort')
    expect(runIds()).toEqual(original)
    // el texto va de la A a la Z primero
    fireEvent.click(screen.getByRole('button', { name: 'Corrida' }))
    expect(runIds()[0]).toBe('ejemplo-2026-11-20-sin-operaciones')
  })

  it('compara decimales en texto exactos, sin float', () => {
    const asc = ['12', '-0.5', '0.05', '0', '-12.25', '0.0500001', '-0', '3.10', '3.1']
    expect([...asc].sort(compareDecimal)).toEqual([
      '-12.25',
      '-0.5',
      '0',
      '-0',
      '0.05',
      '0.0500001',
      '3.10',
      '3.1',
      '12',
    ])
    // más allá de la precisión de un float
    expect(compareDecimal('0.10000000000000000001', '0.1')).toBe(1)
    expect(compareDecimal('-0.10000000000000000001', '-0.1')).toBe(-1)
  })

  it('los vacíos van al final en las dos direcciones y el orden es estable', () => {
    const rows = [
      { id: 'a', v: '1' },
      { id: 'b', v: null },
      { id: 'c', v: '2' },
      { id: 'd', v: '1' },
    ]
    const ids = (dir: 'asc' | 'desc') => sortRows(rows, (r) => r.v, dir).map((r) => r.id)
    expect(ids('asc')).toEqual(['a', 'd', 'c', 'b'])
    expect(ids('desc')).toEqual(['c', 'a', 'd', 'b'])
  })
})
