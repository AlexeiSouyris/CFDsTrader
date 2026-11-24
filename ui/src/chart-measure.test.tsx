import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { measureText, signedFixed, span } from '@/components/ui/chart-measure'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { TradingChart } from '@/components/ui/trading-chart'
import { chartBars } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA, US100_1H } from '@/mocks'

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

/** Mantener apretado en (x0, y0), arrastrar hasta (x1, y1) y soltar. */
function drag(el: Element, x0: number, y0: number, x1: number, y1: number) {
  fireEvent.pointerDown(el, { clientX: x0, clientY: y0, button: 0, pointerId: 1 })
  fireEvent.pointerMove(el, { clientX: (x0 + x1) / 2, clientY: (y0 + y1) / 2, pointerId: 1 })
  fireEvent.pointerMove(el, { clientX: x1, clientY: y1, pointerId: 1 })
  fireEvent.pointerUp(el, { clientX: x1, clientY: y1, pointerId: 1 })
}

/** El % esperado, calculado aparte: (final − inicial) / inicial × 100, con signo y 2 decimales. */
function expectedPct(g: Element) {
  const from = Number(g.getAttribute('data-from'))
  const to = Number(g.getAttribute('data-to'))
  const t = (((to - from) / from) * 100).toFixed(2)
  return `${t.startsWith('-') ? '−' : '+'}${t.replace('-', '').replace('.', ',')} %`
}

type Bar = { start: string; open: string; close: string }

/** Cuántas velas se ven (sale del `aria-label`) y la x del centro de cada una. */
function geometry<B extends Bar>(svg: Element, bars: B[], axisW: number) {
  const count = Number(/, (\d+) velas/.exec(svg.getAttribute('aria-label')!)![1])
  const view = bars.slice(bars.length - count)
  const slot = (720 - axisW) / count
  return { view, xOf: (i: number) => (i + 0.5) * slot }
}

/** Dos velas separadas cuyo cierre sube (o baja) de la primera a la segunda. */
function pair(view: Bar[], dir: 'up' | 'down'): [number, number] {
  const i = Math.floor(view.length * 0.3)
  const c = Number(view[i].close)
  for (let j = i + 3; j < view.length; j++) {
    const d = Number(view[j].close) - c
    if (dir === 'up' ? d > 0 : d < 0) return [i, j]
  }
  throw new Error(`sin par ${dir}`)
}

/** La vela alcista (o bajista) de cuerpo más grande, así el borde del cuerpo es claro. */
function bodyOf(view: Bar[], dir: 'up' | 'down'): number {
  let best = -1
  let size = 0
  view.forEach((b, i) => {
    const d = Number(b.close) - Number(b.open)
    if ((dir === 'up' ? d > 0 : d < 0) && Math.abs(d) > size) {
      best = i
      size = Math.abs(d)
    }
  })
  return best
}

/** Bordes de arriba y de abajo del cuerpo dibujado de una vela. */
function bodyEdges(container: Element, start: string) {
  const r = container.querySelector(`[data-candle="${start}"] rect`)!
  const top = Number(r.getAttribute('y'))
  return { top, bottom: top + Number(r.getAttribute('height')) }
}

describe('herramienta de medición de los gráficos (#281, #355)', () => {
  beforeEach(() => {
    box(440)
  })
  afterEach(() => vi.restoreAllMocks())

  const chart = () => {
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={chartBars(DATA)}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
      />,
    )
    const svg = screen.getByRole('img', { name: /US100, \d+ velas de 15 minutos/ })
    const measure = () => container.querySelector('[data-measure]')
    return { svg, measure, container, ...geometry(svg, chartBars(DATA), 70) }
  }

  it('al subir dibuja un rectángulo verde con la diferencia, el % y las velas', () => {
    const { svg, measure, xOf, view } = chart()
    const [i, j] = pair(view, 'up')
    expect(measure()).toBeNull()
    fireEvent.pointerDown(svg, { clientX: xOf(i), clientY: 300, button: 0, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: xOf(j), clientY: 100, pointerId: 1 })
    // mientras se arrastra ya se ve
    expect(measure()).toHaveAttribute('data-measure', 'up')
    fireEvent.pointerUp(svg, { clientX: xOf(j), clientY: 100, pointerId: 1 })

    const g = measure()!
    expect(g).toHaveAttribute('data-measure', 'up')
    expect(g.querySelector('[data-measure-box]')).toHaveAttribute('fill', 'var(--chart-up)')
    const [head, foot] = g.querySelectorAll('[data-measure-line]')
    expect(head.textContent).toMatch(/^\+[\d.]+,\d+ \(\+\d+,\d{2} %\)$/)
    expect(head.textContent).toContain(`(${expectedPct(g)})`)
    expect(Number(g.getAttribute('data-to'))).toBeGreaterThan(Number(g.getAttribute('data-from')))
    expect(foot.textContent).toBe(
      `${j - i} velas · ${span(Date.parse(view[j].start) - Date.parse(view[i].start))}`,
    )
  })

  it('al bajar el rectángulo es rojo y el % es negativo', () => {
    const { svg, measure, xOf, view } = chart()
    const [i, j] = pair(view, 'down')
    drag(svg, xOf(i), 100, xOf(j), 350)
    const g = measure()!
    expect(g).toHaveAttribute('data-measure', 'down')
    expect(g.querySelector('[data-measure-box]')).toHaveAttribute('fill', 'var(--chart-down)')
    const head = g.querySelector('[data-measure-line="0"]')!.textContent!
    expect(head).toMatch(/^−[\d.]+,\d+ \(−\d+,\d{2} %\)$/)
    expect(head).toContain(`(${expectedPct(g)})`)
  })

  it('los dos puntos se pegan al cierre de su vela, no a la altura del cursor (#355)', () => {
    const { svg, measure, xOf, view, container } = chart()
    const up = bodyOf(view, 'up')
    const down = bodyOf(view, 'down')
    // mismo par de velas con alturas de cursor opuestas: la medición es la misma
    drag(svg, xOf(up), 20, xOf(down), 400)
    const a = measure()!
    expect(Number(a.getAttribute('data-from'))).toBeCloseTo(Number(view[up].close), 6)
    expect(Number(a.getAttribute('data-to'))).toBeCloseTo(Number(view[down].close), 6)
    const pts = () =>
      ['from', 'to'].map((k) =>
        container.querySelector(`[data-measure-point="${k}"]`)!.getAttribute('cy'),
      )
    const first = pts()
    drag(svg, xOf(up), 400, xOf(down), 20)
    expect(pts()).toEqual(first)
    // alcista: el cierre es el borde de arriba del cuerpo; bajista: el de abajo
    const [yFrom, yTo] = first.map(Number)
    expect(yFrom).toBeCloseTo(bodyEdges(container, view[up].start).top, 6)
    expect(yTo).toBeCloseTo(bodyEdges(container, view[down].start).bottom, 6)
  })

  it('queda visible al soltar y se borra con Esc o con un clic; otro arrastre la reemplaza', () => {
    const { svg, measure, xOf, view } = chart()
    const [i, j] = pair(view, 'up')
    const [k, m] = pair(view, 'down')
    drag(svg, xOf(i), 300, xOf(j), 100)
    expect(measure()).toHaveAttribute('data-measure', 'up')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(measure()).toBeNull()

    drag(svg, xOf(i), 300, xOf(j), 100)
    drag(svg, xOf(k), 100, xOf(m), 300)
    expect(measure()).toHaveAttribute('data-measure', 'down')
    expect(document.querySelectorAll('[data-measure]')).toHaveLength(1)

    // un clic (apretar y soltar sin moverse) la borra
    fireEvent.pointerDown(svg, { clientX: 200, clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 200, clientY: 200, pointerId: 1 })
    expect(measure()).toBeNull()
  })

  it('el punto sigue al cierre de la vela bajo el cursor, esté arriba o abajo de las velas (#355)', () => {
    const { svg, xOf, view, container } = chart()
    const dot = () => container.querySelector('[data-hover-dot]')
    const cy = () => Number(dot()!.querySelector('circle')!.getAttribute('cy'))
    expect(dot()).toBeNull()

    const up = bodyOf(view, 'up')
    fireEvent.pointerMove(svg, { clientX: xOf(up), clientY: 12 })
    expect(dot()).toHaveAttribute('data-hover-dot', view[up].start)
    const high = cy()
    fireEvent.pointerMove(svg, { clientX: xOf(up), clientY: 410 })
    expect(cy()).toBe(high)
    // vela alcista: el cierre está arriba del cuerpo
    expect(high).toBeCloseTo(bodyEdges(container, view[up].start).top, 6)

    const down = bodyOf(view, 'down')
    fireEvent.pointerMove(svg, { clientX: xOf(down), clientY: 12 })
    expect(dot()).toHaveAttribute('data-hover-dot', view[down].start)
    // vela bajista: el cierre está abajo del cuerpo
    expect(cy()).toBeCloseTo(bodyEdges(container, view[down].start).bottom, 6)

    // fuera del panel de precio (sobre el eje) o al salir del gráfico, desaparece
    fireEvent.pointerMove(svg, { clientX: 700, clientY: 200 })
    expect(dot()).toBeNull()
    fireEvent.pointerMove(svg, { clientX: xOf(up), clientY: 200 })
    expect(dot()).not.toBeNull()
    fireEvent.pointerLeave(svg)
    expect(dot()).toBeNull()
  })

  it('no rompe los filtros ni el rango del gráfico', () => {
    const { svg, measure } = chart()
    drag(svg, 300, 300, 400, 100)
    fireEvent.click(screen.getByRole('button', { name: 'Activos' }))
    expect(measure()).not.toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Todo' }))
    expect(screen.getByRole('button', { name: 'Todo' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('en el gráfico de precio mide en el panel de precio y la cruz de lectura sigue andando', () => {
    vi.restoreAllMocks()
    box(520)
    const { container } = render(<TradingChart instrument="US100" bars={US100_1H.candles} />)
    const svg = screen.getByRole('img', { name: /US100, \d+ velas de 1 hora/ })
    const { xOf, view } = geometry(svg, US100_1H.candles, 62)
    const [i, j] = pair(view, 'up')
    drag(svg, xOf(i), 200, xOf(j), 60)
    const g = container.querySelector('[data-measure]')!
    expect(g).toHaveAttribute('data-measure', 'up')
    expect(g.querySelector('[data-measure-line="0"]')!.textContent).toContain(`(${expectedPct(g)})`)
    // la cruz de lectura (hover) se sigue dibujando
    expect(container.querySelector('line[opacity="0.3"]')).not.toBeNull()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(container.querySelector('[data-measure]')).toBeNull()
  })

  it('en el gráfico de precio el punto va al cierre, con velas y con línea (#355)', () => {
    vi.restoreAllMocks()
    box(520)
    const { container } = render(<TradingChart instrument="US100" bars={US100_1H.candles} />)
    const svg = screen.getByRole('img', { name: /US100, \d+ velas de 1 hora/ })
    const { xOf, view } = geometry(svg, US100_1H.candles, 62)
    const dot = () => container.querySelector('[data-hover-dot]')
    const cy = () => Number(dot()!.querySelector('circle')!.getAttribute('cy'))

    const down = bodyOf(view, 'down')
    fireEvent.pointerMove(svg, { clientX: xOf(down), clientY: 10 })
    expect(dot()).toHaveAttribute('data-hover-dot', view[down].start)
    expect(cy()).toBeCloseTo(bodyEdges(container, view[down].start).bottom, 6)
    // la cruz de lectura ya trae su línea vertical: el punto no agrega otra
    expect(dot()!.querySelector('line')).toBeNull()

    // en el gráfico de línea el punto va sobre la línea, que pasa por el cierre
    fireEvent.click(screen.getByRole('button', { name: 'Línea' }))
    const up = bodyOf(view, 'up')
    fireEvent.pointerMove(svg, { clientX: xOf(up), clientY: 220 })
    const d = container.querySelector('path[stroke="var(--chart-line)"]')!.getAttribute('d')!
    const pt = d.split(/[ML]/).filter(Boolean)[up].split(',').map(Number)
    expect(cy()).toBeCloseTo(pt[1], 1)
    fireEvent.pointerLeave(svg)
    expect(dot()).toBeNull()
  })

  it('los textos: signo, 2 decimales y lapso', () => {
    expect(signedFixed(12.5, 2)).toBe('+12,50')
    expect(signedFixed(-0.001, 2)).toBe('0,00')
    expect(signedFixed(-1234.5, 1)).toBe('−1.234,5')
    expect(span(0)).toBe('0 min')
    expect(span(45 * 60_000)).toBe('45 min')
    expect(span((26 * 60 + 15) * 60_000)).toBe('1 d 2 h 15 min')
    expect(measureText(100, 110, 1)).toMatchObject({ trend: 'up', price: '+10,0', pct: '+10,00 %' })
    expect(measureText(200, 150, 0)).toMatchObject({ trend: 'down', price: '−50', pct: '−25,00 %' })
    expect(measureText(100.04, 100.01, 1)).toMatchObject({ trend: 'flat', pct: '0,00 %' })
  })
})
