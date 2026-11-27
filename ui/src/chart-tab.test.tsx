import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { measureTab } from '@/components/ui/chart-measure'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { chartBars } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA } from '@/mocks'

const BARS = chartBars(DATA)
const PARTS: [string, string, string, string] = ['+431,3', '+2,28 %', '30 velas', '42 d']

function chart() {
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
  const num = (el: Element, a: string) => Number(el.getAttribute(a))
  const tab = () => utils.container.querySelector('[data-measure-tab]')!
  const box = () => utils.container.querySelector('[data-measure-box]')!
  const rect = () => {
    const b = box()
    return { x: num(b, 'x'), y: num(b, 'y'), w: num(b, 'width'), h: num(b, 'height') }
  }
  const t = () => {
    const el = tab()
    return {
      x: num(el, 'data-x'),
      y: num(el, 'data-y'),
      w: num(el, 'data-w'),
      h: num(el, 'data-h'),
    }
  }
  return { ...utils, svg, view, xOf: (i: number) => (i + 0.5) * slot, tab, rect, t }
}

/** Dos velas separadas por `gap` velas, con el cierre de la segunda por encima (o debajo). */
function pair(
  view: { close: string }[],
  dir: 'up' | 'down',
  gap = 20,
  from = 0.2,
): [number, number] {
  const i = Math.floor(view.length * from)
  for (let j = i + gap; j < view.length; j++) {
    const d = Number(view[j].close) - Number(view[i].close)
    if (dir === 'up' ? d > 0 : d < 0) return [i, j]
  }
  throw new Error('sin par')
}

describe('pestaña de la medición pegada al rectángulo (#394)', () => {
  beforeEach(() => {
    vi.spyOn(SVGElement.prototype, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      x: 0,
      y: 0,
      width: 720,
      height: 440,
      right: 720,
      bottom: 440,
      toJSON: () => ({}),
    } as DOMRect)
  })
  afterEach(() => vi.restoreAllMocks())

  it('va pegada al borde del rectángulo, centrada en x, dentro del panel y del color del signo', () => {
    const { svg, view, xOf, tab, rect, t } = chart()
    const [i, j] = pair(view, 'up')
    fireEvent.pointerDown(svg, {
      clientX: xOf(i),
      clientY: 200,
      button: 0,
      pointerId: 1,
      shiftKey: true,
    })
    fireEvent.pointerMove(svg, { clientX: xOf(j), clientY: 200, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: xOf(j), clientY: 200, pointerId: 1 })
    const r = rect()
    const p = t()
    const side = tab().getAttribute('data-measure-tab')
    // sin hueco: el borde de la pestaña es el borde del rectángulo
    if (side === 'above') expect(p.y + p.h).toBeCloseTo(r.y, 6)
    else if (side === 'below') expect(p.y).toBeCloseTo(r.y + r.h, 6)
    else expect(p.y).toBeCloseTo(r.y, 6)
    // centrada sobre el rectángulo (salvo que el borde del panel la empuje)
    if (p.x > 0 && p.x + p.w < 650) expect(p.x + p.w / 2).toBeCloseTo(r.x + r.w / 2, 6)
    expect(p.x).toBeGreaterThanOrEqual(0)
    expect(p.x + p.w).toBeLessThanOrEqual(650)
    expect(p.y).toBeGreaterThanOrEqual(6)
    expect(p.y + p.h).toBeLessThanOrEqual(440 - 22)
    // mismo color: borde y texto del signo
    const stroke = tab().querySelector('path[stroke]')!
    expect(stroke).toHaveAttribute('stroke', 'var(--chart-up)')
    expect(tab().querySelector('tspan[fill]')).toHaveAttribute('fill', 'var(--chart-up)')
  })

  it('sigue al rectángulo, no al cursor, mientras se arrastra', () => {
    const { svg, view, xOf, t } = chart()
    const [i, j] = pair(view, 'down')
    fireEvent.pointerDown(svg, {
      clientX: xOf(i),
      clientY: 200,
      button: 0,
      pointerId: 1,
      shiftKey: true,
    })
    fireEvent.pointerMove(svg, { clientX: xOf(j), clientY: 60, pointerId: 1 })
    const first = t()
    // misma vela, el cursor arriba o abajo: la pestaña no se mueve
    fireEvent.pointerMove(svg, { clientX: xOf(j), clientY: 380, pointerId: 1 })
    expect(t()).toEqual(first)
    // otra vela: el rectángulo cambia y la pestaña lo acompaña
    fireEvent.pointerMove(svg, { clientX: xOf(j + 5), clientY: 380, pointerId: 1 })
    expect(t().x).not.toBe(first.x)
  })

  it('con un rectángulo angosto la pestaña es más ancha, pero centrada sobre él', () => {
    const { svg, view, xOf, rect, t } = chart()
    const i = Math.floor(view.length / 2)
    fireEvent.pointerDown(svg, {
      clientX: xOf(i),
      clientY: 200,
      button: 0,
      pointerId: 1,
      shiftKey: true,
    })
    fireEvent.pointerMove(svg, { clientX: xOf(i + 1), clientY: 200, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: xOf(i + 1), clientY: 200, pointerId: 1 })
    const r = rect()
    const p = t()
    expect(p.w).toBeGreaterThan(r.w)
    expect(p.x + p.w / 2).toBeCloseTo(r.x + r.w / 2, 6)
  })

  it('ubicación: arriba si entra, si no abajo, si no por dentro; nunca fuera del panel', () => {
    const base = { x0: 300, x1: 340, top: 6, bottom: 400, plotW: 650, parts: PARTS }
    const above = measureTab({ ...base, yTop: 100, yBot: 200 })
    expect(above.side).toBe('above')
    expect(above.y + above.h).toBe(100)
    expect(above.lines).toEqual(['+431,3 · +2,28 % · 30 velas · 42 d'])
    expect(above.x + above.w / 2).toBeCloseTo(320, 6)

    const below = measureTab({ ...base, yTop: 10, yBot: 200 })
    expect(below.side).toBe('below')
    expect(below.y).toBe(200)

    const inside = measureTab({ ...base, yTop: 10, yBot: 395 })
    expect(inside.side).toBe('inside')
    expect(inside.y).toBe(10)
    expect(inside.y + inside.h).toBeLessThanOrEqual(400)

    // contra el borde derecho no se sale: se corre hacia adentro
    const edge = measureTab({ ...base, x0: 630, x1: 650, yTop: 100, yBot: 200 })
    expect(edge.x + edge.w).toBeCloseTo(650, 6)
    expect(edge.x).toBeGreaterThanOrEqual(0)
  })

  it('en un panel angosto (celular) pasa a dos líneas', () => {
    const tab = measureTab({
      x0: 100,
      x1: 140,
      yTop: 100,
      yBot: 200,
      top: 6,
      bottom: 400,
      plotW: 180,
      parts: ['+431,30', '+2,28 %', '30 velas', '42 d 3 h 15 min'],
    })
    expect(tab.lines).toEqual(['+431,30 · +2,28 %', '30 velas · 42 d 3 h 15 min'])
    expect(tab.x).toBeGreaterThanOrEqual(0)
  })
})
