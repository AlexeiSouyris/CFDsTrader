import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'

/** Un gateway falso: solo responde el catálogo; lo demás falla (y la app usa los ejemplos). */
const INSTRUMENTS = {
  default_instrument: 'DE40',
  default_timeframe: '15m',
  timeframes: ['15m', '1h', '1d'],
  instruments: [
    {
      id: 'DE40',
      name: 'Alemania 40',
      asset_class: 'index',
      currency: 'EUR',
      exchange_timezone: 'Europe/Berlin',
    },
    {
      id: 'US100',
      name: 'EEUU Tech 100',
      asset_class: 'index',
      currency: 'EUR',
      exchange_timezone: 'America/New_York',
    },
  ],
}

beforeEach(() => {
  localStorage.clear()
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const url = new URL(String(input), 'http://localhost')
      return url.pathname === '/api/v1/instruments'
        ? new Response(JSON.stringify(INSTRUMENTS), { status: 200 })
        : new Response('sin gateway', { status: 503 })
    }),
  )
})
afterEach(() => vi.unstubAllGlobals())

const openSearch = () => {
  render(<App initialStage="app" />)
  fireEvent.click(screen.getByRole('button', { name: /Buscar o elegir un comando/ }))
  return screen.getByRole('dialog', { name: 'Paleta de comandos' })
}
const input = () => screen.getByRole('combobox', { name: 'Buscar' })
const options = () => within(screen.getByRole('listbox')).getAllByRole('option')

describe('buscador del encabezado (#362)', () => {
  // va primero: después el catálogo queda recordado y se ve al instante
  it('Enter antes de que llegue el catálogo elige apenas llega', async () => {
    openSearch()
    fireEvent.change(input(), { target: { value: 'alemania' } })
    expect(within(screen.getByRole('dialog')).queryAllByRole('option')).toHaveLength(0)
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(await screen.findByRole('group', { name: 'Instrumento del gráfico 1' })).toBeVisible()
    expect(location.hash).toBe('#terminal')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(
      within(screen.getByRole('group', { name: 'Instrumento del gráfico 1' })).getByRole('button', {
        name: 'DE40',
      }),
    ).toHaveAttribute('aria-pressed', 'true')
  })

  it('un clic en una opción navega y cierra', () => {
    openSearch()
    fireEvent.change(input(), { target: { value: 'liquidez' } })
    fireEvent.click(screen.getByRole('option', { name: /Liquidez/ }))
    expect(location.hash).toBe('#liquidez')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('pointerdown + pointerup en la misma opción navega aunque el click no llegue', () => {
    openSearch()
    const opt = screen.getByRole('option', { name: /^Terminal/ })
    fireEvent.pointerDown(opt, { button: 0 })
    fireEvent.pointerUp(opt, { button: 0 })
    expect(location.hash).toBe('#terminal')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('elegir la vista que ya está abierta también cierra', () => {
    history.replaceState(null, '', '#panel')
    openSearch()
    fireEvent.click(screen.getByRole('option', { name: /^Panel/ }))
    expect(location.hash).toBe('#panel')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('Enter elige la primera opción', () => {
    openSearch()
    fireEvent.change(input(), { target: { value: 'diario' } })
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(location.hash).toBe('#operaciones')
  })

  it('↑/↓ recorren la lista con aria-activedescendant y Enter elige la marcada', () => {
    openSearch()
    const first = options()[0]
    expect(input()).toHaveAttribute('aria-activedescendant', first.id)
    expect(first).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(input(), { key: 'ArrowDown' })
    const second = options()[1]
    expect(input()).toHaveAttribute('aria-activedescendant', second.id)
    expect(second).toHaveAttribute('aria-selected', 'true')
    // ↑ desde la primera vuelve a la última
    fireEvent.keyDown(input(), { key: 'ArrowUp' })
    fireEvent.keyDown(input(), { key: 'ArrowUp' })
    const all = options()
    expect(input()).toHaveAttribute('aria-activedescendant', all[all.length - 1].id)
    fireEvent.keyDown(input(), { key: 'ArrowDown' })
    fireEvent.keyDown(input(), { key: 'ArrowDown' })
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(location.hash).toBe('#terminal')
  })

  it('Esc cierra sin navegar', () => {
    openSearch()
    fireEvent.keyDown(input(), { key: 'Escape' })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(location.hash).toBe('')
  })

  it('un clic en el relleno del diálogo no lo cierra; fuera de su rectángulo sí', () => {
    const dialog = openSearch()
    dialog.getBoundingClientRect = () =>
      ({ left: 100, right: 500, top: 100, bottom: 400, width: 400, height: 300 }) as DOMRect
    fireEvent.click(dialog, { clientX: 105, clientY: 105 })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    fireEvent.click(dialog, { clientX: 20, clientY: 20 })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('busca instrumentos del catálogo y elegir uno abre el Terminal con ese gráfico', async () => {
    openSearch()
    fireEvent.change(input(), { target: { value: 'alemania' } })
    const opt = await screen.findByRole('option', { name: /Alemania 40 \(DE40\)/ })
    expect(options()).toHaveLength(1)
    fireEvent.click(opt)
    expect(location.hash).toBe('#terminal')
    const group = await screen.findByRole('group', { name: 'Instrumento del gráfico 1' })
    expect(within(group).getByRole('button', { name: 'Alemania 40' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('con el Terminal ya abierto, elegir un instrumento cambia el primer gráfico', async () => {
    history.replaceState(null, '', '#terminal')
    openSearch()
    expect(
      within(screen.getByRole('group', { name: 'Instrumento del gráfico 1' })).getByRole('button', {
        name: 'US100',
      }),
    ).toHaveAttribute('aria-pressed', 'true')
    fireEvent.change(input(), { target: { value: 'DE40' } })
    await screen.findByRole('option', { name: /\(DE40\)$/ })
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    const group = screen.getByRole('group', { name: 'Instrumento del gráfico 1' })
    expect(within(group).getByRole('button', { name: 'Alemania 40' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('la página 404 abre el buscador', () => {
    history.replaceState(null, '', '#no-existe')
    render(<App initialStage="app" />)
    act(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Buscar una vista o un instrumento' }))
    })
    expect(screen.getByRole('dialog', { name: 'Paleta de comandos' })).toBeVisible()
  })
})
