import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { placeLabels } from '@/components/ui/chart-measure'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { FvgTable, ImbalanceGauge, LevelsTable, Rate } from '@/components/ui/liquidity-panels'
import { roundedText } from '@/components/ui/rounded-text'
import { decimal, percent, signed } from '@/lib/format'
import { chartBars } from '@/lib/liquidity-bars'
import { levelName, STATE_LABEL } from '@/lib/liquidity-labels'
import { LIQUIDITY_US100 as DATA } from '@/mocks'
import { LiquidityContent, LiquidityView } from '@/views/liquidity'

describe('vista de liquidez en vivo (#231)', () => {
  afterEach(() => vi.unstubAllGlobals())

  const answer = (body: unknown, status = 200) =>
    vi.fn(async () => new Response(JSON.stringify(body), { status }))

  const INSTRUMENTS = {
    default_instrument: 'US100',
    default_timeframe: '15m',
    timeframes: ['15m', '1h', '1d'],
    instruments: ['DE40', 'US100'].map((id) => ({
      id,
      name: id,
      asset_class: 'index',
      currency: 'EUR',
      exchange_timezone: 'Europe/Berlin',
    })),
  }

  /** Un gateway falso: `/api/v1/instruments` y `/api/v1/liquidity`, con la vista que se le pase. */
  const gateway = (view: (url: URL) => unknown) =>
    vi.fn(async (input: string | URL | Request) => {
      const url = new URL(String(input), 'http://ui.test')
      const body = url.pathname === '/api/v1/instruments' ? INSTRUMENTS : view(url)
      return new Response(JSON.stringify(body), { status: 200 })
    })

  it('se abre desde la paleta y pide los datos al gateway', async () => {
    const fetch = gateway(() => DATA)
    vi.stubGlobal('fetch', fetch)
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: 'k', metaKey: true })
    fireEvent.click(screen.getByRole('option', { name: /^Liquidez/ }))
    expect(screen.getByRole('heading', { level: 1, name: 'Liquidez' })).toBeInTheDocument()
    expect(location.hash).toBe('#liquidez')
    expect(
      await screen.findByRole('img', { name: /US100, \d+ velas de 15 minutos/ }),
    ).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith(
      '/api/v1/liquidity?instrument=US100&timeframe=15m',
      expect.anything(),
    )
  })

  it('los selectores piden otro instrumento y timeframe, y el diario usa sus velas', async () => {
    const daily = {
      ...DATA,
      instrument_id: 'DE40',
      timeframe: '1d',
      candles: [
        { day: '2026-11-19', start: '2026-11-18T23:00:00Z', end: '2026-11-19T23:00:00Z' },
        { day: '2026-11-20', start: '2026-11-19T23:00:00Z', end: '2026-11-20T23:00:00Z' },
      ].map((d) => ({
        instrument_id: 'DE40',
        ...d,
        open: '10.0',
        high: '12.0',
        low: '9.0',
        close: '11.0',
        volume: null,
      })),
      fvgs: [],
      imbalance: [],
    }
    const fetch = gateway((url) => (url.searchParams.get('timeframe') === '1d' ? daily : DATA))
    vi.stubGlobal('fetch', fetch)
    render(<LiquidityView />)
    await screen.findByRole('img', { name: /US100, \d+ velas de 15 minutos/ })
    fireEvent.click(screen.getByRole('button', { name: 'DE40' }))
    fireEvent.click(screen.getByRole('button', { name: '1d' }))
    expect(await screen.findByRole('img', { name: /DE40, 2 velas diarias/ })).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith(
      '/api/v1/liquidity?instrument=DE40&timeframe=1d',
      expect.anything(),
    )
  })

  it('con la base vacía muestra "sin datos todavía"', async () => {
    const empty = {
      instrument_id: 'US100',
      generated_at: '2026-11-23T10:00:00Z',
      as_of: null,
      candles: [],
      levels: [],
      fvgs: [],
      imbalance: [],
      stats: [],
    }
    vi.stubGlobal(
      'fetch',
      gateway(() => empty),
    )
    render(<LiquidityView />)
    expect(await screen.findByRole('heading', { name: 'Sin datos todavía' })).toBeInTheDocument()
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('con una sola vela muestra el gráfico y avisa lo que falta', () => {
    const one = { ...DATA, candles: DATA.candles.slice(-1), fvgs: [], imbalance: [], stats: [] }
    render(<LiquidityContent data={one} />)
    expect(screen.getByRole('img', { name: /US100, 1 vela de 15 minutos/ })).toBeInTheDocument()
    expect(screen.getByText('Todavía no hay estadísticas guardadas.')).toBeInTheDocument()
    expect(screen.getByText(/calentamiento/)).toBeInTheDocument()
  })

  it('si el gateway no responde lo dice, sin inventar datos', async () => {
    vi.stubGlobal('fetch', answer('error interno', 500))
    render(<LiquidityView />)
    expect(await screen.findByRole('alert')).toHaveTextContent('500')
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('muestra cada nivel con el estado y el precio que manda el motor', () => {
    render(<LiquidityContent data={DATA} />)
    const table = screen.getByRole('table', { name: 'Niveles de liquidez y su estado' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(DATA.levels.length)
    DATA.levels.forEach((l, i) => {
      expect(rows[i]).toHaveTextContent(levelName(l))
      expect(rows[i]).toHaveTextContent(decimal(l.price))
      expect(rows[i]).toHaveTextContent(STATE_LABEL[l.state])
    })
    // el ejemplo cubre todos los tipos y los tres estados
    const kinds = new Set(DATA.levels.map((l) => l.kind))
    for (const k of ['eqh', 'eql', 'pdh', 'pdl', 'pwh', 'pwl', 'session_high', 'session_low'])
      expect(kinds.has(k as never)).toBe(true)
    expect(new Set(DATA.levels.map((l) => l.state))).toEqual(new Set(['intact', 'swept', 'broken']))
  })

  it('separa los FVG abiertos de los llenos, tal como vienen', () => {
    render(<LiquidityContent data={DATA} />)
    const open = DATA.fvgs.filter((f) => f.status === 'open').length
    const filled = DATA.fvgs.filter((f) => f.status !== 'open').length
    expect(open).toBeGreaterThan(0)
    expect(filled).toBeGreaterThan(0)
    expect(screen.getByRole('heading', { name: `FVG abiertos (${open})` })).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: `FVG llenos o vencidos (${filled})` }),
    ).toBeInTheDocument()
  })

  it('las tasas muestran el IC 95 % y n, o "datos insuficientes"', () => {
    const { rerender } = render(
      <Rate
        minCases={30}
        stat={{
          metric: 'touch',
          successes: 36,
          n: 58,
          insufficient: false,
          rate: '0.6207',
          ci_low: '0.4921',
          ci_high: '0.7344',
        }}
      />,
    )
    expect(screen.getByText('62,0 %')).toBeInTheDocument()
    expect(screen.getByText(/IC 95 %: 49,2–73,4 · n = 58/)).toBeInTheDocument()
    rerender(
      <Rate
        minCases={30}
        stat={{
          metric: 'fill',
          successes: 5,
          n: 12,
          insufficient: true,
          rate: null,
          ci_low: null,
          ci_high: null,
        }}
      />,
    )
    expect(screen.getByText('Datos insuficientes')).toBeInTheDocument()
    expect(screen.getByText('n = 12 de 30')).toBeInTheDocument()
    expect(screen.queryByText(/%/)).not.toBeInTheDocument()
  })

  it('el panel de estadísticas usa los números del motor, con y sin casos suficientes', () => {
    render(<LiquidityContent data={DATA} />)
    const ok = DATA.stats.flatMap((s) => s.rates).find((r) => !r.insufficient && r.rate)!
    expect(screen.getAllByText(`${percent(ok.rate!)} %`).length).toBeGreaterThan(0)
    expect(screen.getAllByText('Datos insuficientes').length).toBeGreaterThan(0)
    expect(screen.getByText(/Gaps/)).toBeInTheDocument()
  })

  it('el desequilibrio muestra el último valor con el rótulo del contrato', () => {
    const current = DATA.imbalance.at(-1)!
    render(<ImbalanceGauge imbalance={current} />)
    expect(screen.getByText('aproximación desde el precio')).toBeInTheDocument()
    // Con dos decimales (#357): el valor guardado no cambia, solo cómo se escribe.
    expect(screen.getByText(signed(roundedText(current.index)))).toBeInTheDocument()
    expect(screen.getByRole('meter', { name: /aproximación desde el precio/ })).toHaveAttribute(
      'aria-valuetext',
      signed(roundedText(current.index)),
    )
    expect(screen.getByText('no es flujo de órdenes')).toBeInTheDocument()
  })

  it('con muchos FVG muestra los más nuevos y un botón para ver el resto (#357)', () => {
    const many = DATA.fvgs
    expect(many.length).toBeGreaterThan(3)
    render(<FvgTable fvgs={many} preview={3} />)
    const table = screen.getByRole('table', { name: /FVG/ })
    expect(within(table).getAllByRole('row')).toHaveLength(1 + 3)
    const more = screen.getByRole('button', { name: `Mostrar los ${many.length - 3} restantes` })
    expect(more).toHaveAttribute('aria-expanded', 'false')
    fireEvent.click(more)
    expect(within(table).getAllByRole('row')).toHaveLength(1 + many.length)
    expect(screen.getByRole('button', { name: /Mostrar solo los 3 más nuevos/ })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
  })

  it('el ejemplo trae un nivel vencido y la tabla lo muestra atenuado', () => {
    const expired = DATA.levels.filter((l) => l.expired_at)
    expect(expired).toHaveLength(1)
    render(<LiquidityContent data={DATA} />)
    const table = screen.getByRole('table', { name: 'Niveles de liquidez y su estado' })
    const rows = within(table)
      .getAllByRole('row')
      .filter((r) => r.hasAttribute('data-expired'))
    expect(rows).toHaveLength(1)
    expect(rows[0]).toHaveTextContent(decimal(expired[0].price))
  })

  it('un nivel vencido (expired_at) se muestra atenuado y fuera de los activos', () => {
    const base = DATA.levels.find((l) => l.state === 'intact')!
    const expired = { ...base, expired_at: DATA.as_of }
    render(<LevelsTable levels={[expired]} />)
    const row = screen.getByText(levelName(expired)).closest('tr')!
    expect(row).toHaveAttribute('data-expired', 'true')
    expect(row).toHaveTextContent('vencido')
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={chartBars(DATA)}
        levels={[expired]}
        fvgs={[]}
        imbalance={[]}
      />,
    )
    expect(container.querySelector('[data-expired]')).not.toBeNull()
    fireEvent.click(within(container).getByRole('button', { name: 'Activos' }))
    expect(container.querySelector('[data-level-state]')).toBeNull()
  })

  it('no dibuja un nivel antes de que exista', () => {
    const level = DATA.levels.find((l) => l.kind === 'eqh')!
    const before = chartBars(DATA).filter((b) => b.start < level.exists_from)
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={before.slice(0, -1)}
        levels={[level]}
        fvgs={[]}
        imbalance={[]}
      />,
    )
    expect(container.querySelectorAll('[data-level-state]')).toHaveLength(0)
  })

  it('los rótulos del eje no se pisan entre sí ni con los precios, y no salen del panel (#355)', () => {
    const bars = chartBars(DATA)
    const base = DATA.levels.find((l) => l.state === 'intact' && !l.expired_at)!
    const lows = bars.slice(-192).map((b) => Number(b.low))
    const lo = Math.min(...lows)
    // muchos niveles juntos cerca del mínimo y varios por debajo de la escala
    const levels = [0, 1, 2, 3, 4, 5].map((k) => ({
      ...base,
      kind: 'session_low' as const,
      price: String(lo + k * 2),
      exists_from: bars[0].start,
    }))
    const below = [1, 2, 3].map((k) => ({
      ...base,
      kind: 'pdl' as const,
      price: String(lo - 1000 * k),
      exists_from: bars[0].start,
    }))
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={bars}
        levels={[...levels, ...below]}
        fvgs={[]}
        imbalance={[]}
        height={440}
      />,
    )
    const groups = [...container.querySelectorAll('[data-level-label]')]
    const ys = groups.map((g) => Number(g.querySelector('rect')!.getAttribute('y')))
    // los que se pisan se agrupan en uno con "+N" (#580): están los 9, en pocos rótulos
    const total = groups.reduce((s, g) => s + Number(g.getAttribute('data-label-count') ?? 1), 0)
    expect(total).toBe(9)
    expect(ys.length).toBeLessThan(9)
    expect(groups.some((g) => / \+\d+$/.test(g.querySelector('text')!.textContent!))).toBe(true)
    // la caja de cada rótulo (11 px) queda dentro del panel de precio: de 6 a 440 − 22
    for (const y of ys) {
      expect(y).toBeGreaterThanOrEqual(6)
      expect(y + 11).toBeLessThanOrEqual(440 - 22)
    }
    const sorted = [...ys].sort((a, b) => a - b)
    sorted.slice(1).forEach((y, k) => expect(y - sorted[k]).toBeGreaterThanOrEqual(11.9))
    // los precios del eje que quedan a la vista no chocan con ningún rótulo
    const ticks = [...container.querySelectorAll('[data-axis-tick]')].map(
      (t) => Number(t.getAttribute('y')) - 3,
    )
    expect(ticks.length).toBeGreaterThan(0)
    for (const t of ticks)
      for (const y of ys) expect(Math.abs(t - (y + 6))).toBeGreaterThanOrEqual(12)
  })

  it('placeLabels separa, empuja desde abajo y no sale nunca del rango', () => {
    const ys = (xs: number[], min: number, max: number) =>
      placeLabels(
        xs.map((y) => ({ y })),
        min,
        max,
      ).map((t) => t.y)
    expect(ys([50, 10, 30], 0, 100)).toEqual([10, 30, 50])
    expect(ys([10, 11, 12], 0, 100)).toEqual([10, 22, 34])
    // pegados al borde de abajo: suben desde ahí
    expect(ys([95, 99, 200], 0, 100)).toEqual([76, 88, 100])
    // si no entran todos, ninguno sale del rango
    for (const y of ys([5, 5, 5, 5, 5], 0, 30)) {
      expect(y).toBeGreaterThanOrEqual(0)
      expect(y).toBeLessThanOrEqual(30)
    }
  })

  it('el porcentaje corre la coma en el texto, sin float', () => {
    expect(percent('0.6207')).toBe('62,0')
    expect(percent('1')).toBe('100,0')
    expect(percent('0.0874', 2)).toBe('8,74')
  })

  it('ningún texto de la vista promete ganancias', () => {
    const { container } = render(<LiquidityContent data={DATA} />)
    expect(container.textContent).not.toMatch(/ganancia (segura|garantizada)|sin riesgo|asegurad/i)
  })

  it('la fuente de futuros se pide con ?source=futures y la vista dice de dónde son los precios', async () => {
    const withSources = {
      ...INSTRUMENTS,
      sources: [
        { id: 'ig', label: 'IG CFD' },
        { id: 'futures', label: 'futuros CME/Eurex (base distinta al CFD)' },
      ],
    }
    const futures = {
      ...DATA,
      source: { id: 'futures', label: 'futuros CME/Eurex (base distinta al CFD)' },
    }
    const fetch = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(String(input), 'http://ui.test')
      const body =
        url.pathname === '/api/v1/instruments'
          ? withSources
          : url.searchParams.get('source') === 'futures'
            ? futures
            : { ...DATA, source: { id: 'ig', label: 'IG CFD' } }
      return new Response(JSON.stringify(body), { status: 200 })
    })
    vi.stubGlobal('fetch', fetch)
    render(<LiquidityView />)
    expect(await screen.findByText('IG CFD', { selector: 'strong' })).toBeInTheDocument()
    expect(screen.queryByRole('note')).not.toBeInTheDocument()
    fireEvent.click(
      screen.getByRole('button', { name: 'futuros CME/Eurex (base distinta al CFD)' }),
    )
    expect(
      await screen.findByText('futuros CME/Eurex (base distinta al CFD)', { selector: 'strong' }),
    ).toBeInTheDocument()
    expect(screen.getByRole('note')).toHaveTextContent(/precios del futuro, no del CFD/)
    expect(fetch).toHaveBeenCalledWith(
      '/api/v1/liquidity?instrument=US100&timeframe=15m&source=futures',
      expect.anything(),
    )
  })
})
