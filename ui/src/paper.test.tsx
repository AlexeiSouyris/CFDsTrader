import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { gatewayTokenFor } from '@/lib/gateway-token'
import type {
  Order,
  OrderReport,
  PaperAccount,
  Position,
  TicketVerdict,
} from '@/lib/paper-contracts'
import { BACKTEST_RUNS, CURRENT_MEMBER, PAPER_ACCOUNT, PAPER_POSITIONS } from '@/mocks'
import { PerformanceView } from '@/views/performance'
import { TerminalView } from '@/views/terminal'

/** Datos del broker simulado de prueba (no son precios reales), con la forma de los contratos. */
const ACCOUNT: PaperAccount = {
  ...PAPER_ACCOUNT,
  equity: '10000.00',
  unrealized_pl: '0',
  realized_pl_today: '0',
  margin_used: '0',
  margin_free: '10000.00',
  leverage: '0',
  daily_loss_used_pct: '0',
  open_positions: 1,
}
const POSITION: Position = {
  ...PAPER_POSITIONS[0],
  position_id: 'gw:US30:1',
  instrument_id: 'US30',
}

const verdictFor = (o: Order, accepted = true): TicketVerdict => ({
  order_id: o.order_id,
  accepted,
  rejection: accepted ? null : 'stop_wrong_side',
  reference: '100.0',
  size: accepted ? '2.5' : null,
  risk: accepted ? '50.00' : null,
  risk_pct: accepted ? '0.50' : null,
  leverage: accepted ? '1.2' : null,
  margin: accepted ? '400.00' : null,
  leverage_capped: false,
  as_of: '2026-11-23T10:00:00Z',
})

type Call = { url: string; init?: RequestInit }

/** Un gateway falso con las rutas del broker simulado. */
function gateway(opts: {
  accept?: boolean
  report?: Partial<OrderReport>
  previewStatus?: number
}) {
  const calls: Call[] = []
  let positions: Position[] = []
  const fetch = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input)
    calls.push({ url, init })
    const json = (b: unknown, status = 200) => new Response(JSON.stringify(b), { status })
    if (url === '/api/v1/paper/account') return json(ACCOUNT)
    if (url === '/api/v1/paper/positions') return json(positions)
    if (url === '/api/v1/paper/preview') {
      if (opts.previewStatus)
        return new Response('escritura apagada: falta el token local del gateway', {
          status: opts.previewStatus,
        })
      return json(verdictFor(JSON.parse(String(init?.body)) as Order, opts.accept ?? true))
    }
    if (url === '/api/v1/paper/orders') {
      const o = JSON.parse(String(init?.body)) as Order
      positions = [POSITION]
      return json({
        order_id: o.order_id,
        status: 'filled',
        rejection: null,
        position_id: 'gw:US30:1',
        size: '2.5',
        fill_price: '100.4',
        commission: '0',
        at: '2026-11-23T10:00:00Z',
        ...opts.report,
      } satisfies OrderReport)
    }
    if (url === '/api/v1/backtest_runs') return json([BACKTEST_RUNS[1]])
    return new Response('ruta desconocida', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
  return { calls, posts: () => calls.filter((c) => c.init?.method === 'POST') }
}

const ticket = () => screen.getByRole('form', { name: 'Ticket de orden' })

describe('terminal con el broker simulado del gateway (#292)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('muestra la cuenta y las posiciones del gateway, marcadas como simulación', async () => {
    gateway({})
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    expect(screen.getByRole('status', { name: /Modo simulación/ })).toHaveTextContent(
      'MODO SIMULACIÓN',
    )
    expect(screen.getByRole('region', { name: 'Cuenta de simulación' })).toHaveTextContent(
      '10.000,00',
    )
    // sin el escenario de ejemplo cuando hay gateway
    expect(screen.queryByRole('group', { name: 'Escenario de ejemplo' })).toBeNull()
    expect(screen.getByText('No hay posiciones abiertas.')).toBeInTheDocument()
  })

  it('valida con preview, envía con orders el mismo order_id y vuelve a pedir las posiciones', async () => {
    const gw = gateway({})
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    const form = ticket()
    fireEvent.change(within(form).getByLabelText(/Stop/), { target: { value: '99,5' } })
    fireEvent.change(within(form).getByLabelText(/TP1/), { target: { value: '' } })
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))

    const verdict = await within(form).findByRole('region', {
      name: 'Validación de riesgo del motor',
    })
    expect(verdict).toHaveTextContent('El motor la acepta')
    expect(verdict).toHaveTextContent('2,5 €/pt')
    expect(verdict).toHaveTextContent('50,00 € · 0,50 %')

    const [preview] = gw.posts()
    expect(preview.url).toBe('/api/v1/paper/preview')
    const headers = preview.init!.headers as Record<string, string>
    expect(headers['Content-Type']).toBe('application/json')
    // el token no lo pone el navegador: lo agrega el proxy de Vite
    expect(Object.keys(headers).map((h) => h.toLowerCase())).not.toContain('x-cfds-token')
    const order = JSON.parse(String(preview.init!.body)) as Order
    expect(order).toMatchObject({
      mode: 'paper',
      instrument_id: 'US100',
      side: 'long',
      stop: '99.5',
      tp1: null,
      proposal_id: null,
    })
    expect(order).not.toHaveProperty('size')
    expect(order.created_at).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/)
    expect(order.order_id).toMatch(/^[A-Za-z0-9._:-]{1,200}$/)

    fireEvent.click(within(form).getByRole('button', { name: 'Comprar en simulación' }))
    expect(await within(form).findByText('Ejecutada en simulación (paper).')).toBeInTheDocument()
    const sent = gw.posts()[1]
    expect(sent.url).toBe('/api/v1/paper/orders')
    expect((JSON.parse(String(sent.init!.body)) as Order).order_id).toBe(order.order_id)
    // después del fill se vuelven a pedir las posiciones
    expect(await screen.findByRole('table', { name: 'Posiciones abiertas' })).toHaveTextContent(
      'US30',
    )
    expect(gw.posts()).toHaveLength(2)
  })

  it('un rechazo del riesgo lo dice con su motivo y no deja enviar', async () => {
    gateway({ accept: false })
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    const form = ticket()
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    expect(await within(form).findByText('El motor la rechaza')).toBeInTheDocument()
    expect(
      within(form).getByText('El stop está del lado equivocado del precio.'),
    ).toBeInTheDocument()
    expect(within(form).queryByRole('button', { name: /en simulación/ })).toBeNull()
  })

  it('un informe rechazado del broker muestra el motivo', async () => {
    gateway({
      report: {
        status: 'rejected',
        rejection: 'insufficient_margin',
        position_id: null,
        size: null,
        fill_price: null,
        commission: null,
      },
    })
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    const form = ticket()
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    fireEvent.click(await within(form).findByRole('button', { name: 'Comprar en simulación' }))
    expect(await within(form).findByText('El broker simulado la rechazó.')).toBeInTheDocument()
    expect(within(form).getByText('No alcanza el margen libre.')).toBeInTheDocument()
  })

  it('si el gateway no deja escribir (sin token), lo avisa sin inventar respuesta', async () => {
    gateway({ previewStatus: 403 })
    render(<TerminalView member={CURRENT_MEMBER} />)
    await screen.findByText('simulación · broker simulado')
    const form = ticket()
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    const alert = await within(form).findByRole('alert')
    expect(alert).toHaveTextContent('403')
    expect(alert).toHaveTextContent('falta el token')
    expect(within(form).queryByRole('region', { name: /Validación/ })).toBeNull()
  })

  it('sin gateway usa los datos de ejemplo y no envía nada', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('', { status: 502 })),
    )
    render(<TerminalView member={CURRENT_MEMBER} />)
    await waitFor(() => expect(screen.getByText('simulación · ejemplo')).toBeInTheDocument())
    expect(screen.getByRole('group', { name: 'Escenario de ejemplo' })).toBeInTheDocument()
    expect(screen.getByRole('table', { name: 'Posiciones abiertas' })).toHaveTextContent('US500')
  })
})

describe('Performance con las corridas del gateway (#292)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('lista las corridas de GET /api/v1/backtest_runs', async () => {
    gateway({})
    render(<PerformanceView onOpen={() => {}} />)
    expect(await screen.findByText('1 corrida del motor')).toBeInTheDocument()
    const rows = within(screen.getByRole('table', { name: 'Corridas de backtest' })).getAllByRole(
      'row',
    )
    expect(rows).toHaveLength(2)
    expect(rows[1]).toHaveTextContent(BACKTEST_RUNS[1].run_id)
    expect(document.body).toHaveTextContent('simulación (paper)')
  })

  it('sin gateway muestra las de ejemplo, marcadas así', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('', { status: 503 })),
    )
    render(<PerformanceView onOpen={() => {}} />)
    expect(
      await screen.findByText(`${BACKTEST_RUNS.length} corridas (ejemplo)`),
    ).toBeInTheDocument()
    expect(document.body).toHaveTextContent('datos de ejemplo')
  })
})

describe('token del proxy de Vite (#292)', () => {
  const ui = { origin: 'http://localhost:5173', host: 'localhost:5173' }

  it('solo va en los POST del broker simulado que vienen de la misma UI', () => {
    const post = { ...ui, method: 'POST', url: '/api/v1/paper/orders' }
    expect(gatewayTokenFor(post, 'secreto-de-prueba')).toBe('secreto-de-prueba')
    expect(gatewayTokenFor({ ...post, url: '/api/v1/paper/preview' }, 'x')).toBe('x')
    // sin token configurado, nada
    expect(gatewayTokenFor(post, undefined)).toBeNull()
    // lecturas y otras rutas, sin token
    expect(gatewayTokenFor({ ...post, method: 'GET' }, 'x')).toBeNull()
    expect(gatewayTokenFor({ ...post, url: '/api/v1/liquidity' }, 'x')).toBeNull()
    // otra web, o sin Origin: sin token
    expect(gatewayTokenFor({ ...post, origin: 'https://otra.example' }, 'x')).toBeNull()
    expect(gatewayTokenFor({ ...post, origin: undefined }, 'x')).toBeNull()
    expect(gatewayTokenFor({ ...post, origin: 'no es una url' }, 'x')).toBeNull()
  })
})
