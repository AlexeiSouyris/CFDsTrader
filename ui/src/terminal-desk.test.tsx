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
    {
      id: 'US500',
      name: 'US 500',
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
function gateway(
  opts: { eventsDown?: boolean; previewStatus?: number; verdict?: Partial<TicketVerdict> } = {},
) {
  const calls: Call[] = []
  const fetch = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input)
    calls.push({ url, init })
    const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
    if (url === '/api/v1/instruments') return json(CATALOG)
    if (url.startsWith('/api/v1/liquidity?instrument=DE40&timeframe=15m'))
      return json(view('DE40', [DE40_CANDLE]))
    if (url.startsWith('/api/v1/liquidity?instrument=US500&timeframe=15m'))
      return json(view('US500', [{ ...DE40_CANDLE, instrument_id: 'US500', close: '200.5' }]))
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
      if (opts.previewStatus)
        return new Response(
          'no es un order.v1 válido: decimal fuera del formato del contrato: "1.2.3" at line 1',
          { status: opts.previewStatus },
        )
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
        ...opts.verdict,
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
/** Con gateway, el ticket aparece cuando llegan el catálogo y las velas; arranca vacío (#578). */
async function readyTicket(stop = '99') {
  await screen.findByText('simulación · broker simulado')
  // el de US30 (sin velas) queda deshabilitado cuando llega la lista de mercados
  await waitFor(() =>
    expect(screen.getByRole('option', { name: /US30.*sin velas/ })).toBeDisabled(),
  )
  const form = ticket()
  if (stop) fireEvent.change(within(form).getByLabelText(/Stop/), { target: { value: stop } })
  return form
}

describe('terminal de trading completa (#552)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('la lista de mercados muestra el último cierre del gateway, sin inventar variación', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    const de40 = await within(markets()).findByText('100,50')
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

  it('el ticket arranca vacío, sigue al gráfico activo y ofrece solo lo operable (#578)', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = await readyTicket('')
    const instrument = () => within(ticket()).getByLabelText('Instrumento')
    expect(instrument()).toHaveValue('DE40')
    expect(within(form).getByLabelText(/Stop/)).toHaveValue('')
    expect(within(form).getByLabelText(/TP1/)).toHaveValue('')
    // solo los del catálogo del motor; el que no tiene velas se ve pero no se elige
    const options = within(form).getAllByRole('option') as HTMLOptionElement[]
    expect(options.map((o) => o.value)).toEqual(['DE40', 'US30', 'US500'])
    expect(
      within(form).getByRole('option', { name: /US30 · Wall Street \(sin velas\)/ }),
    ).toBeDisabled()

    // el gráfico pasa a US500 (operable): el ticket lo sigue, vacío
    fireEvent.change(within(form).getByLabelText(/Stop/), { target: { value: '99' } })
    fireEvent.click(within(markets()).getByText('US 500'))
    const pick = screen.getByRole('group', { name: 'Instrumento del gráfico 1' })
    expect(within(pick).getByRole('button', { name: 'US500' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(instrument()).toHaveValue('US500')
    expect(within(ticket()).getByLabelText(/Stop/)).toHaveValue('')
    // a US30 (sin velas): el gráfico cambia, el ticket se queda en el último operable
    fireEvent.click(within(markets()).getByText('Wall Street'))
    expect(within(markets()).getByText('US30').closest('button')).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(instrument()).toHaveValue('US500')
  })

  it('valida el precio con mensajes propios, sin el globo del navegador ni llamar al motor (#578)', async () => {
    const gw = gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = await readyTicket('')
    expect(form).toHaveAttribute('novalidate')
    expect(within(form).getByLabelText(/Stop/)).not.toHaveAttribute('required')
    const validate = () =>
      fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    validate()
    expect(within(form).getByRole('alert')).toHaveTextContent('El stop es obligatorio')
    for (const bad of ['abc', '-5', '0', '1.2.3,4']) {
      fireEvent.change(within(form).getByLabelText(/Stop/), { target: { value: bad } })
      validate()
      expect(within(form).getByRole('alert')).toHaveTextContent('precio positivo')
      expect(within(form).getByLabelText(/Stop/)).toHaveAttribute('aria-invalid', 'true')
    }
    fireEvent.change(within(form).getByLabelText(/Stop/), { target: { value: '24.800,5' } })
    fireEvent.change(within(form).getByLabelText(/TP1/), { target: { value: 'x' } })
    validate()
    expect(within(form).getByRole('alert')).toHaveTextContent('El TP1 tiene que ser')
    expect(gw.calls.some((c) => c.init?.method === 'POST')).toBe(false)

    // con separador de miles y coma decimal, viaja como decimal del contrato
    fireEvent.change(within(form).getByLabelText(/TP1/), { target: { value: '' } })
    validate()
    await within(form).findByRole('region', { name: 'Validación de riesgo del motor' })
    const [preview] = gw.calls.filter((c) => c.init?.method === 'POST')
    expect((JSON.parse(String(preview.init!.body)) as Order).stop).toBe('24800.5')
  })

  it('un error del contrato se dice con palabras propias, no con el texto crudo (#578)', async () => {
    gateway({ previewStatus: 400 })
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = await readyTicket()
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    const alert = await within(form).findByRole('alert')
    expect(alert).toHaveTextContent('El motor no pudo procesar la orden.')
    expect(alert).toHaveTextContent('Revisá los precios')
    expect(alert).not.toHaveTextContent('order.v1')
    expect(alert).not.toHaveTextContent('No se pudo hablar con el motor')
  })

  it('sin precio de referencia no muestra "vela usada" (#578)', async () => {
    gateway({
      verdict: {
        accepted: false,
        rejection: 'unknown_instrument',
        reference: null,
        size: null,
        risk: null,
        risk_pct: null,
        leverage: null,
        margin: null,
      },
    })
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = await readyTicket()
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    const verdict = await within(form).findByRole('region', {
      name: 'Validación de riesgo del motor',
    })
    expect(verdict).toHaveTextContent('Instrumento desconocido.')
    expect(verdict).not.toHaveTextContent('Vela usada')
  })

  it('una orden enviada queda en la pestaña Órdenes con el informe del broker simulado', async () => {
    const gw = gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = await readyTicket()
    fireEvent.click(screen.getByRole('tab', { name: 'Órdenes (0)' }))
    expect(screen.getByRole('tabpanel')).toHaveTextContent('no tiene una ruta para listar')

    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    fireEvent.click(await within(form).findByRole('button', { name: 'Comprar en simulación' }))
    expect(await within(form).findByText('El broker simulado la rechazó.')).toBeInTheDocument()

    const tab = await screen.findByRole('tab', { name: 'Órdenes (1)' })
    expect(tab).toHaveAttribute('aria-selected', 'true')
    const table = within(screen.getByRole('tabpanel')).getByRole('table', {
      name: 'Órdenes de la sesión',
    })
    const [row] = within(table).getAllByRole('row').slice(1)
    expect(row).toHaveTextContent('DE40')
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
    // el selector 1/2/4 solo en la pantalla del gráfico (#578)
    const picker = () => document.querySelector('[data-layout-picker]')!
    expect(picker().className).not.toContain('max-lg:hidden')
    fireEvent.click(within(panes).getByRole('button', { name: 'Mercados' }))
    expect(desk.children[0].className).not.toContain('max-lg:hidden')
    expect(desk.children[1].className).toContain('max-lg:hidden')
    expect(picker().className).toContain('max-lg:hidden')
    // "simulación" ocupa su lugar: no flota encima del contenido (sin alto cero ni margen negativo)
    expect(document.querySelector('[data-sim-pill]')!.className).not.toMatch(/\bh-0\b|-mb-/)
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
        groups={[
          {
            id: 'prueba',
            title: 'Prueba',
            sourceLabel: 'fuente de prueba',
            rows: [
              {
                instrument_id: 'DE40',
                last: '100.5',
                at: '2026-11-24T10:00:00Z',
                change_pct: '-0.42',
                delay_s: 600,
              },
            ],
          },
        ]}
        names={{ DE40: 'Alemania 40' }}
        selected="DE40"
        onSelect={() => {}}
      />,
    )
    const row = screen.getByRole('button', { name: /DE40/ })
    expect(row).toHaveTextContent('−0,42 %')
    expect(row).toHaveTextContent('10 min de atraso')
    expect(screen.queryByText(/la fuente todavía no los manda/)).toBeNull()
  })
})
