import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AnalogDistribution, SentimentGauge } from '@/components/ui/market-context'
import { roundedText } from '@/components/ui/rounded-text'
import type { AnalogReport, EmotionalState, SentimentIndex } from '@/lib/context-contracts'
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

/** Un gateway falso con las lecturas del contexto. */
function gateway(analogs: AnalogReport[], emotions: EmotionalState[] = []) {
  return vi.fn(async (input: string | URL | Request) => {
    const url = new URL(String(input), 'http://ui.test')
    const inst = url.searchParams.get('instrument')
    let body: unknown
    if (url.pathname === '/api/v1/analogs') body = analogs
    else if (url.pathname === '/api/v1/emotions') body = emotions
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

  it('el índice largo se escribe con dos decimales, sin pasar por float (#357)', () => {
    expect(roundedText('-0.3975550788299792283856028877')).toBe('-0.40')
    expect(roundedText('0.335648857561208158817720571')).toBe('0.34')
    expect(roundedText('-0.004')).toBe('0.00')
    expect(roundedText('0.995')).toBe('1.00')
    expect(roundedText('12345678901234567890.125')).toBe('12345678901234567890.13')
    expect(roundedText('-3.2306', 4)).toBe('-3.2306')
    expect(roundedText('-0.35')).toBe('-0.35')
    expect(roundedText('abc')).toBe('abc')
    render(<SentimentGauge index={sentiment('US100.FUT', '-0.3975550788299792283856028877')} />)
    expect(screen.getByText('−0,40')).toBeInTheDocument()
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuetext', '−0,40')
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

  it('sin elegir, abre en el primer instrumento con índice y no en uno vacío (#357)', async () => {
    // DE40 solo tiene análogos y va primero en orden alfabético; el índice está en US100.FUT.
    vi.stubGlobal('fetch', gateway([report('DE40', 20), report('US100.FUT', 20)]))
    render(<MarketContextView />)
    expect(await screen.findByRole('meter', { name: /US100.FUT/ })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'US100.FUT' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('si una lectura falla lo dice en su tarjeta, sin decir que no hay datos (#357)', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: string | URL | Request) => {
        const url = new URL(String(input), 'http://ui.test')
        if (url.pathname === '/api/v1/analogs')
          return new Response(JSON.stringify([report('US100.FUT', 20)]), { status: 200 })
        return new Response('boom', { status: 500 })
      }),
    )
    render(<MarketContextView />)
    expect(await screen.findByText(/No se pudo leer el índice del motor/)).toBeInTheDocument()
    expect(screen.getByText(/No se pudieron leer los estados emocionales/)).toBeInTheDocument()
    expect(screen.queryByText('Sin índice guardado para este instrumento.')).not.toBeInTheDocument()
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

  it('los estados emocionales van al lado del índice, con los valores de su regla (#220)', async () => {
    // Datos sintéticos con la forma de emotional_state.v1, tal como los calcularía el motor.
    const emotions: EmotionalState[] = [
      {
        instrument_id: 'US500.FUT',
        source: 'price',
        basis: '1h',
        as_of: '2026-11-20T21:00:00Z',
        states: [
          {
            state: 'selling_climax',
            values: {
              close: '5810.5',
              ema: '5902.25',
              atr: '28.4',
              stretch: '-3.2306',
              atr_multiple: '3',
              lookback: '20',
            },
          },
          { state: 'rejection', values: { side: 'long', via: 'level,sweep' } },
        ],
      },
      {
        instrument_id: 'US500.FUT',
        source: 'price',
        basis: '15m',
        as_of: '2026-11-20T21:15:00Z',
        states: [],
      },
      {
        instrument_id: 'US500.FUT',
        source: 'sentiment',
        basis: 'sentiment_index',
        as_of: '2026-11-20T23:00:00Z',
        states: [{ state: 'extreme_fear', values: { index: '-0.72', threshold: '-0.6' } }],
      },
      {
        instrument_id: 'US100.FUT',
        source: 'price',
        basis: '1h',
        as_of: '2026-11-20T21:00:00Z',
        states: [],
      },
    ]
    const fetch = gateway([], emotions)
    vi.stubGlobal('fetch', fetch)
    render(<MarketContextView />)
    // Primero US100.FUT: sin estados activos.
    const list = await screen.findByLabelText('Estados emocionales')
    expect(list).toHaveTextContent('Ninguno activo.')
    fireEvent.click(screen.getByRole('button', { name: 'US500.FUT' }))
    await screen.findByText('Miedo extremo')
    const all = screen.getByLabelText('Estados emocionales')
    // El índice primero, después 1h y 15m.
    expect(
      [...all.querySelectorAll('[data-basis]')].map((s) => s.getAttribute('data-basis')),
    ).toEqual(['sentiment_index', '1h', '15m'])
    expect(all.querySelector('[data-state="extreme_fear"]')).toHaveTextContent(
      'índice −0,72 ≤ −0,6',
    )
    const climax = all.querySelector('[data-state="selling_climax"]')!
    // Los valores tal como llegan, solo con coma: la UI no los recalcula.
    expect(climax).toHaveTextContent('(cierre − EMA) / ATR = −3,2306 (umbral 3)')
    expect(climax).toHaveTextContent('cierre 5.810,5, EMA 5.902,25, ATR 28,4')
    expect(climax).toHaveTextContent('mínimo de 20 velas')
    expect(all.querySelector('[data-state="rejection"]')).toHaveTextContent(
      'alcista · por soporte o resistencia, sweep',
    )
    expect(screen.getByText(/no un pronóstico/)).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith('/api/v1/emotions', expect.anything())
  })

  it('sin estados guardados lo dice', async () => {
    vi.stubGlobal('fetch', gateway([]))
    render(<MarketContextView />)
    expect(
      await screen.findByText('Sin estados guardados para este instrumento.'),
    ).toBeInTheDocument()
  })

  it('el porcentaje con signo mueve la coma en el texto', () => {
    expect(signedPercent('-0.031')).toBe('−3,1')
    expect(signedPercent('0.004')).toBe('+0,4')
    expect(signedPercent('0')).toBe('0,0')
    expect(signedPercent('-0.0001')).toBe('0,0')
  })
})
