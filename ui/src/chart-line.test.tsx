import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { closeAt, rangeTrend } from '@/components/ui/chart-measure'
import { DepthChart } from '@/components/ui/depth-chart'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { TradingChart } from '@/components/ui/trading-chart'
import { dateTimeUtc, decimal } from '@/lib/format'
import { chartBars } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA, US100_1H, US100_BOOK, type PriceBar } from '@/mocks'

// jsdom no tiene layout: el SVG mide lo que dibuja (720 de ancho, 1:1), como en el navegador.
const box = (height: number) =>
  vi.spyOn(SVGElement.prototype, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    x: 0,
    y: 0,
    width: 720,
    height,
    right: 720,
    bottom: height,
    toJSON: () => ({}),
  } as DOMRect)

const BARS = chartBars(DATA)

function liquidity() {
  const utils = render(
    <LiquidityChart
      instrument="US100"
      bars={BARS}
      levels={DATA.levels}
      fvgs={DATA.fvgs}
      imbalance={DATA.imbalance}
    />,
  )
  const svg = screen.getByRole('img', { name: /US100, \d+ velas de 15 minutos/ })
  const count = Number(/, (\d+) velas/.exec(svg.getAttribute('aria-label')!)![1])
  const view = BARS.slice(BARS.length - count)
  const slot = (720 - 70) / count
  return { ...utils, svg, view, slot, xOf: (i: number) => (i + 0.5) * slot }
}

const pickMode = (mode: 'Velas' | 'Línea') =>
  fireEvent.click(
    within(screen.getByRole('group', { name: 'Tipo de gráfico' })).getByRole('button', {
      name: mode,
    }),
  )

const readout = (c: Element) => c.querySelector('[data-chart-readout]')!
const change = (c: Element) => c.querySelector('[data-chart-change]')!

/** El % esperado sin signo, calculado aparte: |final − inicial| / inicial × 100 con 2 decimales. */
const absPct = (a: string, b: string) =>
  `${Math.abs(((Number(b) - Number(a)) / Number(a)) * 100)
    .toFixed(2)
    .replace('.', ',')} %`

/** Espejo de las velas (cada precio p pasa a 2·M − p): el rango cambia de signo. */
function mirror(bars: PriceBar[], m = 20000): PriceBar[] {
  const f = (v: string) => (2 * m - Number(v)).toFixed((v.split('.')[1] ?? '').length)
  return bars.map((b) => ({
    ...b,
    open: f(b.open),
    close: f(b.close),
    high: f(b.low),
    low: f(b.high),
  }))
}

describe('gráficos más fluidos y modo línea (#377)', () => {
  beforeEach(() => {
    box(440)
  })
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('el selector Velas/Línea cambia el dibujo en los dos gráficos', () => {
    const { container } = liquidity()
    const group = screen.getByRole('group', { name: 'Tipo de gráfico' })
    expect(within(group).getByRole('button', { name: 'Velas' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(container.querySelectorAll('[data-candle]').length).toBeGreaterThan(10)
    expect(container.querySelector('[data-line-area]')).toBeNull()

    pickMode('Línea')
    expect(within(group).getByRole('button', { name: 'Línea' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(container.querySelector('[data-candle]')).toBeNull()
    const area = container.querySelector('[data-line-area]')!
    expect(area.querySelector('[data-close-line]')).not.toBeNull()
    expect(area.querySelector('[data-area]')!.getAttribute('fill')).toMatch(/^url\(#/)
    expect(area.querySelectorAll('[data-grid-x]').length).toBeGreaterThan(0)
    expect(area.querySelector('[data-last-dot]')).not.toBeNull()
    // los niveles de liquidez se siguen viendo sobre la línea
    expect(container.querySelector('[data-level-state]')).not.toBeNull()

    pickMode('Velas')
    expect(container.querySelector('[data-line-area]')).toBeNull()
  })

  it('la línea y el área degradada toman el color del signo del rango visible', () => {
    vi.restoreAllMocks()
    box(520)
    const check = (bars: PriceBar[]) => {
      const { container, unmount } = render(<TradingChart instrument="US100" bars={bars} />)
      pickMode('Línea')
      const view = bars.slice(-120)
      const up = Number(view[view.length - 1].close) > Number(view[0].close)
      const color = up ? 'var(--chart-up)' : 'var(--chart-down)'
      const area = container.querySelector('[data-line-area]')!
      expect(area).toHaveAttribute('data-line-area', up ? 'up' : 'down')
      expect(area.querySelector('[data-close-line]')).toHaveAttribute('stroke', color)
      const stops = area.querySelectorAll('stop')
      expect(stops.length).toBeGreaterThanOrEqual(2)
      stops.forEach((s) => expect(s).toHaveAttribute('stop-color', color))
      // se desvanece hacia abajo: arriba opaco, abajo transparente
      expect(Number(stops[0].getAttribute('stop-opacity'))).toBeGreaterThan(0)
      expect(stops[stops.length - 1]).toHaveAttribute('stop-opacity', '0')
      // la cabecera dice lo mismo que el color
      expect(change(container)).toHaveAttribute('data-chart-change', up ? 'up' : 'down')
      expect(change(container).textContent).toContain(up ? '▲' : '▼')
      unmount()
      return up
    }
    const a = check(US100_1H.candles)
    const b = check(mirror(US100_1H.candles))
    expect(a).not.toBe(b)
    expect(rangeTrend([1, 2, 1], 0)).toBe('flat')
  })

  it('la cabecera muestra el cambio del rango, el de la vela bajo el cursor y vuelve al salir', () => {
    const { container, svg, view, xOf } = liquidity()
    const first = view[0].close
    const last = view[view.length - 1].close
    expect(readout(container)).toHaveAttribute('data-chart-readout', 'idle')
    expect(change(container).textContent).toContain(absPct(first, last))
    expect(readout(container).textContent).toContain('2 días')

    const j = Math.floor(view.length * 0.6)
    fireEvent.pointerMove(svg, { clientX: xOf(j), clientY: 200 })
    expect(readout(container)).toHaveAttribute('data-chart-readout', 'hover')
    expect(readout(container).querySelector('p')!.textContent).toBe(decimal(view[j].close))
    const text = change(container).textContent!
    const rising = Number(view[j].close) > Number(first)
    expect(text).toMatch(rising ? /^\+[\d.]+,\d+ ▲ \d+,\d{2} %$/ : /^−[\d.]+,\d+ ▼ \d+,\d{2} %$/)
    expect(text).toContain(absPct(first, view[j].close))
    expect(readout(container).textContent).toContain(dateTimeUtc(view[j].start))

    fireEvent.pointerLeave(svg)
    expect(readout(container)).toHaveAttribute('data-chart-readout', 'idle')
  })

  it('al arrastrar en modo línea resalta el tramo con el color del signo y muestra las fechas', () => {
    const { container, svg, view, xOf } = liquidity()
    pickMode('Línea')
    const i = Math.floor(view.length * 0.2)
    const j = Math.floor(view.length * 0.7)
    fireEvent.pointerDown(svg, { clientX: xOf(i), clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: xOf(j), clientY: 220, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: xOf(j), clientY: 220, pointerId: 1 })

    const up = Number(view[j].close) > Number(view[i].close)
    const color = up ? 'var(--chart-up)' : 'var(--chart-down)'
    const g = container.querySelector('[data-measure]')!
    expect(g).toHaveAttribute('data-mode', 'line')
    expect(g).toHaveAttribute('data-measure', up ? 'up' : 'down')
    expect(g.querySelector('[data-measure-segment]')).toHaveAttribute('stroke', color)
    // en modo línea no va el rectángulo de las velas
    expect(g.querySelector('[data-measure-box]')).toBeNull()

    expect(readout(container)).toHaveAttribute('data-chart-readout', 'measure')
    expect(readout(container).querySelector('p')!.textContent).toBe(decimal(view[j].close))
    const text = change(container).textContent!
    expect(text).toContain(up ? '▲' : '▼')
    expect(text).toContain(absPct(view[i].close, view[j].close))
    expect(change(container)).toHaveAttribute('data-chart-change', up ? 'up' : 'down')
    expect(readout(container).textContent).toContain(
      `${dateTimeUtc(view[i].start).replace(/ UTC$/, '')} – ${dateTimeUtc(view[j].start)}`,
    )

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(container.querySelector('[data-measure]')).toBeNull()
    expect(readout(container)).not.toHaveAttribute('data-chart-readout', 'measure')
  })

  it('en modo línea el punto recorre la línea entre cierres; con velas se pega al cierre', () => {
    const { container, svg, view, xOf, slot } = liquidity()
    const dot = () => container.querySelector('[data-hover-dot]')!
    const i = Math.floor(view.length / 2)
    const x = xOf(i) + slot * 0.4

    fireEvent.pointerMove(svg, { clientX: x, clientY: 200 })
    expect(Number(dot().getAttribute('data-x'))).toBeCloseTo(xOf(i), 6)

    pickMode('Línea')
    fireEvent.pointerMove(svg, { clientX: x, clientY: 200 })
    expect(Number(dot().getAttribute('data-x'))).toBeCloseTo(x, 6)
    const yLine = container
      .querySelector('[data-close-line]')!
      .getAttribute('d')!
      .split(/[ML]/)
      .filter(Boolean)
      .map((p) => p.split(',').map(Number))
    // sobre el tramo recto entre los dos cierres
    const y = yLine[i][1] + (yLine[i + 1][1] - yLine[i][1]) * 0.4
    expect(Number(dot().getAttribute('data-y'))).toBeCloseTo(y, 0)
    expect(closeAt([10, 20], 0.25)).toBe(12.5)
    // la cabecera lee la vela más cercana, no un precio inventado
    expect(readout(container).querySelector('p')!.textContent).toBe(decimal(view[i].close))
  })

  it('el punto se desliza cuadro a cuadro hasta la vela nueva', () => {
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => frames.push(cb))
    vi.stubGlobal('cancelAnimationFrame', () => frames.splice(0))
    const { container, svg, xOf } = liquidity()
    const dot = () => container.querySelector('[data-hover-dot]')!
    const cx = () => Number(dot().querySelector('[data-dot]')!.getAttribute('cx'))

    fireEvent.pointerMove(svg, { clientX: xOf(10), clientY: 200 })
    // al aparecer va directo
    expect(cx()).toBeCloseTo(xOf(10), 6)
    fireEvent.pointerMove(svg, { clientX: xOf(40), clientY: 200 })
    expect(Number(dot().getAttribute('data-x'))).toBeCloseTo(xOf(40), 6)
    expect(cx()).toBeCloseTo(xOf(10), 6)

    const t0 = performance.now()
    let k = 0
    act(() => frames.shift()!(t0 + 16 * ++k))
    expect(cx()).toBeGreaterThan(xOf(10))
    expect(cx()).toBeLessThan(xOf(40))
    while (frames.length && k < 100) act(() => frames.shift()!(t0 + 16 * ++k))
    expect(cx()).toBeCloseTo(xOf(40), 6)
    // al llegar se detiene
    expect(frames).toHaveLength(0)
  })

  it('con prefers-reduced-motion el punto y la medición van directo, sin animación', () => {
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => frames.push(cb))
    vi.spyOn(window, 'matchMedia').mockImplementation(
      (q: string) =>
        ({
          matches: q.includes('prefers-reduced-motion'),
          media: q,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
        }) as unknown as MediaQueryList,
    )
    const { container, svg, xOf } = liquidity()
    const cx = () =>
      Number(container.querySelector('[data-hover-dot] [data-dot]')!.getAttribute('cx'))
    fireEvent.pointerMove(svg, { clientX: xOf(10), clientY: 200 })
    fireEvent.pointerMove(svg, { clientX: xOf(40), clientY: 200 })
    expect(cx()).toBeCloseTo(xOf(40), 6)

    fireEvent.pointerDown(svg, { clientX: xOf(5), clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: xOf(30), clientY: 200, pointerId: 1 })
    const to = container.querySelector('[data-measure-point="to"]')!
    expect(Number(to.getAttribute('cx'))).toBeCloseTo(xOf(30), 6)
    expect(frames).toHaveLength(0)
  })
})

describe('profundidad: la sonda se mueve sin volver a dibujar el gráfico (#377)', () => {
  afterEach(() => vi.restoreAllMocks())

  it('sigue al cursor, marca compras o ventas y se va al salir', () => {
    vi.spyOn(SVGElement.prototype, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      x: 0,
      y: 0,
      width: 560,
      height: 260,
      right: 560,
      bottom: 260,
      toJSON: () => ({}),
    } as DOMRect)
    const { container } = render(<DepthChart instrument="US100" book={US100_BOOK} />)
    const svg = screen.getByRole('img', { name: /Profundidad de US100/ })
    const probe = () => container.querySelector('[data-depth-probe]')
    expect(probe()).toBeNull()
    fireEvent.pointerMove(svg, { clientX: 120, clientY: 100 })
    expect(probe()).toHaveAttribute('data-depth-probe', 'bid')
    fireEvent.pointerMove(svg, { clientX: 440, clientY: 100 })
    expect(probe()).toHaveAttribute('data-depth-probe', 'ask')
    expect(Number(probe()!.getAttribute('data-x'))).toBe(440)
    fireEvent.pointerLeave(svg)
    expect(probe()).toBeNull()
  })
})
