/**
 * Calculadora de riesgo del laboratorio (#795): arma el pedido de `risk_preview.v1` desde el
 * catálogo, espera un momento después del último cambio y muestra lo que calcula el motor (no hace
 * cuentas). El catálogo es el real del motor (`src/mocks/lab-catalog.json`).
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { RiskCalculator } from '@/components/ui/lab-risk'
import type { RiskPreview, RiskPreviewRequest, StrategyCatalog } from '@/lib/lab-contracts'
import catalogJson from '@/mocks/lab-catalog.json'

const CATALOG = catalogJson as unknown as StrategyCatalog

/** Una respuesta del motor con números de ejemplo (no de mercado). */
const preview = (over: Partial<RiskPreview> = {}): RiskPreview => ({
  instrument_id: 'DE40',
  timeframe: '15m',
  direction: 'long',
  as_of: '2026-11-26T06:00:00Z',
  reference: '5000',
  atr: '10',
  atr_period: 14,
  stop: '4985',
  stop_atr: '1.5',
  equity: '100000',
  risk_pct_requested: '1',
  risk_pct_applied: '1',
  accepted: true,
  rejection: null,
  size: '57.1428',
  notional: '285714',
  leverage: '2.857',
  margin: '14285.7',
  risk: '999.99',
  risk_pct: '0.99999',
  leverage_capped: false,
  group_capped: false,
  round_trip_cost: '120.5',
  account_currency: 'EUR',
  instrument_currency: 'EUR',
  fx_rate: '1',
  limits: {
    max_risk_pct: '1',
    min_stop_atr: '0.5',
    max_stop_atr: '3',
    gap_atr: '0.25',
    max_leverage: '10',
    margin_pct: '5',
  },
  notes: ['precio de referencia: cierre de la última vela cerrada'],
  ...over,
})

function gateway(answer: (req: RiskPreviewRequest) => RiskPreview) {
  const sent: RiskPreviewRequest[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      if (url !== '/api/v1/lab/risk_preview') return new Response('no', { status: 501 })
      const req = JSON.parse(String(init.body)) as RiskPreviewRequest
      sent.push(req)
      return new Response(JSON.stringify(answer(req)), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      })
    }),
  )
  return sent
}

afterEach(() => vi.unstubAllGlobals())

describe('la calculadora de riesgo', () => {
  it('no pide nada hasta tener capital, y después pide una sola vez por cambio', async () => {
    const sent = gateway(() => preview())
    render(<RiskCalculator cat={CATALOG} />)
    expect(screen.getByText(/Escribí el capital/)).toBeInTheDocument()
    const capital = screen.getByLabelText(/Capital \(EUR/)
    for (const v of ['1', '10', '100', '1000', '100000'])
      fireEvent.change(capital, { target: { value: v } })
    const result = await screen.findByText('Pérdida si salta el stop')
    expect(sent).toHaveLength(1)
    const atr = CATALOG.blocks.find((b) => b.type === 'atr')!.params
    expect(sent[0]).toEqual({
      instrument_id: CATALOG.markets[0].instrument_id,
      timeframe: CATALOG.markets[0].timeframes[0],
      direction: 'long',
      equity: '100000',
      risk_pct: CATALOG.limits.max_risk_pct,
      atr_period: Number(atr.find((p) => p.name === 'period')!.default),
      stop: { kind: 'atr', k: String(atr.find((p) => p.name === 'k')!.default) },
    })
    const dl = result.closest('dl')!
    expect(within(dl).getByText(/999,99 EUR/)).toBeInTheDocument()
    expect(screen.getByText(/precio de referencia/)).toBeInTheDocument()
  })

  it('con stop a precio y corto, y muestra por qué rechaza', async () => {
    const sent = gateway(() =>
      preview({
        accepted: false,
        rejection: 'stop_too_close',
        size: null,
        notional: null,
        leverage: null,
        margin: null,
        risk: null,
        risk_pct: null,
        round_trip_cost: null,
      }),
    )
    render(<RiskCalculator cat={CATALOG} />)
    fireEvent.click(screen.getByRole('button', { name: 'Corto' }))
    fireEvent.click(screen.getByRole('button', { name: 'Precio' }))
    fireEvent.change(screen.getByLabelText('Precio del stop'), { target: { value: '5001,5' } })
    fireEvent.change(screen.getByLabelText(/Capital \(EUR/), { target: { value: '5000' } })
    expect(await screen.findByText(/El stop está demasiado cerca/)).toBeInTheDocument()
    await waitFor(() => expect(sent.length).toBeGreaterThan(0))
    expect(sent.at(-1)).toMatchObject({
      direction: 'short',
      stop: { kind: 'price', price: '5001.5' },
    })
  })

  it('avisa el error del gateway', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('escritura apagada', { status: 403 })),
    )
    render(<RiskCalculator cat={CATALOG} />)
    fireEvent.change(screen.getByLabelText(/Capital \(EUR/), { target: { value: '5000' } })
    expect(await screen.findByText(/403/)).toBeInTheDocument()
  })
})
