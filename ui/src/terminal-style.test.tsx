/**
 * Terminal con su estilo propio (#581, etapa 3): sección oscura, divisores de paneles accesibles
 * por teclado que recuerdan el ancho y el OHLC de la vela en la cabecera del gráfico.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CURRENT_MEMBER, PAPER_ACCOUNT } from '@/mocks'
import { TerminalView } from '@/views/terminal'

const PANES_KEY = 'cfdstrader.terminal.panes'

/** Datos de prueba con la forma de los contratos (no son precios reales). */
const CANDLE = {
  instrument_id: 'DE40',
  timeframe: '15m',
  start: '2026-11-24T09:45:00Z',
  open: '100.0',
  high: '101.0',
  low: '99.0',
  close: '100.5',
  volume: null,
}

/** Un gateway falso de solo prueba: nada sale al broker simulado real. */
function gateway() {
  const fetch = vi.fn(async (input: string | URL | Request) => {
    const url = String(input)
    const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
    if (url === '/api/v1/instruments')
      return json({
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
      })
    if (url.startsWith('/api/v1/liquidity?'))
      return json({
        instrument_id: new URLSearchParams(url.split('?')[1]).get('instrument'),
        source: { id: 'ig', label: 'IG CFD' },
        timeframe: '15m',
        generated_at: '2026-11-24T10:00:00Z',
        as_of: '2026-11-24T10:00:00Z',
        candles: [CANDLE],
        levels: [],
        fvgs: [],
        imbalance: [],
        stats: [],
      })
    if (url === '/api/v1/paper/account') return json(PAPER_ACCOUNT)
    if (url === '/api/v1/paper/positions') return json([])
    return new Response('', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

const separator = (name: string) => screen.getByRole('separator', { name })

describe('Terminal con estilo propio (#581)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
    localStorage.clear()
  })

  it('es la sección "terminal": oscura y sin vidrio', () => {
    gateway()
    const { container } = render(<TerminalView member={CURRENT_MEMBER} />)
    const root = container.querySelector('[data-terminal]')!
    expect(root).toHaveAttribute('data-section', 'terminal')
    // el acento ámbar va en los controles: la pestaña activa se marca con él, no los datos
    const tab = screen.getByRole('tab', { selected: true })
    expect(tab.className).toContain('border-accent')
  })

  it('los divisores se mueven con el teclado, con límites, y recuerdan el ancho', () => {
    gateway()
    const { unmount } = render(<TerminalView member={CURRENT_MEMBER} />)
    const left = separator('Ancho de la lista de mercados')
    const right = separator('Ancho del ticket')
    expect(left).toHaveAttribute('aria-orientation', 'vertical')
    expect(left).toHaveAttribute('aria-controls', 'terminal-markets')
    expect(left).toHaveAttribute('tabindex', '0')
    expect(left).toHaveAttribute('aria-valuenow', '200')
    expect(right).toHaveAttribute('aria-valuenow', '320')

    fireEvent.keyDown(left, { key: 'ArrowRight' })
    expect(left).toHaveAttribute('aria-valuenow', '216')
    fireEvent.keyDown(left, { key: 'End' })
    expect(left).toHaveAttribute('aria-valuenow', '360')
    fireEvent.keyDown(left, { key: 'ArrowRight' })
    expect(left).toHaveAttribute('aria-valuenow', '360')
    // el ticket está a la derecha: la flecha izquierda lo agranda
    fireEvent.keyDown(right, { key: 'ArrowLeft', shiftKey: true })
    expect(right).toHaveAttribute('aria-valuenow', '384')
    fireEvent.keyDown(right, { key: 'Home' })
    expect(right).toHaveAttribute('aria-valuenow', '272')

    const desk = document.querySelector<HTMLElement>('[data-terminal-desk]')!
    expect(desk.style.getPropertyValue('--w-left')).toBe('360px')
    expect(JSON.parse(localStorage.getItem(PANES_KEY)!)).toEqual({ left: 360, right: 272 })

    unmount()
    render(<TerminalView member={CURRENT_MEMBER} />)
    expect(separator('Ancho de la lista de mercados')).toHaveAttribute('aria-valuenow', '360')
  })

  it('con anchos rotos o fuera de rango, o sin almacenamiento, usa los de siempre', () => {
    gateway()
    localStorage.setItem(PANES_KEY, JSON.stringify({ left: 9999, right: 'ancho' }))
    const { unmount } = render(<TerminalView member={CURRENT_MEMBER} />)
    expect(separator('Ancho de la lista de mercados')).toHaveAttribute('aria-valuenow', '360')
    expect(separator('Ancho del ticket')).toHaveAttribute('aria-valuenow', '320')
    unmount()

    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    render(<TerminalView member={CURRENT_MEMBER} />)
    const left = separator('Ancho de la lista de mercados')
    expect(left).toHaveAttribute('aria-valuenow', '200')
    fireEvent.keyDown(left, { key: 'ArrowLeft' })
    expect(left).toHaveAttribute('aria-valuenow', '184')
  })

  it('la cabecera del gráfico muestra el OHLC de la vela, tal como llega', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    // primero puede verse el ejemplo; cuando llegan las velas del motor, las suyas
    const values = () =>
      [...document.querySelectorAll('[data-chart-ohlc] dd')].map((d) => d.textContent)
    await waitFor(() => expect(values()).toEqual(['100,0', '101,0', '99,0', '100,5']))
    const ohlc = document.querySelector<HTMLElement>('[data-chart-ohlc]')!
    expect(within(ohlc).getByTitle('máximo')).toHaveTextContent('H')
  })
})
