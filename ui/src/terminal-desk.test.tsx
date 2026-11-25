import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Watchlist } from '@/components/ui/watchlist'
import { liquiditySummaries } from '@/lib/market-summary'
import type { Order, OrderReport, PaperAccount, TicketVerdict } from '@/lib/paper-contracts'
import { CURRENT_MEMBER, PAPER_ACCOUNT } from '@/mocks'
import { TerminalView } from '@/views/terminal'

/** Datos de prueba con la forma de los contratos (no son precios reales). */
const ACCOUNT: PaperAccount = { ...PAPER_ACCOUNT, realized_pl_today: '-12.50', open_positions: 0 }

const CATALOG = {
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
      id: 'US30',
      name: 'Wall Street',
      asset_class: 'index',
      currency: 'EUR',
      exchange_timezone: 'America/New_York',
    },
  ],
}

const view = (instrument: string, candles: unknown[]) => ({
  instrument_id: instrument,
  source: { id: 'ig', label: 'IG CFD' },
  timeframe: '15m',
  generated_at: '2026-11-24T10:00:00Z',
  as_of: candles.length ? '2026-11-24T10:00:00Z' : null,
  candles,
  levels: [],
  fvgs: [],
  imbalance: [],
  stats: [],
})

const DE40_CANDLE = {
  instrument_id: 'DE40',
  timeframe: '15m',
  start: '2026-11-24T09:45:00Z',
  open: '100.0',
  high: '101.0',
  low: '99.0',
  close: '100.5',
  volume: null,
}

type Call = { url: string; init?: RequestInit }

/** Un gateway falso de solo prueba: nada sale al broker simulado real. */
function gateway(opts: { eventsDown?: boolean } = {}) {
  const calls: Call[] = []
  const fetch = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input)
    calls.push({ url, init })
    const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
    if (url === '/api/v1/instruments') return json(CATALOG)
    if (url.startsWith('/api/v1/liquidity?instrument=DE40&timeframe=15m'))
      return json(view('DE40', [DE40_CANDLE]))
    if (url.startsWith('/api/v1/liquidity?')) {
      const id = new URLSearchParams(url.split('?')[1]).get('instrument')!
      return json(view(id, []))
    }
    if (url === '/api/v1/paper/account') return json(ACCOUNT)
    if (url === '/api/v1/paper/positions') return json([])
    if (url === '/api/v1/ops/events') {
      if (opts.eventsDown) return new Response('', { status: 502 })
      return json([
        {
          ts: '2026-11-24T10:00:00Z',
          level: 'info',
          target: 'broker',
          event: 'order_report',
          fields: { report: { order_id: 'ui-1', status: 'filled' } },
        },
      ])
    }
    if (url === '/api/v1/paper/preview') {
      const o = JSON.parse(String(init?.body)) as Order
      return json({
        order_id: o.order_id,
        accepted: true,
        rejection: null,
        reference: '100.5',
        size: '2',
        risk: '20.00',
        risk_pct: '0.20',
        leverage: '1.1',
        margin: '300.00',
        leverage_capped: false,
        as_of: '2026-11-24T10:00:00Z',
      } satisfies TicketVerdict)
    }
    if (url === '/api/v1/paper/orders') {
      const o = JSON.parse(String(init?.body)) as Order
      return json({
        order_id: o.order_id,
        status: 'rejected',
        rejection: 'insufficient_margin',
        position_id: null,
        size: null,
        fill_price: null,
        commission: null,
        at: '2026-11-24T10:00:05Z',
      } satisfies OrderReport)
    }
    return new Response('', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
  return { calls }
}

const markets = () => screen.getByRole('region', { name: 'Mercados' })
const ticket = () => screen.getByRole('form', { name: 'Ticket de orden' })

describe('terminal de trading completa (#552)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('la lista de mercados muestra el último cierre del gateway, sin inventar variación', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    const de40 = await within(markets()).findByText('100,5')
    const row = de40.closest('button')!
    expect(row).toHaveTextContent('DE40')
    expect(row).toHaveTextContent('Alemania 40')
    expect(row).not.toHaveTextContent('%')
    const us30 = within(markets()).getByText('US30').closest('button')!
    expect(us30).toHaveTextContent('—')
    expect(us30).toHaveTextContent('sin velas')
    expect(markets()).toHaveTextContent('último cierre de 15 m del motor')
    expect(markets()).toHaveTextContent('la fuente todavía no los manda')
  })

  it('elegir un mercado lo abre en el gráfico activo y en el ticket, sin stop', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    fireEvent.click(await within(markets()).findByText('Wall Street'))
    const pick = screen.getByRole('group', { name: 'Instrumento del gráfico 1' })
    expect(within(pick).getByRole('button', { name: 'US30' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(within(ticket()).getByLabelText('Instrumento')).toHaveValue('US30')
    expect(within(ticket()).getByLabelText(/Stop/)).toHaveValue('')
    expect(within(markets()).getByText('US30').closest('button')).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('una orden enviada queda en la pestaña Órdenes con el informe del broker simulado', async () => {
    const gw = gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    fireEvent.click(screen.getByRole('tab', { name: 'Órdenes (0)' }))
    expect(screen.getByRole('tabpanel')).toHaveTextContent('no tiene una ruta para listar')

    const form = ticket()
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    fireEvent.click(await within(form).findByRole('button', { name: 'Comprar en simulación' }))
    expect(await within(form).findByText('El broker simulado la rechazó.')).toBeInTheDocument()

    const tab = await screen.findByRole('tab', { name: 'Órdenes (1)' })
    expect(tab).toHaveAttribute('aria-selected', 'true')
    const table = within(screen.getByRole('tabpanel')).getByRole('table', {
      name: 'Órdenes de la sesión',
    })
    const [row] = within(table).getAllByRole('row').slice(1)
    expect(row).toHaveTextContent('US100')
    expect(row).toHaveTextContent('Rechazada')
    expect(row).toHaveTextContent('No alcanza el margen libre.')
    // validar y enviar, nada más: la terminal no manda órdenes por su cuenta
    expect(gw.calls.filter((c) => c.init?.method === 'POST').map((c) => c.url)).toEqual([
      '/api/v1/paper/preview',
      '/api/v1/paper/orders',
    ])
  })

  it('Historial dice que no hay ruta y muestra el P/L realizado del motor', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    fireEvent.click(screen.getByRole('tab', { name: 'Historial' }))
    const panel = screen.getByRole('tabpanel')
    expect(panel).toHaveTextContent('todavía no tiene una ruta para el historial')
    expect(panel).toHaveTextContent('−12,50 EUR')
    expect(panel).toHaveTextContent('del motor')
  })

  it('Actividad pide los eventos solo al abrirla, y sin gateway lo dice', async () => {
    const gw = gateway()
    const { unmount } = render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    expect(gw.calls.some((c) => c.url === '/api/v1/ops/events')).toBe(false)
    fireEvent.click(screen.getByRole('tab', { name: 'Actividad' }))
    expect(await screen.findByText('Informe de orden')).toBeInTheDocument()
    unmount()
    vi.unstubAllGlobals()

    gateway({ eventsDown: true })
    render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Actividad' }))
    await waitFor(() =>
      expect(screen.getByRole('tabpanel')).toHaveTextContent('Sin conexión con el gateway'),
    )
  })

  it('la barra de dibujo (compartida con el Multigráfico) maneja el gráfico activo', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const bar = screen.getByRole('toolbar', { name: 'Herramientas de dibujo del gráfico 1' })
    const hline = within(bar).getByRole('button', { name: 'Línea horizontal' })
    fireEvent.click(hline)
    expect(hline).toHaveAttribute('aria-pressed', 'true')
    expect(within(bar).getByRole('button', { name: 'Navegar' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
  })

  it('en el celular: selector de panel y "simulación" fijo; la cuenta en una sola franja', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const panes = screen.getByRole('group', { name: 'Panel a la vista' })
    expect(
      within(panes)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Gráfico', 'Mercados', 'Ticket'])
    expect(document.querySelector('[data-sim-pill]')).toHaveTextContent('SIMULACIÓN')
    // Mercados oculto en el celular hasta elegirlo
    const desk = document.querySelector('[data-terminal-desk]')!
    expect(desk.children[0].className).toContain('max-lg:hidden')
    fireEvent.click(within(panes).getByRole('button', { name: 'Mercados' }))
    expect(desk.children[0].className).not.toContain('max-lg:hidden')
    expect(desk.children[1].className).toContain('max-lg:hidden')
  })
})

describe('resumen de mercados (#552)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('sin gateway es un error (y la lista usa el ejemplo), no una lista vacía', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('', { status: 502 })),
    )
    await expect(
      liquiditySummaries.load(['DE40', 'US30'], new AbortController().signal),
    ).rejects.toThrow('502')
  })

  it('muestra la variación y el atraso solo si la fuente los manda, tal cual', () => {
    render(
      <Watchlist
        rows={[
          {
            instrument_id: 'DE40',
            last: '100.5',
            at: '2026-11-24T10:00:00Z',
            change_pct: '-0.42',
            delay_s: 600,
          },
        ]}
        names={{ DE40: 'Alemania 40' }}
        selected="DE40"
        onSelect={() => {}}
        sourceLabel="fuente de prueba"
      />,
    )
    const row = screen.getByRole('button', { name: /DE40/ })
    expect(row).toHaveTextContent('−0,42 %')
    expect(row).toHaveTextContent('10 min de atraso')
    expect(screen.queryByText(/la fuente todavía no los manda/)).toBeNull()
  })
})
