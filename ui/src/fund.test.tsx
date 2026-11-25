import { render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { FundMovement, FundSummary } from '@/lib/fund-api'
import { FUND, FUND_EXAMPLE } from '@/mocks'
import { PerformanceView } from '@/views/performance'
import { FundView } from '@/views/phase10'

/**
 * El ejemplo del glosario (Ana, Beto y Caro) tal como lo manda el gateway (#312). Los números los
 * calculó el motor (ver `engine/crates/gateway/tests/fund_routes.rs`); acá solo se muestran.
 */
const SUMMARY: FundSummary = {
  currency: 'EUR',
  basis: 'last_settlement',
  valued_at: '2026-11-23T07:00:00Z',
  nav: '1.1',
  total_value: '275',
  total_units: '250.000000',
  members: [
    { member_id: 'ana', units: '100', value: '110.00', net_contributions: '100', pnl: '10.00' },
    { member_id: 'beto', units: '50', value: '55.00', net_contributions: '50', pnl: '5.00' },
    {
      member_id: 'caro',
      units: '100.000000',
      value: '110.00',
      net_contributions: '110',
      pnl: '0.00',
    },
  ],
}
const move = (n: number, day: string, member: string, amount: string, units: string, nav: string) =>
  ({
    id: `fund:${day}:${n}`,
    settled_at: `${day}T07:00:00Z`,
    member_id: member,
    kind: 'contribution',
    amount,
    units,
    nav,
    currency: 'EUR',
  }) satisfies FundMovement
const MOVEMENTS: FundMovement[] = [
  move(1, '2026-11-16', 'ana', '100', '100', '1'),
  move(2, '2026-11-16', 'beto', '50', '50', '1'),
  move(1, '2026-11-23', 'caro', '110', '100.000000', '1.10'),
]

function gateway(up = true) {
  const fetch = vi.fn(async (input: string | URL | Request) => {
    const url = String(input)
    const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
    if (!up) return new Response('', { status: 502 })
    if (url === '/api/v1/fund') return json(SUMMARY)
    if (url === '/api/v1/fund/movements') return json(MOVEMENTS)
    return new Response('ruta desconocida', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

const members = () => screen.getByRole('table', { name: 'Cada miembro' })

describe('fondo del motor (fase 6b, #312)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('muestra el resumen y cada miembro tal como los manda el motor', async () => {
    const fetch = gateway()
    const { container } = render(<FundView />)
    await waitFor(() =>
      expect(container.querySelector('[data-fund-source="engine"]')).not.toBeNull(),
    )
    expect(fetch).toHaveBeenCalledWith('/api/v1/fund', expect.anything())
    expect(fetch).toHaveBeenCalledWith('/api/v1/fund/movements', expect.anything())
    expect(screen.getByText('1,1000 €')).toBeInTheDocument()
    expect(screen.getByText('275 €')).toBeInTheDocument()
    expect(screen.getByText('liquidación del', { exact: false })).toBeInTheDocument()

    const rows = within(members()).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(3)
    expect(rows[0]).toHaveTextContent('ana')
    expect(rows[0]).toHaveTextContent('110,00 €')
    expect(rows[0]).toHaveTextContent('+10,00 €')
    expect(rows[1]).toHaveTextContent('+5,00 €')
    expect(rows[2]).toHaveTextContent('+0,00 €')
    // Con gateway no queda nada de ejemplo: los pedidos pendientes (el motor no los publica
    // todavía) no se muestran (#577).
    expect(document.querySelector('[data-example-badge]')).toBeNull()
    expect(screen.getByText(/no publica los pedidos del lunes/)).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: /^Cada miembro/ })).toHaveTextContent('del motor')
  })

  it('la tabla de movimientos es el historial, del más reciente al más antiguo', async () => {
    gateway()
    render(<FundView />)
    const table = await screen.findByRole('table', { name: 'Movimientos del fondo' })
    await waitFor(() => expect(within(table).getAllByRole('row')).toHaveLength(4))
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows[0]).toHaveTextContent('caro')
    expect(rows[0]).toHaveTextContent('110 €')
    expect(rows[0]).toHaveTextContent('1,10')
    expect(rows[2]).toHaveTextContent('ana')
  })

  it('no recalcula: muestra el total que manda el motor aunque no sea la suma de la pantalla', async () => {
    // Un total que no es la suma de las filas: la UI no lo "corrige".
    gateway()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: string | URL | Request) =>
        String(input) === '/api/v1/fund'
          ? new Response(JSON.stringify({ ...SUMMARY, total_value: '999.99' }))
          : new Response(JSON.stringify(MOVEMENTS)),
      ),
    )
    render(<FundView />)
    expect(await screen.findByText('999,99 €')).toBeInTheDocument()
  })

  it('sin gateway muestra el ejemplo, con el sello EJEMPLO', async () => {
    gateway(false)
    const { container } = render(<FundView />)
    // cada miembro, historial y pedidos pendientes llevan el sello (#577)
    await waitFor(() =>
      expect(container.querySelectorAll('[data-example-badge]').length).toBeGreaterThanOrEqual(3),
    )
    expect(container.querySelector('[data-fund-source="example"]')).not.toBeNull()
    expect(screen.getByText('Datos de ejemplo')).toBeInTheDocument()
    expect(within(members()).getAllByRole('row')).toHaveLength(FUND_EXAMPLE.members.length + 1)
    expect(screen.getByText(`${FUND.nav.replace('.', ',')} €`)).toBeInTheDocument()
  })

  it('Performance usa el fondo del motor y, sin gateway, avisa que es de ejemplo', async () => {
    gateway()
    const { unmount } = render(<PerformanceView onOpen={() => {}} />)
    expect(await screen.findByText('simulación, del motor')).toBeInTheDocument()
    expect(screen.getByText(/250,000000 unidades/)).toBeInTheDocument()
    unmount()
    vi.unstubAllGlobals()

    gateway(false)
    render(<PerformanceView onOpen={() => {}} />)
    expect(await screen.findByText('simulación (ejemplo)')).toBeInTheDocument()
  })
})
