/**
 * Armazón de terminal (#708): barra de estado con relojes y frescura de las fuentes, atajos de
 * función, `/` para la línea de comandos y `?` para la lista de atajos.
 */
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { ageText } from '@/lib/format'
import { clockText } from '@/lib/market-clocks'

const NOW = Date.parse('2026-11-25T16:00:00Z')

/** Gateway falso con estado, cotizaciones de IBKR y noticias; lo demás, 503. */
function gateway(down = false) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      if (down) return new Response('sin gateway', { status: 503 })
      const path = new URL(String(input), 'http://ui.test').pathname
      const body: Record<string, unknown> = {
        '/api/v1/ops/status': {
          as_of: '2026-11-25T15:59:30Z',
          mode: 'paper',
          gate: 'ready',
          accepting_orders: true,
          account: null,
          last_reconcile: null,
        },
        '/api/v1/ibkr/watchlist': {
          generated_at: '2026-11-25T15:59:00Z',
          source: 'ibkr',
          instruments: [
            { instrument_id: 'MNQ', quote: { delay_seconds: 960 } },
            { instrument_id: 'QQQ', quote: { delay_seconds: 1200 } },
            { instrument_id: 'MES', quote: null },
          ],
        },
        '/api/v1/news': {
          as_of: '2026-11-25T15:59:00Z',
          generated_at: '2026-11-25T15:59:00Z',
          feed: null,
          items: [
            { item: { published_at: '2026-11-25T13:00:00Z' }, interpretations: [] },
            { item: { published_at: '2026-11-25T15:48:00Z' }, interpretations: [] },
          ],
          next_before: null,
        },
      }
      return path in body
        ? new Response(JSON.stringify(body[path]), { status: 200 })
        : new Response('sin ruta', { status: 503 })
    }),
  )
}

beforeEach(() => {
  localStorage.clear()
  history.replaceState(null, '', '/')
  vi.useFakeTimers({ shouldAdvanceTime: true, now: NOW })
})
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
  localStorage.clear()
})

const status = () => screen.getByRole('contentinfo', { name: 'Estado' })

describe('barra de estado', () => {
  it('relojes de UTC, Nueva York, Londres y Tokio en 24 h', () => {
    gateway(true)
    render(<App initialStage="app" />)
    const clocks = within(status()).getByRole('list', { name: 'Relojes' })
    expect(within(clocks).getByLabelText('Hora UTC: 16:00')).toHaveTextContent('16:00:00')
    expect(within(clocks).getByLabelText('Hora de Nueva York: 11:00')).toBeInTheDocument()
    expect(within(clocks).getByLabelText('Hora de Londres: 16:00')).toBeInTheDocument()
    expect(within(clocks).getByLabelText('Hora de Tokio: 01:00')).toBeInTheDocument()
  })

  it('la frescura de cada fuente, tal como la manda el gateway', async () => {
    gateway()
    render(<App initialStage="app" />)
    const sources = within(status()).getByRole('list', { name: 'Fuentes de datos' })
    expect(await within(sources).findByText('listo · hace 30 s')).toBeInTheDocument()
    // el menor atraso informado; la cotización vacía no cuenta
    expect(await within(sources).findByText('~16 min de atraso')).toBeInTheDocument()
    // la noticia más nueva, no la primera de la lista
    expect(await within(sources).findByText('última hace 12 min')).toBeInTheDocument()
  })

  it('sin gateway lo dice en palabras, sin inventar estados', async () => {
    gateway(true)
    render(<App initialStage="app" />)
    const sources = within(status()).getByRole('list', { name: 'Fuentes de datos' })
    expect(await within(sources).findByText('sin conexión')).toBeInTheDocument()
    expect(within(sources).getAllByText('sin datos')).toHaveLength(2)
  })

  it('formatos: hora por zona y antigüedad corta', () => {
    const at = new Date(NOW)
    expect(clockText(at, 'Asia/Tokyo')).toBe('01:00')
    expect(clockText(at, 'UTC', true)).toBe('16:00:00')
    expect(ageText('2026-11-25T15:59:58Z', NOW)).toBe('ahora')
    expect(ageText('2026-11-25T15:59:00Z', NOW)).toBe('hace 1 min')
    expect(ageText('2026-11-25T12:00:00Z', NOW)).toBe('hace 4 h')
    expect(ageText('2026-11-20T16:00:00Z', NOW)).toBe('hace 5 d')
    expect(ageText('no es una fecha', NOW)).toBeNull()
  })
})

describe('teclado de terminal', () => {
  beforeEach(() => gateway(true))

  it('F1–F4 y F6–F9 abren cada sección; F5 queda para el navegador', () => {
    render(<App initialStage="app" />)
    const go = (key: string) => act(() => void fireEvent.keyDown(window, { key }))
    go('F2')
    expect(location.hash).toBe('#terminal')
    go('F4')
    expect(location.hash).toBe('#investigacion')
    go('F6')
    expect(location.hash).toBe('#noticias')
    go('F9')
    expect(location.hash).toBe('#competencia')
    go('F5')
    expect(location.hash).toBe('#competencia')
    go('F1')
    expect(screen.getByRole('heading', { level: 1, name: 'Inicio' })).toBeInTheDocument()
  })

  it('`/` abre la línea de comandos, salvo mientras se escribe', () => {
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: '/' })
    const field = screen.getByRole('combobox', { name: 'Buscar' })
    fireEvent.keyDown(field, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('`?` muestra la lista de atajos con la tecla de cada sección', () => {
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: '?' })
    const list = screen.getByRole('heading', { name: 'Atajos de teclado' }).closest('section')!
    expect(list).toHaveTextContent('F2Terminal')
    expect(list).toHaveTextContent('F9Trofeos')
  })

  it('la navegación lateral lleva la tecla de cada sección', () => {
    render(<App initialStage="app" />)
    const side = screen.getByRole('navigation', { name: 'Accesos' })
    expect(within(side).getByRole('link', { name: 'Terminal' })).toHaveAttribute(
      'aria-keyshortcuts',
      'F2',
    )
  })
})
