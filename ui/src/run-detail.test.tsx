import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { RunDetail } from '@/lib/backtest-contracts'
import { decimal } from '@/lib/format'
import { BACKTEST_RUN_DETAIL, BACKTEST_RUNS } from '@/mocks'
import { PerformanceView } from '@/views/performance'

const open = (runId: string) =>
  fireEvent.click(screen.getByRole('button', { name: `Ver el detalle de ${runId}` }))
const detailOf = (runId: string) =>
  screen.findByRole('region', { name: `Detalle de la corrida ${runId}` })

describe('detalle de una corrida de backtest (#299)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('sin gateway muestra el detalle de ejemplo: curvas, métricas, kill switch y operaciones', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('', { status: 503 })),
    )
    render(<PerformanceView onOpen={() => {}} />)
    const id = BACKTEST_RUN_DETAIL.run.run_id
    await screen.findByText(`${BACKTEST_RUNS.length} corridas (ejemplo)`)
    open(id)
    const detail = await detailOf(id)
    expect(detail).toHaveTextContent('Simulación · backtest · datos de ejemplo')

    // las dos curvas, tal como las manda el motor, y los eventos del kill switch
    const chart = within(detail).getByRole('img', { name: /Curva de capital de \d+ días/ })
    expect(chart.querySelector('[data-curve="equity"]')).not.toBeNull()
    expect(chart.querySelector('[data-curve="drawdown"]')).not.toBeNull()
    expect(chart).toHaveAccessibleName(
      `Curva de capital de ${BACKTEST_RUN_DETAIL.curve!.points.length} días, de ${decimal(BACKTEST_RUN_DETAIL.curve!.points[0].equity)} a 9.875,40, y drawdown máximo de 8,74 %.`,
    )
    expect(chart.querySelectorAll('[data-kill-switch]')).toHaveLength(2)

    // métricas del motor, sin recalcular
    const dl = within(detail).getByLabelText('Métricas de la corrida')
    expect(dl).toHaveTextContent('−1,25 %')
    expect(dl).toHaveTextContent('8,74 %')
    expect(dl).toHaveTextContent('−0,12')
    expect(dl).toHaveTextContent('12')

    // comprar y mantener al lado
    const bh = within(detail).getByRole('list', { name: 'Comprar y mantener' })
    expect(within(bh).getAllByRole('listitem')).toHaveLength(5)
    expect(bh).toHaveTextContent('US100')

    // kill switch: activación y reactivación
    const ks = within(detail).getByRole('list', { name: 'Eventos del kill switch' })
    expect(ks).toHaveTextContent('Activado')
    expect(ks).toHaveTextContent('Reactivado (solo backtest)')

    // operaciones: entrada, salida, P/L y motivo; la que no tiene resultado lo dice
    const table = within(detail).getByRole('table', { name: 'Operaciones de la corrida' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(BACKTEST_RUN_DETAIL.trades.length)
    const first = BACKTEST_RUN_DETAIL.trades[0].outcome!
    expect(rows[0]).toHaveTextContent('TP1')
    expect(rows[0]).toHaveTextContent(first.net_pnl.replace('.', ','))
    expect(rows.at(-1)).toHaveTextContent('sin resultado')

    // lugar reservado para el walk-forward, sin datos inventados
    expect(detail.querySelector('[data-walk-forward="pending"]')).toHaveTextContent('fase 5')
    expect(detail.textContent).not.toMatch(/garantiz|asegurad|sin riesgo|ganancia segura/i)

    fireEvent.click(within(detail).getByRole('button', { name: /Cerrar el detalle/ }))
    expect(screen.queryByRole('region', { name: /Detalle de la corrida/ })).toBeNull()

    // otra corrida de ejemplo: no hay detalle inventado
    open(BACKTEST_RUNS[0].run_id)
    expect(await screen.findByRole('status')).toHaveTextContent('solo hay un detalle de ejemplo')
  })

  it('con gateway pide GET /api/v1/backtest_runs/{run_id} y dibuja lo que llega', async () => {
    const run = { ...BACKTEST_RUNS[0], run_id: 'bt-gw.1' }
    const detail: RunDetail = { run, curve: null, trades: [] }
    const fetch = vi.fn(async (input: string | URL | Request) => {
      const url = String(input)
      const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
      if (url === '/api/v1/backtest_runs') return json([run])
      if (url === '/api/v1/backtest_runs/bt-gw.1') return json(detail)
      return new Response('', { status: 404 })
    })
    vi.stubGlobal('fetch', fetch)
    render(<PerformanceView onOpen={() => {}} />)
    await screen.findByText('1 corrida del motor')
    open('bt-gw.1')
    const region = await detailOf('bt-gw.1')
    expect(fetch).toHaveBeenCalledWith('/api/v1/backtest_runs/bt-gw.1', expect.anything())
    expect(region).toHaveTextContent('Simulación · backtest')
    expect(region).not.toHaveTextContent('datos de ejemplo')
    // una corrida anterior a #299: sin curva ni comprar y mantener, y lo dice
    expect(region).toHaveTextContent('no guardó su curva')
    expect(region).toHaveTextContent('no tiene comprar y mantener')
    expect(region).toHaveTextContent('no tiene operaciones guardadas')
    expect(region).toHaveTextContent('No se activó en esta corrida.')
  })

  it('si el gateway falla al pedir el detalle, lo avisa', async () => {
    const run = { ...BACKTEST_RUNS[0], run_id: 'bt-gw.2' }
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: string | URL | Request) =>
        String(input) === '/api/v1/backtest_runs'
          ? new Response(JSON.stringify([run]), { status: 200 })
          : new Response('error interno', { status: 500 }),
      ),
    )
    render(<PerformanceView onOpen={() => {}} />)
    await screen.findByText('1 corrida del motor')
    open('bt-gw.2')
    expect(await screen.findByRole('alert')).toHaveTextContent('500')
  })
})
