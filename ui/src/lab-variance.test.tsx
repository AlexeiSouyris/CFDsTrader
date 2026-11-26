/**
 * Simulador de varianza del laboratorio (#806): arma `variance_sim_request.v1` desde el formulario
 * (porcentaje → fracción exacta, sin float), no pide nada hasta tener lo obligatorio y muestra lo
 * que calcula el motor sin hacer cuentas. El informe es uno real del motor
 * (`src/mocks/variance-report.json`, generado con `research::variance`).
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { VarianceSimulator } from '@/components/ui/lab-variance'
import type { VarianceReport, VarianceRequest } from '@/lib/sim-contracts'
import { pctToFraction } from '@/lib/sim-form'
import reportJson from '@/mocks/variance-report.json'

const REPORT = reportJson as unknown as VarianceReport

function gateway(answer: (req: VarianceRequest) => Response) {
  const sent: VarianceRequest[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit = {}) => {
      if (url !== '/api/v1/lab/variance') return new Response('no', { status: 501 })
      const req = JSON.parse(String(init.body)) as VarianceRequest
      sent.push(req)
      return answer(req)
    }),
  )
  return sent
}

const ok = () =>
  new Response(JSON.stringify(REPORT), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
  })

const type = (label: RegExp, value: string) =>
  fireEvent.change(screen.getByLabelText(label), { target: { value } })

afterEach(() => vi.unstubAllGlobals())

describe('porcentaje a fracción', () => {
  it('mueve la coma dos lugares sin float', () => {
    expect(pctToFraction('45')).toBe('0.45')
    expect(pctToFraction('45,5')).toBe('0.455')
    expect(pctToFraction('5')).toBe('0.05')
    expect(pctToFraction('100')).toBe('1.00')
    expect(pctToFraction('0.1')).toBe('0.001')
    expect(pctToFraction('abc')).toBeNull()
    expect(pctToFraction('')).toBeNull()
  })
})

describe('el simulador de varianza', () => {
  it('no pide nada hasta tener lo obligatorio y manda el pedido del contrato', async () => {
    const sent = gateway(ok)
    render(<VarianceSimulator />)
    const run = screen.getByRole('button', { name: 'Simular' })
    expect(run).toBeDisabled()
    type(/Win rate/, '45')
    type(/R medio ganador/, '1,5')
    type(/^Riesgo/, '2')
    type(/^Operaciones$/, '300')
    expect(run).toBeDisabled() // falta cuántas operaciones observadas (incertidumbre prendida)
    type(/Operaciones observadas/, '60')
    expect(run).toBeEnabled()
    fireEvent.click(run)
    await waitFor(() => expect(sent).toHaveLength(1))
    expect(sent[0]).toEqual({
      source: {
        kind: 'model',
        win_rate: '0.45',
        avg_win_r: '1.5',
        avg_loss_r: '1',
        observed_trades: 60,
      },
      risk_pct: '2',
      trades: 300,
    })
    expect(await screen.findByRole('img', { name: /Bandas de capital/ })).toBeInTheDocument()
  })

  it('muestra los números del motor tal cual, con la comparación de la incertidumbre', async () => {
    gateway(ok)
    render(<VarianceSimulator />)
    type(/Win rate/, '45')
    type(/R medio ganador/, '1.5')
    type(/^Riesgo/, '2')
    type(/^Operaciones$/, '300')
    type(/Operaciones observadas/, '60')
    fireEvent.click(screen.getByRole('button', { name: 'Simular' }))
    const result = await screen.findByText(/Incertidumbre del win rate/)
    // 0.236 del motor → 23,6 %; con el win rate fijo, 0.0625 → 6,2 % (el formato corta, no redondea).
    expect(result.parentElement).toHaveTextContent('23,6 %')
    expect(result.parentElement).toHaveTextContent('6,2 %')
    const ruin = screen.getByText('Riesgo de ruina').parentElement!
    expect(within(ruin).getByText(/23,6 % \(21,7–25,5 %\)/)).toBeInTheDocument()
    expect(screen.getByText('Capital final, mediana').nextSibling).toHaveTextContent('×1,88')
    for (const n of REPORT.notes) expect(screen.getByText(`· ${n}`)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /ESMA/ })).toHaveAttribute(
      'href',
      expect.stringContaining('esma.europa.eu'),
    )
  })

  it('sin incertidumbre manda observed_trades null y los opcionales solo si están', async () => {
    const sent = gateway(ok)
    render(<VarianceSimulator />)
    type(/Win rate/, '50')
    type(/R medio ganador/, '1')
    type(/^Riesgo/, '1')
    type(/^Operaciones$/, '100')
    fireEvent.click(screen.getByRole('checkbox'))
    type(/Caminos/, '500')
    type(/Semilla/, '7')
    type(/Ruina/, '50')
    fireEvent.click(screen.getByRole('button', { name: 'Simular' }))
    await waitFor(() => expect(sent).toHaveLength(1))
    expect(sent[0]).toMatchObject({
      source: { kind: 'model', win_rate: '0.50', observed_trades: null },
      paths: 500,
      seed: 7,
      ruin_threshold_pct: '50',
    })
  })

  it('con informes de una corrida puede usar sus R reales', async () => {
    const sent = gateway(ok)
    const ids = ['a'.repeat(64), 'b'.repeat(64)]
    render(<VarianceSimulator reports={ids} />)
    fireEvent.click(screen.getByRole('button', { name: 'R de un informe' }))
    fireEvent.change(screen.getByLabelText(/Informe del laboratorio/), {
      target: { value: ids[1] },
    })
    type(/^Riesgo/, '1')
    type(/^Operaciones$/, '200')
    fireEvent.click(screen.getByRole('button', { name: 'Simular' }))
    await waitFor(() => expect(sent).toHaveLength(1))
    expect(sent[0]).toEqual({
      source: { kind: 'report', report_id: ids[1] },
      risk_pct: '1',
      trades: 200,
    })
  })

  it('muestra el error del motor sin inventar resultados', async () => {
    gateway(() => new Response('risk_pct tiene que estar en (0, 10]', { status: 422 }))
    render(<VarianceSimulator />)
    type(/Win rate/, '45')
    type(/R medio ganador/, '1.5')
    type(/^Riesgo/, '50')
    type(/^Operaciones$/, '100')
    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(screen.getByRole('button', { name: 'Simular' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('422')
    expect(screen.queryByRole('img', { name: /Bandas/ })).toBeNull()
  })
})
