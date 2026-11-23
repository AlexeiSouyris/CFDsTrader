import { render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { OpsEvent, OpsStatus } from '@/lib/ops-api'
import { CURRENT_MEMBER } from '@/mocks'
import { OperationView } from '@/views/operation'
import { TerminalView } from '@/views/terminal'

/** Estado de prueba con la forma de `ops_status.v1` (números del motor, no de la UI). */
const READY: OpsStatus = {
  as_of: '2026-11-23T10:00:00Z',
  mode: 'paper',
  gate: 'ready',
  accepting_orders: true,
  account: {
    kill_switch: { active: false },
    daily_loss_used_pct: '40.0',
    daily_loss_limit_pct: '3',
    leverage: '1.35',
    open_positions: 1,
    max_positions: 5,
  },
  last_reconcile: {
    kind: 'startup',
    at: '2026-11-23T07:00:00Z',
    clean: true,
    matched: 3,
    mismatches: 0,
    drifts: 0,
  },
}

const KILLED: OpsStatus = {
  ...READY,
  accepting_orders: false,
  account: {
    ...READY.account!,
    daily_loss_used_pct: '112.5',
    kill_switch: {
      active: true,
      reason: 'Pérdida máxima diaria superada (spec §10).',
      since: '2026-11-23T09:30:00Z',
    },
  },
}

const EVENTS: OpsEvent[] = [
  {
    ts: '2026-11-23T09:31:00Z',
    level: 'warn',
    target: 'alert',
    event: 'alert',
    fields: { kind: 'kill_switch', message: 'Pérdida máxima diaria superada', at: 'x' },
  },
  {
    ts: '2026-11-23T09:30:00Z',
    level: 'warn',
    target: 'broker',
    event: 'kill_switch',
    fields: { reason: 'Pérdida máxima diaria superada', at: '2026-11-23T09:30:00Z' },
  },
  {
    ts: '2026-11-23T09:00:00Z',
    level: 'info',
    target: 'broker',
    event: 'order_report',
    fields: { report: { order_id: 'ui:7', status: 'filled' } },
  },
  {
    ts: '2026-11-23T08:59:00Z',
    level: 'error',
    target: 'gateway',
    event: 'store_failed',
    fields: { error: 'timeout', token: '***' },
  },
]

function gateway(status: OpsStatus | null, events: OpsEvent[] = EVENTS) {
  const fetch = vi.fn(async (input: string | URL | Request) => {
    const url = String(input)
    const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
    if (status && url === '/api/v1/ops/status') return json(status)
    if (status && url === '/api/v1/ops/events') return json(events)
    return new Response('', { status: 502 })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

const bar = () => screen.getByRole('region', { name: 'Estado de la operación' })

describe('estado de la operación (#314)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('la Terminal muestra compuerta, kill switch y pérdida del día tal como llegan', async () => {
    gateway(READY)
    render(<TerminalView member={CURRENT_MEMBER} />)
    await waitFor(() => expect(bar()).toHaveAttribute('data-ops-status', 'ready'))
    expect(bar()).toHaveTextContent('Acepta órdenes')
    expect(bar()).toHaveTextContent('Kill switch apagado')
    expect(bar()).toHaveTextContent('40,0 % del máximo (3 % del capital)')
    expect(bar()).toHaveTextContent('1,35:1')
    expect(bar()).toHaveTextContent('Reconciliación al arrancar: limpia (3 coinciden)')
    // El ancho es el texto del motor recortado por CSS: la UI no hace cuentas.
    const fill = bar().querySelector<HTMLElement>('[data-daily-loss-bar]')!
    expect(fill.style.width).toBe('min(40%, 100%)')
    expect(screen.queryByText(/Kill switch activo: no se opera/)).toBeNull()
  })

  it('con el kill switch activo hay un banner rojo inconfundible, uno solo', async () => {
    gateway(KILLED)
    render(<TerminalView member={CURRENT_MEMBER} />)
    const banner = await screen.findByText(/Kill switch activo: no se opera/i)
    const alert = banner.closest('[role="alert"]')!
    expect(alert).toHaveAttribute('data-kill-switch', 'active')
    expect(alert.className).toContain('bg-down')
    expect(alert).toHaveTextContent('Pérdida máxima diaria superada (spec §10).')
    expect(alert).toHaveTextContent('no se puede reiniciar')
    expect(bar()).toHaveTextContent('Kill switch ACTIVO')
    expect(bar()).toHaveTextContent('112,5 % del máximo')
    // El banner del ejemplo no se suma al del motor.
    expect(document.querySelectorAll('[role="alert"]')).toHaveLength(1)
  })

  it('no hay ningún control para reiniciar el kill switch', async () => {
    gateway(KILLED)
    render(<OperationView />)
    await screen.findByText(/Kill switch activo: no se opera/i)
    const controls = screen.queryAllByRole('button').map((b) => b.textContent ?? '')
    expect(controls.filter((t) => /reinici|reset|desactiv|liberar/i.test(t))).toEqual([])
  })

  it('sin gateway lo dice y no inventa un estado', async () => {
    gateway(null)
    render(<TerminalView member={CURRENT_MEMBER} />)
    await waitFor(() => expect(bar()).toHaveAttribute('data-ops-status', 'offline'))
    expect(bar()).toHaveTextContent('Sin conexión con el gateway')
    expect(bar()).not.toHaveTextContent('Acepta órdenes')
  })

  it('arrancando y con reconciliación sucia, lo muestra en la barra', async () => {
    gateway({
      ...READY,
      gate: 'starting',
      accepting_orders: false,
      last_reconcile: { ...READY.last_reconcile!, clean: false, mismatches: 1, drifts: 2 },
    })
    render(<OperationView />)
    await waitFor(() => expect(bar()).toHaveAttribute('data-ops-status', 'starting'))
    expect(bar()).toHaveTextContent('Arrancando: no acepta órdenes')
    expect(bar().querySelector('[data-reconcile="dirty"]')).toHaveTextContent(
      '1 posiciones distintas y 2 órdenes que no se reconstruyen igual',
    )
  })

  it('el Panel muestra los eventos y alertas del motor, el más nuevo primero', async () => {
    gateway(READY)
    render(<OperationView />)
    const list = await screen.findByRole('list', { name: 'Eventos y alertas' })
    const items = within(list).getAllByRole('listitem')
    expect(items).toHaveLength(4)
    expect(items[0]).toHaveTextContent('Alerta')
    expect(items[0]).toHaveTextContent('Pérdida máxima diaria superada')
    expect(items[1]).toHaveTextContent('Kill switch')
    expect(items[2]).toHaveTextContent('ui:7 · filled')
    expect(items[3]).toHaveAttribute('data-level', 'error')
    expect(items[3]).toHaveTextContent('timeout')
    expect(list).not.toHaveTextContent('***')
  })

  it('sin eventos o sin gateway lo dice', async () => {
    gateway(READY, [])
    const { unmount } = render(<OperationView />)
    expect(await screen.findByText(/Todavía no hay eventos/)).toBeInTheDocument()
    unmount()
    vi.unstubAllGlobals()
    gateway(null)
    render(<OperationView />)
    expect(await screen.findByText(/no hay eventos para mostrar/)).toBeInTheDocument()
  })
})
