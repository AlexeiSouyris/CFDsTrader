/**
 * Paleta de comandos (#581, etapa 4): el buscador del encabezado convertido en paleta con
 * resultados agrupados (vistas, instrumentos, timeframe, capas, tema y partes del Inicio).
 */
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { TOOLS_KEY } from '@/lib/analysis-layers'

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
afterEach(() => {
  vi.unstubAllGlobals()
  history.replaceState(null, '', '#')
})

/** Abre la paleta con el atajo de teclado. */
function openPalette() {
  fireEvent.keyDown(window, { key: 'k', metaKey: true })
  return screen.getByRole('dialog', { name: 'Paleta de comandos' })
}
const input = () => screen.getByRole('combobox', { name: 'Buscar' })
const type = (q: string) => fireEvent.change(input(), { target: { value: q } })

describe('paleta de comandos (#581)', () => {
  it('⌘K la abre; los resultados van en grupos dentro del listbox', async () => {
    render(<App initialStage="app" />)
    openPalette()
    const list = screen.getByRole('listbox', { name: 'Resultados' })
    await within(list).findByRole('group', { name: 'Instrumentos' })
    const groups = within(list)
      .getAllByRole('group')
      .map((g) => g.getAttribute('data-group'))
    expect(groups).toEqual([
      'Vistas',
      'Instrumentos',
      'Timeframe',
      'Capas',
      'Tema',
      'Ajustes',
      'Inicio',
    ])
    expect(within(list).getByRole('group', { name: 'Timeframe' })).toContainElement(
      screen.getByRole('option', { name: /^Timeframe 1h/ }),
    )
    // con una búsqueda, solo los grupos con algo
    type('tema')
    expect(
      within(list)
        .getAllByRole('group')
        .map((g) => g.getAttribute('data-group')),
    ).toEqual(['Tema'])
  })

  it('el timeframe cambia el gráfico activo del Terminal', async () => {
    history.replaceState(null, '', '#terminal')
    render(<App initialStage="app" />)
    const tf = () => screen.getByRole('group', { name: 'Timeframe del gráfico 1' })
    expect(within(tf()).getByRole('button', { name: '15m' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    openPalette()
    type('1h')
    await screen.findByRole('option', { name: /^Timeframe 1h/ })
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(location.hash).toBe('#terminal')
    expect(within(tf()).getByRole('button', { name: '1h' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('desde otra vista, el timeframe abre el Terminal con ese timeframe', async () => {
    history.replaceState(null, '', '#panel')
    render(<App initialStage="app" />)
    openPalette()
    type('timeframe 1d')
    fireEvent.click(await screen.findByRole('option', { name: /^Timeframe 1d/ }))
    expect(location.hash).toBe('#terminal')
    const tf = await screen.findByRole('group', { name: 'Timeframe del gráfico 1' })
    expect(within(tf).getByRole('button', { name: '1d' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('prende y apaga capas de análisis (la preferencia compartida de los gráficos)', () => {
    render(<App initialStage="app" />)
    openPalette()
    type('ema')
    // la EMA diaria arranca prendida
    fireEvent.click(screen.getByRole('option', { name: /^Apagar EMA diaria/ }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(JSON.parse(localStorage.getItem(TOOLS_KEY)!)).toMatchObject({ ema: false })
    openPalette()
    type('ema')
    expect(screen.getByRole('option', { name: /^Prender EMA diaria/ })).toBeInTheDocument()
  })

  it('cambia el tema', () => {
    const { container } = render(<App initialStage="app" />)
    const root = container.firstElementChild!
    const before = root.getAttribute('data-theme')
    openPalette()
    type('tema')
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(root.getAttribute('data-theme')).not.toBe(before)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('salta a una parte del Inicio y le pasa el foco', async () => {
    history.replaceState(null, '', '#panel')
    render(<App initialStage="app" />)
    openPalette()
    // la vista Inicio también coincide (su descripción), pero se marca la parte que se nombró
    type('inicio resultados')
    expect(screen.getByRole('option', { name: /^Inicio: Resultados recientes/ })).toHaveAttribute(
      'aria-selected',
      'true',
    )
    fireEvent.keyDown(input(), { key: 'Enter' })
    expect(location.hash).toBe('#inicio')
    await act(() => new Promise((r) => setTimeout(r, 120)))
    expect(document.activeElement).toHaveAttribute('id', 'inicio-resultados')
  })

  it('sin coincidencias lo dice', () => {
    render(<App initialStage="app" />)
    openPalette()
    type('zzzz')
    expect(
      screen.getByText('No hay vistas, instrumentos ni comandos con ese nombre.'),
    ).toBeVisible()
  })
})
