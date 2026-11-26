/**
 * Simulador de cuenta fondeada (#807): la plantilla genérica en blanco del motor, el pedido de
 * `prop_sim_request.v1` (blancos como null, el motor dice qué falta con su ruta), los errores en su
 * campo y el resultado del motor tal cual, con la comparación contra el azar y las cifras con
 * fuente. Plantilla y resultados reales del motor (`src/mocks/propsim-*.json`).
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PropSimulator } from '@/components/ui/lab-propsim'
import { requestLabTool } from '@/lib/lab-focus'
import type { PropRules, PropSimRequest, PropSimResult } from '@/lib/sim-contracts'
import resultJson from '@/mocks/propsim-result.json'
import tradesJson from '@/mocks/propsim-result-trades.json'
import templatesJson from '@/mocks/propsim-templates.json'

const TEMPLATES = templatesJson as unknown as PropRules[]
const RESULT = resultJson as unknown as PropSimResult
const TRADES = tradesJson as unknown as PropSimResult

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })

function gateway(answer: (req: PropSimRequest) => Response) {
  const sent: PropSimRequest[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      if (url === '/api/v1/lab/propsim/templates') return json(TEMPLATES)
      if (url !== '/api/v1/lab/propsim') return new Response('no', { status: 501 })
      const req = JSON.parse(String(init.body)) as PropSimRequest
      sent.push(req)
      return answer(req)
    }),
  )
  return sent
}

const type = (label: RegExp | string, value: string) =>
  fireEvent.change(screen.getByLabelText(label), { target: { value } })

/** Completa las operaciones del modelo y el riesgo (lo que exige el contrato). */
function operations() {
  type(/Win rate/, '45')
  type(/R medio ganador/, '1,5')
  type(/Operaciones por día/, '2')
  type(/Operaciones observadas/, '60')
  type(/Riesgo \(% del tamaño\)/, '0,5')
}

afterEach(() => vi.unstubAllGlobals())

describe('el simulador de cuenta fondeada', () => {
  it('arranca con la plantilla genérica en blanco y lo dice', async () => {
    gateway(() => json(RESULT))
    render(<PropSimulator />)
    expect(await screen.findByText(/no trae las reglas de ninguna firma real/)).toBeInTheDocument()
    expect(screen.getByLabelText('Tamaño')).toHaveValue('')
    expect(screen.getByLabelText('Objetivo (%)')).toHaveValue('')
    const run = screen.getByRole('button', { name: 'Simular' })
    expect(run).toBeDisabled()
    operations()
    expect(run).toBeEnabled()
    // Las cifras con fuente: Topstep 2025 y la ESMA.
    expect(screen.getByText(/16,8 %/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /Topstep/ })).toHaveAttribute(
      'href',
      'https://www.topstep.com/risk-disclosure',
    )
    expect(screen.getByRole('link', { name: /ESMA/ })).toHaveAttribute(
      'href',
      expect.stringContaining('esma.europa.eu'),
    )
    expect(screen.getByText(/queda fuera de esta app/)).toBeInTheDocument()
  })

  it('manda los blancos como null y marca en cada campo lo que dice el motor', async () => {
    const sent = gateway(() =>
      json(
        {
          error: 'reglas incompletas o inválidas',
          rules: [
            { path: '/account_size', message: 'falta este valor para simular' },
            { path: '/phases/0/profit_target_pct', message: 'falta este valor para simular' },
          ],
        },
        422,
      ),
    )
    render(<PropSimulator />)
    await screen.findByLabelText('Tamaño')
    operations()
    fireEvent.click(screen.getByRole('button', { name: 'Simular' }))
    await waitFor(() => expect(sent).toHaveLength(1))
    expect(sent[0].rules.account_size).toBeNull()
    expect(sent[0].rules.phases[0].profit_target_pct).toBeNull()
    expect(sent[0].rules.daily_loss).toEqual({ limit_pct: null, base: 'day_start' })
    expect(sent[0].source).toEqual({
      kind: 'model',
      win_rate: '0.45',
      avg_win_r: '1.5',
      avg_loss_r: '1',
      observed_trades: 60,
      trades_per_day: 2,
    })
    expect(await screen.findByRole('alert')).toHaveTextContent('revisá los campos marcados')
    expect(screen.getByLabelText('Tamaño')).toHaveAttribute('aria-invalid', 'true')
    expect(screen.getByLabelText('Objetivo (%)')).toHaveAttribute('aria-invalid', 'true')
    expect(screen.getAllByText('falta este valor para simular')).toHaveLength(2)
  })

  it('arma las reglas completas y muestra el resultado del motor tal cual', async () => {
    const sent = gateway(() => json(RESULT))
    render(<PropSimulator />)
    await screen.findByLabelText('Tamaño')
    type('Tamaño', '50000')
    type('Moneda', 'usd')
    type('Zona del corte del día', 'America/New_York')
    type('Hora del corte', '17:00')
    type('Objetivo (%)', '6')
    type('Días mínimos', '2')
    type('Pérdida diaria (%)', '2')
    type('Caída máxima (%)', '4')
    type('Reparto (%)', '90')
    type('Cuota por intento', '150')
    operations()
    fireEvent.click(screen.getByRole('button', { name: 'Simular' }))
    await waitFor(() => expect(sent).toHaveLength(1))
    expect(sent[0]).toMatchObject({
      risk_per_trade_pct: '0.5',
      rules: {
        id: 'generica',
        account_size: '50000',
        currency: 'USD',
        day: { timezone: 'America/New_York', cutoff: '17:00' },
        phases: [{ profit_target_pct: '6', min_trading_days: 2, max_trading_days: null }],
        funded: { profit_split_pct: '90' },
        fees: { evaluation: '150', activation: null, refund_with_first_payout: false },
      },
    })
    expect(sent[0].monte_carlo).toBeUndefined()
    const result = await screen.findByText(/en el pasado simulado/)
    // 0.475 cobró; al azar 0.1645.
    expect(result).toHaveTextContent('47,5 %')
    expect(result).toHaveTextContent('16,4 %')
    expect(screen.getByText(/le gana al azar/)).toBeInTheDocument()
    const ev = screen.getByText('Valor esperado del programa').nextSibling
    expect(ev).toHaveTextContent('521,45 USD')
    expect(screen.getByText('Valor esperado operando al azar').nextSibling).toHaveTextContent(
      '−165,94 USD',
    )
    const table = screen.getByRole('table')
    expect(within(table).getByText('Cuenta fondeada (primer cobro)')).toBeInTheDocument()
    for (const n of RESULT.notes) expect(screen.getByText(`· ${n}`)).toBeInTheDocument()
  })

  it('la caída intradía avisa que no es simulable y el resultado dice tope superior', async () => {
    gateway(() =>
      json({ ...TRADES, upper_bound: true, unsimulable: ['max_drawdown.trailing_intraday'] }),
    )
    render(<PropSimulator />)
    await screen.findByLabelText('Tamaño')
    fireEvent.click(screen.getByRole('button', { name: 'sigue intradía' }))
    expect(
      screen.getByText(/No simulable: hace falta el capital vela por vela/),
    ).toBeInTheDocument()
    operations()
    fireEvent.click(screen.getByRole('button', { name: 'Simular' }))
    expect(await screen.findByText(/Tope superior/)).toBeInTheDocument()
    // Con una lista real: el recorrido de tus operaciones, en orden.
    expect(screen.getByText('Tus operaciones, en orden')).toBeInTheDocument()
    expect(screen.getByText(/Cuenta fondeada \(primer cobro\): /)).toHaveTextContent('Cobró')
  })

  it('un texto que no es un número no se manda', async () => {
    const sent = gateway(() => json(RESULT))
    render(<PropSimulator />)
    await screen.findByLabelText('Tamaño')
    operations()
    type('Tamaño', 'cincuenta mil')
    expect(screen.getByText('No es un número.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Simular' })).toBeDisabled()
    expect(sent).toHaveLength(0)
  })

  it('la Academia lo pide y el simulador toma el foco', async () => {
    gateway(() => json(RESULT))
    requestLabTool('propsim')
    render(<PropSimulator />)
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'Simulador de cuenta fondeada' })).toHaveFocus(),
    )
  })
})
