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

describe('herramienta de medición de los gráficos (#281)', () => {
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
    return { svg, measure }
  }

  it('al subir dibuja un rectángulo verde con la diferencia, el % y las velas', () => {
    const { svg, measure } = chart()
    expect(measure()).toBeNull()
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 300, button: 0, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: 400, clientY: 100, pointerId: 1 })
    // mientras se arrastra ya se ve
    expect(measure()).toHaveAttribute('data-measure', 'up')
    fireEvent.pointerUp(svg, { clientX: 400, clientY: 100, pointerId: 1 })

    const g = measure()!
    expect(g).toHaveAttribute('data-measure', 'up')
    expect(g.querySelector('[data-measure-box]')).toHaveAttribute('fill', 'var(--chart-up)')
    const [head, foot] = g.querySelectorAll('[data-measure-line]')
    expect(head.textContent).toMatch(/^\+[\d.]+,\d+ \(\+\d+,\d{2} %\)$/)
    expect(head.textContent).toContain(`(${expectedPct(g)})`)
    expect(Number(g.getAttribute('data-to'))).toBeGreaterThan(Number(g.getAttribute('data-from')))
    expect(foot.textContent).toMatch(/^\d+ velas? · /)
  })

  it('al bajar el rectángulo es rojo y el % es negativo', () => {
    const { svg, measure } = chart()
    drag(svg, 300, 100, 500, 350)
    const g = measure()!
    expect(g).toHaveAttribute('data-measure', 'down')
    expect(g.querySelector('[data-measure-box]')).toHaveAttribute('fill', 'var(--chart-down)')
    const head = g.querySelector('[data-measure-line="0"]')!.textContent!
    expect(head).toMatch(/^−[\d.]+,\d+ \(−\d+,\d{2} %\)$/)
    expect(head).toContain(`(${expectedPct(g)})`)
  })

  it('sin diferencia de precio es neutro y marca 0,00 %', () => {
    const { svg, measure } = chart()
    drag(svg, 200, 220, 450, 220)
    const g = measure()!
    expect(g).toHaveAttribute('data-measure', 'flat')
    expect(g.getAttribute('data-from')).toBe(g.getAttribute('data-to'))
    expect(g.querySelector('[data-measure-box]')).toHaveAttribute('fill', 'var(--muted-foreground)')
    expect(g.querySelector('[data-measure-line="0"]')!.textContent).toMatch(/^0,0+ \(0,00 %\)$/)
    // se movió en el tiempo aunque no en el precio
    expect(g.querySelector('[data-measure-line="1"]')!.textContent).not.toMatch(/^0 velas/)
  })

  it('queda visible al soltar y se borra con Esc o con un clic; otro arrastre la reemplaza', () => {
    const { svg, measure } = chart()
    drag(svg, 300, 300, 400, 100)
    expect(measure()).toHaveAttribute('data-measure', 'up')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(measure()).toBeNull()

    drag(svg, 300, 300, 400, 100)
    drag(svg, 300, 100, 400, 300)
    expect(measure()).toHaveAttribute('data-measure', 'down')
    expect(document.querySelectorAll('[data-measure]')).toHaveLength(1)

    // un clic (apretar y soltar sin moverse) la borra
    fireEvent.pointerDown(svg, { clientX: 200, clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 200, clientY: 200, pointerId: 1 })
    expect(measure()).toBeNull()
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
    drag(svg, 200, 200, 400, 60)
    const g = container.querySelector('[data-measure]')!
    expect(g).toHaveAttribute('data-measure', 'up')
    expect(g.querySelector('[data-measure-line="0"]')!.textContent).toContain(`(${expectedPct(g)})`)
    // la cruz de lectura (hover) se sigue dibujando
    expect(container.querySelector('line[opacity="0.3"]')).not.toBeNull()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(container.querySelector('[data-measure]')).toBeNull()
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
