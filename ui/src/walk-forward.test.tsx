import { render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { RunDetailView } from '@/components/ui/run-detail'
import type { RunDetail } from '@/lib/backtest-contracts'
import type { WalkForwardReport, WfPeriod } from '@/lib/walk-forward-api'
import { BACKTEST_RUN_DETAIL, BACKTEST_RUNS } from '@/mocks'

/** Números de ejemplo para el test, no de mercado. */
const period = (from: string, to: string, ret: string, bh: string | null): WfPeriod => ({
  from,
  to,
  candles: 1000,
  metrics: {
    trades: 7,
    total_return: ret,
    max_drawdown: '0.0420',
    sharpe: '-0.35',
    sortino: null,
    win_rate: '0.4',
    profit_factor: '0.9',
    exposure: '0.2',
  },
  final_equity: '9876',
  buy_and_hold:
    bh === null ? null : { return: bh, net_pnl: '12', commission: '0', funding: '1', nights: 40 },
})

const REPORT: WalkForwardReport = {
  report_id: 'wf-test.1',
  instrument_id: 'DE40.FUT',
  candle_source: 'futures',
  config_sha256: 'c'.repeat(64),
  split: { train_months: 24, validation_months: 3, step_months: 3, out_of_sample_months: 12 },
  warmup_days: 30,
  initial_equity: '10000',
  buy_and_hold_size: '1',
  folds: [
    {
      index: 0,
      train: period('2022-01-01T00:00:00Z', '2024-01-01T00:00:00Z', '0.0312', '0.081'),
      test: period('2024-01-01T00:00:00Z', '2024-04-01T00:00:00Z', '-0.0125', '0.02'),
    },
    {
      index: 1,
      train: period('2022-04-01T00:00:00Z', '2024-04-01T00:00:00Z', '0.01', null),
      test: {
        from: '2024-04-01T00:00:00Z',
        to: '2024-07-01T00:00:00Z',
        candles: 0,
        metrics: null,
        final_equity: '10000',
        buy_and_hold: null,
      },
    },
  ],
  test_aggregate: {
    periods: 2,
    with_data: 1,
    trades: 7,
    mean_return: '-0.0125',
    median_return: '-0.0125',
    positive: 0,
    worst_drawdown: '0.0420',
    mean_sharpe: '-0.35',
    sharpe_defined: 1,
    beat_buy_and_hold: 0,
    with_buy_and_hold: 1,
  },
  sensitivity: [],
  stability: [
    {
      file: 'strategy.toml',
      key: 'score.min_score',
      by_value: [
        { value: '4', mean_return: '-0.19', mean_sharpe: '-0.9', trades: 559 },
        { value: '6', mean_return: '0.001', mean_sharpe: '0.06', trades: 108 },
      ],
      return_spread: '0.191',
      same_sign: false,
    },
    {
      file: 'backtest.toml',
      key: 'exits.tp1_r',
      by_value: [
        { value: '1.5', mean_return: '-0.21', mean_sharpe: '-1.4', trades: 373 },
        { value: '2', mean_return: '-0.10', mean_sharpe: null, trades: 374 },
      ],
      return_spread: '0.11',
      same_sign: true,
    },
  ],
  holdout: period('2025-11-01T00:00:00Z', '2026-11-01T00:00:00Z', '-0.1626', '0.0242'),
  computed_at: '2026-11-23T10:00:00Z',
}

const detail = (instrument: string): RunDetail => ({
  ...BACKTEST_RUN_DETAIL,
  run: { ...BACKTEST_RUNS[0], run_id: 'bt-wf.1', instruments: [instrument] },
})

function stubGateway(body: unknown, status = 200) {
  const fetch = vi.fn<typeof globalThis.fetch>(
    async () =>
      new Response(JSON.stringify(body), {
        status,
        headers: { 'Content-Type': 'application/json' },
      }),
  )
  vi.stubGlobal('fetch', fetch)
  return fetch
}

describe('walk-forward y fuera de muestra en el detalle de la corrida (#316)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('pide el último reporte del instrumento y lo muestra tal cual, con el fuera de muestra aparte', async () => {
    const fetch = stubGateway(REPORT)
    render(<RunDetailView detail={detail('DE40.FUT')} onClose={() => {}} />)
    const table = await screen.findByRole('table', { name: 'Ventanas walk-forward' })
    expect(String(fetch.mock.calls[0][0])).toBe('/api/v1/walk_forward?instrument=DE40.FUT')
    expect(document.querySelector('[data-walk-forward="ready"]')).not.toBeNull()

    // entrenamiento y prueba lado a lado, con comprar y mantener de cada tramo
    const rows = within(table).getAllByRole('row').slice(2)
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveTextContent('+3,12 %')
    expect(rows[0]).toHaveTextContent('+8,10 %')
    expect(rows[0]).toHaveTextContent('−1,25 %')
    expect(rows[0]).toHaveTextContent('+2,00 %')
    // tramo vacío: sin números inventados; sin comprar y mantener: guion
    expect(rows[1]).toHaveTextContent('sin velas')
    expect(rows[1]).toHaveTextContent('—')

    expect(screen.getByLabelText('Resumen de las pruebas')).toHaveTextContent(
      'Pruebas: 1 de 2 con datos, 7 operaciones',
    )

    // estabilidad: el cambio de signo, resaltado; nada elegido
    const stab = screen.getByRole('table', { name: 'Estabilidad de los parámetros' })
    const flagged = stab.querySelectorAll('[data-sign-change="true"]')
    expect(flagged).toHaveLength(1)
    expect(flagged[0]).toHaveTextContent('score.min_score')
    expect(flagged[0]).toHaveTextContent('cambia de signo')
    expect(stab).toHaveTextContent('mismo signo')
    expect(stab.textContent).not.toMatch(/mejor|recomendad|óptim/i)

    // el fuera de muestra, aparte
    const holdout = screen.getByRole('region', { name: 'Fuera de muestra' })
    expect(holdout).toHaveTextContent('sin retocar')
    expect(holdout).toHaveTextContent('−16,26 %')
    expect(holdout).toHaveTextContent('+2,42 %')
    expect(within(table).queryByText('−16,26 %')).toBeNull()
  })

  it('sin reporte del instrumento lo dice, sin inventar nada', async () => {
    stubGateway(null)
    render(<RunDetailView detail={detail('US500')} onClose={() => {}} />)
    const none = await screen.findByText(/Todavía no hay un reporte de walk-forward para US500/)
    expect(none).toHaveAttribute('data-walk-forward', 'none')
    expect(screen.queryByRole('table', { name: 'Ventanas walk-forward' })).toBeNull()
  })

  it('si el gateway falla queda pendiente, y con los datos de ejemplo no pide nada', async () => {
    stubGateway('error interno', 500)
    render(<RunDetailView detail={detail('DE40.FUT')} onClose={() => {}} />)
    const pending = await screen.findByText(/No se pudo leer el walk-forward de DE40.FUT/)
    expect(pending).toHaveAttribute('data-walk-forward', 'pending')
    vi.unstubAllGlobals()

    const fetch = stubGateway(REPORT)
    render(<RunDetailView detail={BACKTEST_RUN_DETAIL} example onClose={() => {}} />)
    expect(screen.getAllByText(/no tienen walk-forward/)[0]).toHaveAttribute(
      'data-walk-forward',
      'pending',
    )
    expect(fetch).not.toHaveBeenCalled()
  })
})
