/**
 * Gestos de los gráficos con el dedo (#424): el vertical es de la página, el horizontal mueve
 * el gráfico, dos dedos hacen zoom y mantener apretado mide. La ayuda cambia con `pointer: coarse`.
 */
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { LONG_PRESS_MS } from '@/components/ui/chart-nav'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { TradingChart } from '@/components/ui/trading-chart'
import { chartBars } from '@/lib/liquidity-bars'
import { useTabSwipe } from '@/lib/use-tab-swipe'
import { LIQUIDITY_US100 as DATA, US100_1H } from '@/mocks'

const BARS = chartBars(DATA)

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

/** `matchMedia` de un celular: `pointer: coarse`. */
function coarse() {
  const real = window.matchMedia
  window.matchMedia = ((query: string) => ({
    ...real(query),
    matches: query === '(pointer: coarse)',
  })) as typeof window.matchMedia
  return () => {
    window.matchMedia = real
  }
}

const chart = () => {
  const utils = render(
    <LiquidityChart
      instrument="US100"
      bars={BARS}
      levels={DATA.levels}
      fvgs={DATA.fvgs}
      imbalance={DATA.imbalance}
    />,
  )
  const svg = screen.getByRole('img', { name: /US100, \d+ velas/ })
  const measure = () => utils.container.querySelector('[data-measure]')
  const layer = () => utils.container.querySelector('[data-price-layer]')!.getAttribute('transform')
  return { ...utils, svg, measure, layer }
}

const touch = { pointerType: 'touch', button: 0 }

/** Un `touchmove` cancelable con `n` dedos; devuelve si el gráfico lo canceló. */
function touchMove(el: Element, fingers = 1) {
  const e = new Event('touchmove', { bubbles: true, cancelable: true })
  Object.defineProperty(e, 'touches', { value: Array.from({ length: fingers }, () => ({})) })
  el.dispatchEvent(e)
  return e.defaultPrevented
}

describe('gráficos con el dedo (#424)', () => {
  beforeEach(() => {
    box(440)
  })
  afterEach(() => {
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  it('el vertical es de la página: el gráfico usa pan-y y no cancela el desplazamiento', () => {
    const { svg } = chart()
    expect(svg).toHaveClass('touch-pan-y')
    expect(svg).not.toHaveClass('touch-none')
    // el gráfico no es de las pestañas: deslizar sobre él no las cambia
    expect(svg).toHaveAttribute('data-swipe-ignore')
    fireEvent.pointerDown(svg, { ...touch, clientX: 300, clientY: 200, pointerId: 1 })
    expect(touchMove(svg)).toBe(false)
    fireEvent.pointerUp(svg, { ...touch, clientX: 300, clientY: 200, pointerId: 1 })
    // dos dedos son del gráfico (zoom), no de la página
    expect(touchMove(svg, 2)).toBe(true)
  })

  it('un dedo en horizontal mueve el gráfico y no mide', () => {
    const { svg, measure, layer } = chart()
    const before = layer()
    fireEvent.pointerDown(svg, { ...touch, clientX: 200, clientY: 200, pointerId: 1 })
    fireEvent.pointerMove(svg, { ...touch, clientX: 320, clientY: 205, pointerId: 1 })
    expect(layer()).not.toBe(before)
    fireEvent.pointerUp(svg, { ...touch, clientX: 320, clientY: 205, pointerId: 1 })
    expect(measure()).toBeNull()
  })

  it('mantener apretado y arrastrar mide, sin desplazar el gráfico ni la página', () => {
    vi.useFakeTimers()
    const { svg, measure, layer } = chart()
    const before = layer()
    fireEvent.pointerDown(svg, { ...touch, clientX: 200, clientY: 300, pointerId: 1 })
    act(() => vi.advanceTimersByTime(LONG_PRESS_MS + 10))
    // ya mide: la página no se mueve mientras se arrastra en vertical
    expect(touchMove(svg)).toBe(true)
    fireEvent.pointerMove(svg, { ...touch, clientX: 420, clientY: 120, pointerId: 1 })
    fireEvent.pointerUp(svg, { ...touch, clientX: 420, clientY: 120, pointerId: 1 })
    expect(measure()).not.toBeNull()
    expect(layer()).toBe(before)
  })

  it('si el dedo se mueve antes del toque largo, es un desplazamiento y no mide', () => {
    vi.useFakeTimers()
    const { svg, measure } = chart()
    fireEvent.pointerDown(svg, { ...touch, clientX: 200, clientY: 300, pointerId: 1 })
    fireEvent.pointerMove(svg, { ...touch, clientX: 260, clientY: 300, pointerId: 1 })
    act(() => vi.advanceTimersByTime(LONG_PRESS_MS + 10))
    fireEvent.pointerMove(svg, { ...touch, clientX: 300, clientY: 300, pointerId: 1 })
    fireEvent.pointerUp(svg, { ...touch, clientX: 300, clientY: 300, pointerId: 1 })
    expect(measure()).toBeNull()
  })

  it('con el mouse, quedarse quieto no mide (sigue siendo Shift + arrastrar)', () => {
    vi.useFakeTimers()
    const { svg, measure } = chart()
    fireEvent.pointerDown(svg, {
      pointerType: 'mouse',
      button: 0,
      clientX: 200,
      clientY: 300,
      pointerId: 1,
    })
    act(() => vi.advanceTimersByTime(LONG_PRESS_MS * 2))
    fireEvent.pointerMove(svg, { pointerType: 'mouse', clientX: 420, clientY: 120, pointerId: 1 })
    fireEvent.pointerUp(svg, { pointerType: 'mouse', clientX: 420, clientY: 120, pointerId: 1 })
    expect(measure()).toBeNull()
  })

  it('sobre el eje de precio, el arrastre vertical estira la escala y no mueve la página', () => {
    const { svg } = chart()
    fireEvent.pointerDown(svg, { ...touch, clientX: 700, clientY: 200, pointerId: 1 })
    expect(touchMove(svg)).toBe(true)
    fireEvent.pointerUp(svg, { ...touch, clientX: 700, clientY: 200, pointerId: 1 })
  })

  it('deslizar el dedo sobre el gráfico no cambia de pestaña', () => {
    const onStep = vi.fn()
    function Tabs() {
      useTabSwipe(onStep, true)
      return null
    }
    const { svg } = chart()
    render(<Tabs />)
    const at = (x: number) => ({ clientX: x, clientY: 200 })
    fireEvent.touchStart(svg, { touches: [at(400)] })
    fireEvent.touchEnd(svg, { changedTouches: [at(100)] })
    expect(onStep).not.toHaveBeenCalled()
  })
})

describe('ayuda de gestos según el puntero (#424)', () => {
  beforeEach(() => {
    box(440)
  })
  afterEach(() => vi.restoreAllMocks())

  it('con mouse habla de rueda y Shift', () => {
    const { svg } = chart()
    expect(svg.getAttribute('aria-label')).toMatch(/Ctrl o ⌘ más la rueda/)
    expect(document.body).toHaveTextContent('Shift + arrastrar (o la regla) mide')
    expect(screen.getByRole('button', { name: 'Regla: medir arrastrando' })).toHaveAttribute(
      'title',
      'Medir: activá la regla o usá Shift + arrastrar',
    )
  })

  it('con el dedo habla de deslizar, pellizcar y mantener apretado', () => {
    const restore = coarse()
    try {
      const { svg } = chart()
      expect(svg.getAttribute('aria-label')).toMatch(/pellizcá con dos/)
      expect(svg.getAttribute('aria-label')).not.toMatch(/rueda/)
      expect(document.body).toHaveTextContent('Mantené apretado y arrastrá (o usá la regla)')
      expect(document.body).not.toHaveTextContent('Shift')
      expect(screen.getByRole('button', { name: 'Regla: medir arrastrando' })).toHaveAttribute(
        'title',
        'Medir: activá la regla o mantené apretado y arrastrá',
      )
    } finally {
      restore()
    }
  })

  it('el gráfico de precio también cambia su ayuda', () => {
    const restore = coarse()
    try {
      render(<TradingChart instrument="US100" bars={US100_1H.candles} />)
      const svg = screen.getByRole('img', { name: /US100, \d+ velas/ })
      expect(svg).toHaveClass('touch-pan-y')
      expect(svg.getAttribute('aria-label')).toMatch(/pellizcá con dos/)
    } finally {
      restore()
    }
  })
})
