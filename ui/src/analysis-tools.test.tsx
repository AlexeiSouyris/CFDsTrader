import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import type { ChartAnalysis } from '@/lib/analysis-contracts'
import {
  activeAt,
  analysisLayers,
  DEFAULT_TOOLS,
  loadTools,
  saveTools,
  TOOLS,
  TOOLS_KEY,
} from '@/lib/analysis-layers'
import { GLOSSARY_BY_ID } from '@/lib/glossary'
import { chartBars, type ChartBar } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA } from '@/mocks'

const bar = (k: number): ChartBar => {
  const start = new Date(Date.UTC(2026, 10, 20, 14, 15 * k)).toISOString().replace('.000Z', 'Z')
  const end = new Date(Date.UTC(2026, 10, 20, 14, 15 * (k + 1))).toISOString().replace('.000Z', 'Z')
  return { start, end, open: '100.0', high: `10${k}.5`, low: `9${k}.5`, close: '100.5' }
}
const BARS = Array.from({ length: 6 }, (_, k) => bar(k))

/** Un bloque como lo manda el motor (los valores son de prueba: acá no se calcula nada). */
function fixture(n: number): ChartAnalysis {
  const leg = (s: number, e: number, c: number) => ({
    direction: 'up' as const,
    start_index: s,
    start_price: '100.0',
    end_index: e,
    end_price: '110.0',
    confirmed_index: c,
    retracements: [
      { ratio: '0.5', price: '105.0' },
      { ratio: '0.618', price: '103.82' },
    ],
    extensions: [{ ratio: '1.618', price: '116.18' }],
    golden_pocket_low: '103.5',
    golden_pocket_high: '103.82',
  })
  const pivots = (day: string, from: number) => ({
    day,
    from_index: from,
    pp: '100',
    r1: '101',
    r2: '102',
    r3: '103',
    s1: '99',
    s2: '98',
    s3: '97',
  })
  return {
    ema_daily_period: 365,
    ema_daily: Array.from({ length: n }, (_, i) => (i === 2 ? null : `99.${i}`)),
    fibonacci: [leg(0, 1, 2), leg(2, 3, 4), leg(0, 1, 99)],
    golden_pocket: [
      { index: 3, leg: 0, touches: true, closes_inside: true },
      { index: 5, leg: 1, touches: true, closes_inside: false },
      { index: 5, leg: 0, touches: false, closes_inside: false },
    ],
    wyckoff: [
      {
        kind: 'spring',
        break_index: 1,
        confirmed_index: 2,
        level: '95.0',
        touches: 3,
        extreme: '94.5',
      },
      {
        kind: 'upthrust',
        break_index: 3,
        confirmed_index: 4,
        level: '104.0',
        touches: 4,
        extreme: '104.5',
      },
    ],
    bottoms: [{ index: 1, price: '91.5', prior_high: '104', rebound_index: 2, confirmed_index: 3 }],
    ma_cross: {
      kind: 'sma',
      fast_period: 50,
      slow_period: 200,
      fast: Array.from({ length: n }, () => null),
      slow: Array.from({ length: n }, () => null),
      crosses: [
        { kind: 'golden', day: '2026-11-19', known_from_index: 1 },
        { kind: 'death', day: '2026-11-20', known_from_index: 5 },
      ],
    },
    floor_pivots: [pivots('2026-11-19', 0), pivots('2026-11-20', 4)],
    vix: Array.from({ length: n }, (_, i) =>
      i === n - 1 ? { close: '21.5', regime: 'elevated' as const } : null,
    ),
  }
}

describe('mapeo del bloque analysis (#501): índices del motor → cierres de vela', () => {
  const layers = analysisLayers(fixture(6), BARS)

  it('la EMA se corta donde el motor manda null y cada punto va al cierre de su vela', () => {
    expect(layers.emaPeriod).toBe(365)
    expect(layers.ema.map((r) => r.length)).toEqual([2, 3])
    expect(layers.ema[0][0]).toEqual({ at: BARS[0].end, price: '99.0' })
    expect(layers.ema[1][0]).toEqual({ at: BARS[3].end, price: '99.3' })
  })

  it('cada tramo de Fibonacci vale desde su confirmación hasta la del siguiente', () => {
    // el tercero tiene un índice fuera de las velas: se descarta
    expect(layers.fibonacci).toHaveLength(2)
    const [a, b] = layers.fibonacci
    expect(a.from).toBe(BARS[2].end)
    expect(a.to).toBe(BARS[4].end)
    expect(b.to).toBeNull()
    expect(a.lines.map((l) => [l.kind, l.ratio, l.price])).toEqual([
      ['retracement', '0.5', '105.0'],
      ['retracement', '0.618', '103.82'],
      ['extension', '1.618', '116.18'],
    ])
    expect(a.pocket).toEqual({ low: '103.5', high: '103.82' })
    // solo las velas que tocan, en su tramo
    expect(a.events).toEqual([{ at: BARS[3].end, closesInside: true }])
    expect(b.events).toEqual([{ at: BARS[5].end, closesInside: false }])
  })

  it('spring, fondo y cruce dorado van debajo de la vela de la señal; upthrust y de la muerte, arriba', () => {
    const by = Object.fromEntries(layers.markers.map((m) => [m.kind, m]))
    expect(by.spring).toMatchObject({ at: BARS[2].end, side: 'below', anchor: BARS[2].low })
    expect(by.upthrust).toMatchObject({ at: BARS[4].end, side: 'above', anchor: BARS[4].high })
    expect(by.bottom).toMatchObject({ at: BARS[3].end, side: 'below' })
    expect(by.golden).toMatchObject({ at: BARS[1].end, side: 'below' })
    expect(by.death).toMatchObject({ at: BARS[5].end, side: 'above' })
    expect(by.spring.detail).toBe('nivel 95,0 (3 toques), mínimo 94,5')
    expect(by.golden.detail).toContain('SMA 50 y 200')
  })

  it('los pivotes diarios valen desde su primera vela hasta el día siguiente', () => {
    expect(layers.pivots.map((p) => [p.day, p.from, p.to])).toEqual([
      ['2026-11-19', BARS[0].end, BARS[4].end],
      ['2026-11-20', BARS[4].end, null],
    ])
    expect(layers.pivots[0].levels.map((l) => l.name)).toEqual([
      'R3',
      'R2',
      'R1',
      'P',
      'S1',
      'S2',
      'S3',
    ])
  })

  it('el VIX es el de la última vela, tal como llega', () => {
    expect(layers.vix).toEqual({ close: '21.5', regime: 'elevated' })
    expect(analysisLayers({ ...fixture(6), vix: [] }, BARS).vix).toBeNull()
  })

  it('activeAt elige la entrada que vale en ese cierre', () => {
    const t = (i: number) => Date.parse(BARS[i].end)
    expect(activeAt(layers.pivots, t(0))?.day).toBe('2026-11-19')
    expect(activeAt(layers.pivots, t(5))?.day).toBe('2026-11-20')
    expect(activeAt(layers.fibonacci, t(1))).toBeNull()
    expect(activeAt(layers.fibonacci, t(3))?.from).toBe(BARS[2].end)
  })
})

describe('preferencia de herramientas (#501)', () => {
  beforeEach(() => localStorage.clear())
  afterEach(() => vi.restoreAllMocks())

  it('por defecto solo la EMA está prendida', () => {
    expect(loadTools()).toEqual(DEFAULT_TOOLS)
    expect(Object.entries(DEFAULT_TOOLS).filter(([, on]) => on)).toEqual([['ema', true]])
  })

  it('guarda y lee; ignora lo roto o desconocido', () => {
    saveTools({ ...DEFAULT_TOOLS, fibonacci: true, ema: false })
    expect(loadTools()).toMatchObject({ fibonacci: true, ema: false, vix: false })
    localStorage.setItem(TOOLS_KEY, '{"vix": "sí", "otra": true}')
    expect(loadTools()).toEqual(DEFAULT_TOOLS)
    localStorage.setItem(TOOLS_KEY, '{roto')
    expect(loadTools()).toEqual(DEFAULT_TOOLS)
  })

  it('si el navegador no deja usar localStorage, anda igual con las de siempre', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    expect(loadTools()).toEqual(DEFAULT_TOOLS)
    expect(() => saveTools({ ...DEFAULT_TOOLS, vix: true })).not.toThrow()
  })

  it('cada herramienta tiene su definición en el glosario', () => {
    for (const t of TOOLS) expect(GLOSSARY_BY_ID.get(t.term), t.term).toBeDefined()
  })
})

describe('capas en el gráfico (#501)', () => {
  beforeEach(() => localStorage.clear())
  const bars = chartBars(DATA)
  const draw = (analysis?: ChartAnalysis) =>
    render(
      <LiquidityChart
        instrument="US100"
        bars={bars}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
        analysis={analysis}
      />,
    )
  const menu = () => {
    fireEvent.click(screen.getByRole('button', { name: /Herramientas/ }))
    return screen.getByRole('dialog', { name: 'Herramientas de análisis' })
  }

  it('con la EMA prendida por defecto y el resto apagado; los toggles prenden cada capa y se recuerdan', () => {
    const n = bars.length
    const analysis = {
      ...fixture(n),
      fibonacci: [
        {
          ...fixture(n).fibonacci[0],
          start_index: n - 20,
          end_index: n - 12,
          confirmed_index: n - 9,
          // dentro de la escala del gráfico, para que su rótulo vaya al eje
          retracements: [
            { ratio: '0.5', price: bars[n - 1].high },
            { ratio: '0.618', price: bars[n - 1].close },
          ],
        },
      ],
      golden_pocket: [{ index: n - 4, leg: 0, touches: true, closes_inside: true }],
    }
    const { container } = draw(analysis)
    expect(container.querySelectorAll('[data-ema]').length).toBeGreaterThan(0)
    expect(container.querySelector('[data-fib-leg]')).toBeNull()
    expect(container.querySelector('[data-marker]')).toBeNull()
    expect(container.querySelector('[data-pivots]')).toBeNull()
    expect(container.querySelector('[data-vix]')).toBeNull()

    const m = menu()
    expect(m.textContent).not.toMatch(/compr|vend|gananc/i)
    fireEvent.click(within(m).getByRole('checkbox', { name: 'Fibonacci y golden pocket' }))
    expect(container.querySelector('[data-golden-pocket]')).not.toBeNull()
    expect(container.querySelectorAll('[data-fib-ratio]').length).toBe(3)
    expect(container.querySelector('[data-pocket-touch="inside"]')).not.toBeNull()
    // los rótulos del tramo van al eje, con la razón tal como llega
    expect(screen.getAllByText('F 0,618').length).toBeGreaterThan(0)

    fireEvent.click(within(m).getByRole('checkbox', { name: 'Spring y upthrust' }))
    expect(container.querySelector('[data-marker="spring"]')).not.toBeNull()
    expect(container.querySelector('[data-marker="bottom"]')).toBeNull()
    fireEvent.click(within(m).getByRole('checkbox', { name: 'Puntos pivote diarios' }))
    expect(container.querySelectorAll('[data-pivots]').length).toBe(2)
    fireEvent.click(within(m).getByRole('checkbox', { name: 'Régimen del VIX' }))
    expect(container.querySelector('[data-vix="elevated"]')?.textContent).toBe('VIX 21,5 · elevado')

    fireEvent.click(within(m).getByRole('checkbox', { name: 'EMA diaria' }))
    expect(container.querySelector('[data-ema]')).toBeNull()
    expect(JSON.parse(localStorage.getItem(TOOLS_KEY)!)).toMatchObject({
      ema: false,
      fibonacci: true,
      wyckoff: true,
      pivots: true,
      vix: true,
      bottoms: false,
    })
  })

  it('los nombres del menú abren la definición del glosario', () => {
    draw(fixture(bars.length))
    const m = menu()
    fireEvent.click(within(m).getByRole('button', { name: 'Precio de fondo' }))
    expect(screen.getByRole('dialog', { name: 'Precio de fondo' }).textContent).toContain(
      'No es un término técnico estándar',
    )
  })

  it('sin bloque analysis, el menú avisa que este gráfico no las trae', () => {
    const { container } = draw()
    expect(within(menu()).getByRole('note').textContent).toMatch(/15 m del CFD de IG/)
    expect(container.querySelector('[data-ema]')).toBeNull()
  })

  it('Escape cierra el menú', () => {
    draw(fixture(bars.length))
    menu()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('dialog', { name: 'Herramientas de análisis' })).toBeNull()
  })
})
