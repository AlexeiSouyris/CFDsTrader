import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AnalogDistribution, SentimentGauge } from '@/components/ui/market-context'
import type { AnalogReport, SentimentIndex } from '@/lib/context-contracts'
import { signedPercent } from '@/lib/format'
import { MarketContextView } from '@/views/market-context'

// Números sintéticos, con la forma de sentiment.v1 y analog_report.v1.
const sentiment = (id: string, value: string, day = '20'): SentimentIndex => ({
  instrument_id: id,
  as_of: `2026-11-${day}T21:00:00Z`,
  published_at: `2026-11-${day}T23:00:00Z`,
  value,
  warmup_complete: true,
  min_components: 3,
  components: [
    {
      name: 'cot',
      status: 'valid',
      weight: '1',
      observed_for: '2026-11-17T00:00:00Z',
      published_at: '2026-11-20T21:00:00Z',
      raw: '0.1',
      percentile: '0.6',
      component: '0.2',
    },
  ],
})

const report = (id: string, count: number): AnalogReport => {
  const group = (name: 'all' | 'same_context') => ({
    group: name,
    count,
    insufficient: count < 20,
    horizons: [5, 20].map((horizon) => ({
      horizon,
      percentiles: [
        { p: 10, return: '-0.031' },
        { p: 50, return: '0.004' },
        { p: 90, return: '0.027' },
      ],
      positive_pct: '55',
    })),
    hits: { up: 6, down: 9, none: 5 },
    without_atr: 0,
  })
  return {
    instrument_id: id,
    source: 'futures',
    window_end_day: '2026-11-20',
    as_of: '2026-11-20T22:00:00Z',
    computed_at: '2026-11-23T12:00:00Z',
    window: 20,
    min_neighbors: 20,
    context: { vol: 'mid', rates: 'estable' },
    neighbors: [],
    groups: [group('all'), group('same_context')],
  }
}

/** Un gateway falso con las tres lecturas del contexto. */
function gateway(analogs: AnalogReport[]) {
  return vi.fn(async (input: string | URL | Request) => {
    const url = new URL(String(input), 'http://ui.test')
    const inst = url.searchParams.get('instrument')
    let body: unknown
    if (url.pathname === '/api/v1/analogs') body = analogs
    else if (inst)
      body = [
        sentiment(inst, '-0.1', '18'),
        sentiment(inst, '-0.2', '19'),
        sentiment(inst, '-0.35'),
      ]
    else body = [sentiment('US100.FUT', '0.2'), sentiment('US500.FUT', '-0.35')]
    return new Response(JSON.stringify(body), { status: 200 })
  })
}

describe('contexto de mercado del Panel (#294)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('el medidor muestra el valor tal como llega, de −1 a +1', () => {
    render(<SentimentGauge index={sentiment('US500.FUT', '-0.35')} />)
    const meter = screen.getByRole('meter', { name: /sentimiento de US500.FUT/ })
    expect(meter).toHaveAttribute('aria-valuetext', '−0,35')
    expect(meter).toHaveAttribute('aria-valuemin', '-1')
    expect(meter).toHaveAttribute('aria-valuemax', '1')
  })

  it('los análogos muestran p10, p50, p90, % positivos y n, y dicen que no es un pronóstico', () => {
    render(<AnalogDistribution report={report('US500.FUT', 20)} />)
    expect(screen.getByText('n = 20')).toBeInTheDocument()
    expect(screen.getAllByText('−3,1 %').length).toBe(2)
    expect(screen.getAllByText('+0,4 %').length).toBe(2)
    expect(screen.getAllByText('+2,7 %').length).toBe(2)
    expect(screen.getAllByText('55 %').length).toBe(2)
    expect(screen.getByText(/No es un pronóstico ni una recomendación/)).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('con pocos vecinos avisa que la muestra es chica', () => {
    render(<AnalogDistribution report={report('US500.FUT', 7)} />)
    expect(screen.getByRole('alert')).toHaveTextContent('Muestra chica: 7 casos')
    expect(screen.getByRole('alert')).toHaveTextContent('Datos insuficientes')
  })

  it('el panel lee al gateway, elige el instrumento y muestra la historia', async () => {
    const fetch = gateway([report('US100.FUT', 20), report('US500.FUT', 7)])
    vi.stubGlobal('fetch', fetch)
    render(<MarketContextView />)
    expect(await screen.findByRole('meter', { name: /US100.FUT/ })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'US500.FUT' }))
    expect(await screen.findByRole('meter', { name: /US500.FUT/ })).toBeInTheDocument()
    expect(
      await screen.findByRole('img', { name: /Historia del índice: 3 valores/ }),
    ).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('Muestra chica')
    expect(fetch).toHaveBeenCalledWith('/api/v1/sentiment?instrument=US500.FUT', expect.anything())
  })

  it('sin análogos guardados lo dice, y ningún texto promete nada', async () => {
    vi.stubGlobal('fetch', gateway([]))
    const { container } = render(<MarketContextView />)
    const card = await screen.findByText('Sin análogos guardados para este instrumento.')
    expect(within(card.parentElement!).queryByRole('table')).not.toBeInTheDocument()
    expect(container.textContent).not.toMatch(
      /ganancia (segura|garantizada)|garantiz|asegurad|va a subir|va a bajar|comprá|vendé/i,
    )
  })

  it('el porcentaje con signo mueve la coma en el texto', () => {
    expect(signedPercent('-0.031')).toBe('−3,1')
    expect(signedPercent('0.004')).toBe('+0,4')
    expect(signedPercent('0')).toBe('0,0')
    expect(signedPercent('-0.0001')).toBe('0,0')
  })
})
