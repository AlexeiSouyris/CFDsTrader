import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { TradingChart } from '@/components/ui/trading-chart'
import {
  autoPrice,
  clampView,
  MIN_SPAN,
  niceTicks,
  panBy,
  rangeView,
  RIGHT_ROOM,
  stretchPrice,
  timeTicks,
  ViewportStore,
  zoomAt,
} from '@/lib/chart-viewport'
import type { ChartAnalysis } from '@/lib/analysis-contracts'
import { TOOLS_KEY } from '@/lib/analysis-layers'
import { chartBars, mergeBars, type ChartBar } from '@/lib/liquidity-bars'
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

const BARS = chartBars(DATA)

describe('encuadre: zoom, desplazamiento y límites (#401)', () => {
  const n = 300

  it('el zoom deja quieto el punto bajo el cursor', () => {
    const v = { start: 100, end: 200 }
    for (const anchor of [0, 0.3, 0.5, 0.9]) {
      for (const factor of [0.5, 0.8, 1.25]) {
        const z = zoomAt(v, factor, anchor, n)
        const before = v.start + anchor * (v.end - v.start)
        const after = z.start + anchor * (z.end - z.start)
        expect(after).toBeCloseTo(before, 9)
        expect(z.end - z.start).toBeCloseTo(100 * factor, 9)
      }
    }
  })

  it('no acerca más que MIN_SPAN velas ni aleja más que todas', () => {
    expect(zoomAt({ start: 100, end: 120 }, 0.01, 0.5, n).end).toBeCloseTo(110 + MIN_SPAN / 2)
    const z = zoomAt({ start: 100, end: 120 }, 0.01, 0.5, n)
    expect(z.end - z.start).toBe(MIN_SPAN)
    const all = zoomAt({ start: 100, end: 200 }, 100, 0.5, n)
    expect(all.end - all.start).toBe(n)
    expect(all.start).toBeGreaterThanOrEqual(0)
    // con menos velas que MIN_SPAN, se ven todas
    expect(clampView({ start: 0, end: 2 }, 3)).toEqual({ start: 0, end: 3 })
  })

  it('el desplazamiento para en la primera vela y deja poco lugar vacío a la derecha', () => {
    expect(panBy({ start: 10, end: 110 }, -50, n)).toEqual({ start: 0, end: 100 })
    const right = panBy({ start: 150, end: 250 }, 1000, n)
    expect(right.end).toBeCloseTo(n + 100 * RIGHT_ROOM)
    expect(right.end - right.start).toBe(100)
    expect(rangeView(Infinity, n)).toEqual({ start: 0, end: n })
    expect(rangeView(72, n)).toEqual({ start: n - 72, end: n })
  })

  it('escala de precio: automática con lo visible, estirada a mano alrededor del centro', () => {
    const lows = [10, 1, 20, 30]
    const highs = [11, 50, 21, 31]
    const r = autoPrice(lows, highs, { start: 2, end: 4 })!
    // solo las velas 2 y 3: de 20 a 31, con 8 % de margen
    expect(r.lo).toBeCloseTo(20 - 11 * 0.08)
    expect(r.hi).toBeCloseTo(31 + 11 * 0.08)
    const s = stretchPrice({ lo: 90, hi: 110 }, 2)
    expect(s).toEqual({ lo: 80, hi: 120 })
    expect(autoPrice([NaN], [NaN], { start: 0, end: 1 })).toBeNull()
  })

  it('precios y fechas del eje: redondos y pegados a su vela', () => {
    expect(niceTicks(20012.3, 20088.1).ticks).toEqual([20020, 20040, 20060, 20080])
    expect(niceTicks(1.1, 1.2).digits).toBe(2)
    const a = timeTicks({ start: 10, end: 110 }, 300)
    const b = timeTicks({ start: 13, end: 113 }, 300)
    // al desplazar, las fechas que siguen a la vista son las mismas velas
    expect(b.filter((i) => a.includes(i)).length).toBeGreaterThan(0)
    expect(new Set(a.map((i) => i % 24))).toEqual(new Set([0]))
  })

  it('la inercia sigue después de soltar, frena sola y para en el borde', () => {
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => frames.push(cb))
    vi.stubGlobal('cancelAnimationFrame', () => frames.splice(0))
    const store = new ViewportStore({ start: 100, end: 200 }, n)
    store.fling(-0.05)
    let t = performance.now()
    let prev = store.get().start
    const steps: number[] = []
    for (let k = 0; k < 400 && frames.length; k++) {
      t += 16
      frames.shift()!(t)
      steps.push(prev - store.get().start)
      prev = store.get().start
    }
    expect(steps[0]).toBeGreaterThan(0)
    // cada paso más corto que el anterior (frena), y termina
    expect(steps[5]).toBeLessThan(steps[0])
    expect(frames).toHaveLength(0)
    expect(store.get().start).toBeLessThan(100)
    expect(store.get().user).toBe(true)
    vi.unstubAllGlobals()
  })

  it('al sumar velas viejas al principio el encuadre se corre y sigue mostrando lo mismo', () => {
    const store = new ViewportStore({ start: 100, end: 200 }, n)
    store.setCount(n + 50, 50)
    expect(store.get()).toMatchObject({ start: 150, end: 250 })
    const a = BARS.slice(0, 3)
    expect(mergeBars(BARS.slice(2, 5), a).map((b) => b.start)).toEqual(
      BARS.slice(0, 5).map((b) => b.start),
    )
  })
})

describe('gráficos navegables (#401)', () => {
  beforeEach(() => {
    box(440)
  })
  afterEach(() => {
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  const chart = (loadOlder?: (before: string) => Promise<ChartBar[]>, bars = BARS) => {
    const utils = render(
      <LiquidityChart
        instrument="US100"
        bars={bars}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
        loadOlder={loadOlder}
      />,
    )
    const svg = screen.getByRole('img', { name: /US100, \d+ velas/ })
    const count = () => Number(/, (\d+) velas/.exec(svg.getAttribute('aria-label')!)![1])
    const hovered = (x: number) => {
      fireEvent.pointerMove(svg, { clientX: x, clientY: 200 })
      return utils.container.querySelector('[data-hover-dot]')?.getAttribute('data-hover-dot')
    }
    return { ...utils, svg, count, hovered }
  }

  it('arrastrar desplaza al pasado sin volver a dibujar las velas y sin medir', () => {
    const { container, svg, hovered } = chart()
    const before = hovered(400)!
    const candle = container.querySelector('[data-candle]')
    fireEvent.pointerDown(svg, { clientX: 200, clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: 260, clientY: 200, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: 320, clientY: 200, pointerId: 1 })
    // mientras se arrastra, las velas se mueven con la transformación de la capa
    expect(container.querySelector('[data-price-layer]')!.getAttribute('transform')).toMatch(
      /^matrix\(1 0 0 [\d.]+ [\d.]+ /,
    )
    expect(container.querySelector('[data-candle]')).toBe(candle)
    fireEvent.pointerUp(svg, { clientX: 320, clientY: 200, pointerId: 1 })
    expect(container.querySelector('[data-measure]')).toBeNull()
    // bajo el mismo lugar ahora hay una vela anterior
    const after = hovered(400)!
    expect(Date.parse(after)).toBeLessThan(Date.parse(before))
    // y aparece el botón para volver a la última vela
    fireEvent.click(screen.getByRole('button', { name: 'Ir a la última vela' }))
  })

  it('el botón «ir a la última vela» se desliza hasta ella', async () => {
    const { svg } = chart()
    fireEvent.keyDown(svg, { key: 'Home' })
    fireEvent.click(screen.getByRole('button', { name: 'Ir a la última vela' }))
    await act(() => new Promise((r) => setTimeout(r, 700)))
    expect(screen.queryByRole('button', { name: 'Ir a la última vela' })).toBeNull()
  })

  it('Shift + arrastrar mide; la regla también; el arrastre simple no', () => {
    const { container, svg } = chart()
    const measure = () => container.querySelector('[data-measure]')
    const drag = (shiftKey: boolean) => {
      fireEvent.pointerDown(svg, { clientX: 150, clientY: 300, button: 0, pointerId: 1, shiftKey })
      fireEvent.pointerMove(svg, { clientX: 400, clientY: 100, pointerId: 1, shiftKey })
      fireEvent.pointerUp(svg, { clientX: 400, clientY: 100, pointerId: 1, shiftKey })
    }
    drag(false)
    expect(measure()).toBeNull()
    drag(true)
    expect(measure()).not.toBeNull()
    // un clic sin arrastre la borra, como antes
    fireEvent.pointerDown(svg, { clientX: 300, clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 300, clientY: 200, pointerId: 1 })
    expect(measure()).toBeNull()
    const ruler = screen.getByRole('button', { name: 'Regla: medir arrastrando' })
    fireEvent.click(ruler)
    expect(ruler).toHaveAttribute('aria-pressed', 'true')
    drag(false)
    expect(measure()).not.toBeNull()
  })

  it('la rueda acerca y aleja anclada en el cursor', () => {
    const { svg, count, hovered } = chart()
    const n0 = count()
    const under = hovered(200)
    fireEvent.wheel(svg, { deltaY: -400, clientX: 200, clientY: 200 })
    expect(count()).toBeLessThan(n0)
    // la vela bajo el cursor es la misma
    expect(hovered(200)).toBe(under)
    fireEvent.wheel(svg, { deltaY: 400, clientX: 200, clientY: 200 })
    expect(count()).toBeGreaterThan(n0 / 2)
    // el pellizco del trackpad (ctrl + rueda) también, y el desplazamiento horizontal mueve
    const n1 = count()
    fireEvent.wheel(svg, { deltaY: -60, ctrlKey: true, clientX: 200, clientY: 200 })
    expect(count()).toBeLessThan(n1)
    const u = hovered(300)!
    fireEvent.wheel(svg, { deltaX: -200, clientX: 300, clientY: 200 })
    vi.useFakeTimers()
    act(() => vi.advanceTimersByTime(500))
    vi.useRealTimers()
    expect(Date.parse(hovered(300)!)).toBeLessThan(Date.parse(u))
  })

  it('escala de precio: arrastre vertical sobre el eje la vuelve manual y el doble clic, automática', () => {
    const { container, svg } = chart()
    const ticks = () =>
      [...container.querySelectorAll('[data-axis-tick]')].map((t) => t.textContent).join('|')
    const auto = ticks()
    expect(screen.queryByRole('button', { name: /Escala auto/ })).toBeNull()
    fireEvent.pointerDown(svg, { clientX: 700, clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: 700, clientY: 320, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 700, clientY: 320, pointerId: 1 })
    // comprimida: entran más precios, el eje cambia y queda en manual
    expect(ticks()).not.toBe(auto)
    expect(screen.getByRole('button', { name: /Escala auto/ })).toBeInTheDocument()
    // en manual, desplazar no reajusta la escala sola
    const manual = ticks()
    fireEvent.pointerDown(svg, { clientX: 200, clientY: 200, button: 0, pointerId: 2 })
    fireEvent.pointerMove(svg, { clientX: 300, clientY: 200, pointerId: 2 })
    fireEvent.pointerUp(svg, { clientX: 300, clientY: 200, pointerId: 2 })
    expect(ticks()).toBe(manual)
    fireEvent.doubleClick(svg, { clientX: 700, clientY: 200 })
    expect(screen.queryByRole('button', { name: /Escala auto/ })).toBeNull()
  })

  it('teclas con el foco: ← → desplazan, + − acercan y alejan, Fin vuelve a la última', () => {
    const { svg, count } = chart()
    const n0 = count()
    fireEvent.keyDown(svg, { key: '+' })
    expect(count()).toBeLessThan(n0)
    fireEvent.keyDown(svg, { key: '-' })
    fireEvent.keyDown(svg, { key: '-' })
    expect(count()).toBeGreaterThan(n0 * 0.9)
    expect(screen.queryByRole('button', { name: 'Ir a la última vela' })).toBeNull()
    fireEvent.keyDown(svg, { key: 'ArrowLeft' })
    expect(screen.getByRole('button', { name: 'Ir a la última vela' })).toBeInTheDocument()
    fireEvent.keyDown(svg, { key: 'End' })
    expect(screen.queryByRole('button', { name: 'Ir a la última vela' })).toBeNull()
  })

  it('el botón de rango pone el encuadre y queda marcado; al moverse se desmarca', () => {
    const { svg, count } = chart()
    const group = screen.getByRole('group', { name: 'Rango' })
    expect(within(group).getByRole('button', { name: '2 días' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    fireEvent.keyDown(svg, { key: '+' })
    expect(within(group).getByRole('button', { name: '2 días' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
    fireEvent.click(within(group).getByRole('button', { name: '1 día' }))
    expect(count()).toBe(96)
    expect(within(group).getByRole('button', { name: '1 día' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('al llegar al borde izquierdo pide velas anteriores al gateway y no salta', async () => {
    const older = BARS.slice(0, 100)
    const shown = BARS.slice(100)
    const calls: string[] = []
    let answer: ChartBar[] = older
    const loadOlder = vi.fn(async (before: string) => {
      calls.push(before)
      return answer
    })
    const { container, svg, hovered } = chart(loadOlder, shown)
    // abrir no pide nada
    expect(loadOlder).not.toHaveBeenCalled()
    fireEvent.keyDown(svg, { key: 'Home' })
    expect(calls).toEqual([shown[0].start])
    expect(container.querySelector('[data-history="loading"]')).not.toBeNull()
    const under = hovered(300)
    await act(async () => {})
    // llegaron 100 velas más viejas: lo que se veía sigue en el mismo lugar
    expect(hovered(300)).toBe(under)
    expect(container.querySelector('[data-history]')).toBeNull()
    // seguir hacia atrás pide desde la primera nueva; si no hay más, lo dice
    answer = []
    fireEvent.keyDown(svg, { key: 'Home' })
    expect(calls).toEqual([shown[0].start, older[0].start])
    await act(async () => {})
    expect(container.querySelector('[data-history="done"]')).not.toBeNull()
    fireEvent.keyDown(svg, { key: 'ArrowLeft' })
    expect(loadOlder).toHaveBeenCalledTimes(2)
  })

  it('las velas más viejas traen su análisis del motor y se suma al de la vista (#511)', async () => {
    localStorage.setItem(TOOLS_KEY, JSON.stringify({ wyckoff: true }))
    const older = BARS.slice(0, 100)
    const shown = BARS.slice(100)
    // Solo un spring en cada pedido (valores de prueba: acá no se calcula nada).
    const block = (n: number, at: number): ChartAnalysis => ({
      ema_daily_period: 365,
      ema_daily: Array.from({ length: n }, () => null),
      fibonacci: [],
      golden_pocket: [],
      wyckoff: [
        {
          kind: 'spring',
          break_index: at - 1,
          confirmed_index: at,
          level: '95.0',
          touches: 3,
          extreme: '94.5',
        },
      ],
      bottoms: [],
      ma_cross: {
        kind: 'sma',
        fast_period: 50,
        slow_period: 200,
        fast: Array.from({ length: n }, () => null),
        slow: Array.from({ length: n }, () => null),
        crosses: [],
      },
      floor_pivots: [],
      vix: Array.from({ length: n }, () => null),
    })
    const loadOlder = vi.fn(async () => ({ bars: older, analysis: block(older.length, 10) }))
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={shown}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
        analysis={block(shown.length, 5)}
        loadOlder={loadOlder}
      />,
    )
    const svg = screen.getByRole('img', { name: /US100, \d+ velas/ })
    const springs = () => container.querySelectorAll('[data-marker="spring"]').length
    expect(springs()).toBe(1)
    fireEvent.keyDown(svg, { key: 'Home' })
    await act(async () => {})
    expect(loadOlder).toHaveBeenCalledTimes(1)
    expect(springs()).toBe(2)
    localStorage.clear()
  })

  it('el gráfico de precio también se desplaza y mide con Shift', () => {
    vi.restoreAllMocks()
    box(520)
    const { container } = render(<TradingChart instrument="US100" bars={US100_1H.candles} />)
    const svg = screen.getByRole('img', { name: /US100, \d+ velas de 1 hora/ })
    fireEvent.pointerDown(svg, { clientX: 200, clientY: 100, button: 0, pointerId: 1 })
    fireEvent.pointerMove(svg, { clientX: 400, clientY: 100, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 400, clientY: 100, pointerId: 1 })
    expect(container.querySelector('[data-measure]')).toBeNull()
    expect(screen.getByRole('button', { name: 'Ir a la última vela' })).toBeInTheDocument()
    fireEvent.pointerDown(svg, {
      clientX: 200,
      clientY: 100,
      button: 0,
      pointerId: 1,
      shiftKey: true,
    })
    fireEvent.pointerMove(svg, { clientX: 400, clientY: 120, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 400, clientY: 120, pointerId: 1 })
    expect(container.querySelector('[data-measure]')).not.toBeNull()
  })
})
