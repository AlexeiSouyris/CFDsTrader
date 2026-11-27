import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { exampleChart } from '@/test/example-chart'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import {
  drawingStore,
  MAX_DRAWINGS,
  parseDrawings,
  posAt,
  timeAt,
  type Drawing,
  type DrawTool,
} from '@/lib/chart-drawings'
import { chartBars } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA } from '@/mocks'
import { MultichartView } from '@/views/multichart'

const KEY = 'cfdstrader.drawings.US100'
const H = Date.parse('2026-01-05T10:00:00Z')
const M15 = 15 * 60_000
// tres velas de 15 m seguidas y una después de un hueco de una hora
const starts = [H, H + M15, H + 2 * M15, H + 4 * M15 + 3_600_000]
const ends = starts.map((s) => s + M15)

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

const hline = (id: string, p: number): Drawing => ({
  id,
  kind: 'hline',
  a: { t: '2026-01-05T10:00:00.000Z', p },
  b: { t: '2026-01-05T10:00:00.000Z', p },
})

describe('dibujos: hora ↔ posición (#543)', () => {
  it('ancla a la hora UTC: dentro de la vela se reparte entre inicio y cierre', () => {
    expect(timeAt(starts, ends, 0)).toBe(H)
    expect(timeAt(starts, ends, 1.5)).toBe(H + M15 + M15 / 2)
    expect(posAt(starts, ends, H + M15 + M15 / 2)).toBe(1.5)
    // ida y vuelta, con lo que sea dentro de las velas
    for (const f of [0, 0.25, 1, 2.75, 3.5])
      expect(posAt(starts, ends, timeAt(starts, ends, f))).toBeCloseTo(f)
  })

  it('antes de la primera y después de la última sigue con la duración de esa vela', () => {
    expect(timeAt(starts, ends, -2)).toBe(H - 2 * M15)
    expect(posAt(starts, ends, H - 2 * M15)).toBe(-2)
    expect(timeAt(starts, ends, 6)).toBe(starts[3] + 3 * M15)
    expect(posAt(starts, ends, starts[3] + 3 * M15)).toBe(6)
  })

  it('una hora en un hueco (sin velas) cae al inicio de la vela siguiente', () => {
    expect(posAt(starts, ends, H + 3 * M15 + 1000)).toBe(3)
  })

  it('sin velas o con una hora inválida, no hay posición', () => {
    expect(posAt([], [], H)).toBeNaN()
    expect(timeAt([], [], 1)).toBeNaN()
    expect(posAt(starts, ends, NaN)).toBeNaN()
  })
})

describe('dibujos: guardado por instrumento (#543)', () => {
  beforeEach(() => drawingStore.reset())
  afterEach(() => {
    localStorage.clear()
    drawingStore.reset()
    vi.restoreAllMocks()
  })

  it('descarta lo mal formado y tiene un tope', () => {
    expect(parseDrawings('x')).toEqual([])
    expect(
      parseDrawings([
        hline('a', 1),
        { id: 'b', kind: 'circle', a: { t: 'x', p: 1 }, b: { t: 'x', p: 1 } },
        { id: 'c', kind: 'trend', a: { t: 'nope', p: 1 }, b: { t: '2026-01-01T00:00:00Z', p: 2 } },
        { id: 'd', kind: 'rect', a: { t: '2026-01-01T00:00:00Z', p: '1' } },
      ]).map((d) => d.id),
    ).toEqual(['a'])
    const many = Array.from({ length: MAX_DRAWINGS + 5 }, (_, i) => hline(`h${i}`, i))
    expect(parseDrawings(many)).toHaveLength(MAX_DRAWINGS)
  })

  it('agrega, borra y limpia; cada instrumento tiene lo suyo', () => {
    drawingStore.add('US100', hline('a', 1))
    drawingStore.add('US100', hline('b', 2))
    drawingStore.add('DE40', hline('c', 3))
    expect(JSON.parse(localStorage.getItem(KEY)!).map((d: Drawing) => d.id)).toEqual(['a', 'b'])
    drawingStore.remove('US100', 'a')
    expect(drawingStore.get('US100').map((d) => d.id)).toEqual(['b'])
    drawingStore.clear('US100')
    expect(localStorage.getItem(KEY)).toBeNull()
    expect(drawingStore.get('DE40')).toHaveLength(1)
    // se lee de nuevo desde el navegador
    drawingStore.reset()
    expect(drawingStore.get('DE40')[0].id).toBe('c')
  })

  it('si el navegador no deja guardar, sigue andando en memoria', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    drawingStore.add('US100', hline('a', 1))
    expect(drawingStore.get('US100')).toHaveLength(1)
  })
})

describe('dibujos sobre el gráfico (#543)', () => {
  beforeEach(() => {
    drawingStore.reset()
    box(440)
  })
  afterEach(() => {
    localStorage.clear()
    drawingStore.reset()
    vi.restoreAllMocks()
  })

  /** El gráfico con la herramienta manejada desde afuera, como el Multigráfico. */
  function Harness({ initial }: { initial: DrawTool }) {
    const [tool, setTool] = useState<DrawTool>(initial)
    return (
      <>
        <span data-testid="tool">{tool}</span>
        <button type="button" onClick={() => setTool('trend')}>
          tendencia
        </button>
        <button type="button" onClick={() => setTool('rect')}>
          rectángulo
        </button>
        <LiquidityChart
          instrument="US100"
          bars={chartBars(DATA)}
          levels={DATA.levels}
          fvgs={DATA.fvgs}
          imbalance={DATA.imbalance}
          drawTool={tool}
          onDrawToolChange={setTool}
        />
      </>
    )
  }
  const surface = () => document.querySelector('[data-drawing-input] rect')!
  const shapes = () => [...document.querySelectorAll('[data-drawing]')]
  const dragOn = (el: Element, x0: number, y0: number, x1: number, y1: number) => {
    fireEvent.pointerDown(el, { clientX: x0, clientY: y0, button: 0, pointerId: 1 })
    fireEvent.pointerMove(el, { clientX: x1, clientY: y1, pointerId: 1 })
    fireEvent.pointerUp(el, { clientX: x1, clientY: y1, pointerId: 1 })
  }

  it('sin la prop, el gráfico no tiene capa de dibujos (los demás usos no cambian)', () => {
    drawingStore.add('US100', hline('a', 20000))
    render(
      <LiquidityChart
        instrument="US100"
        bars={chartBars(DATA)}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
      />,
    )
    expect(document.querySelector('[data-drawings]')).toBeNull()
    expect(document.querySelector('[data-drawing-input]')).toBeNull()
  })

  it('línea horizontal con un clic: se guarda con hora UTC y precio, y vuelve a navegar', () => {
    render(<Harness initial="hline" />)
    const el = surface()
    fireEvent.pointerDown(el, { clientX: 200, clientY: 200, button: 0, pointerId: 1 })
    // mientras se arrastra se ve el borrador
    expect(document.querySelector('[data-drawing-draft]')).not.toBeNull()
    fireEvent.pointerUp(el, { clientX: 200, clientY: 200, pointerId: 1 })

    const saved = JSON.parse(localStorage.getItem(KEY)!) as Drawing[]
    expect(saved).toHaveLength(1)
    expect(saved[0].kind).toBe('hline')
    expect(saved[0].a.t).toMatch(/Z$/)
    expect(Number.isFinite(saved[0].a.p)).toBe(true)
    expect(screen.getByTestId('tool')).toHaveTextContent('none')
    // queda seleccionada y la superficie de dibujo se fue
    expect(shapes()).toHaveLength(1)
    expect(shapes()[0]).toHaveAttribute('data-selected', 'true')
    expect(document.querySelector('[data-drawing-input]')).toBeNull()
    // y está a la altura donde se hizo clic
    expect(Number(shapes()[0].querySelector('line')!.getAttribute('y1'))).toBeCloseTo(200, 0)
  })

  it('tendencia y rectángulo: se dibujan arrastrando; un clic sin arrastre no dibuja', () => {
    render(<Harness initial="none" />)
    fireEvent.click(screen.getByText('tendencia'))
    fireEvent.pointerDown(surface(), { clientX: 100, clientY: 100, button: 0, pointerId: 1 })
    fireEvent.pointerUp(surface(), { clientX: 101, clientY: 100, pointerId: 1 })
    expect(shapes()).toHaveLength(0)

    dragOn(surface(), 100, 300, 400, 120)
    expect(shapes().map((s) => s.getAttribute('data-drawing'))).toEqual(['trend'])
    const ln = shapes()[0].querySelector('line')!
    expect(Number(ln.getAttribute('x1'))).toBeCloseTo(100, 0)
    expect(Number(ln.getAttribute('y2'))).toBeCloseTo(120, 0)

    fireEvent.click(screen.getByText('rectángulo'))
    dragOn(surface(), 300, 150, 200, 250)
    expect(shapes().map((s) => s.getAttribute('data-drawing'))).toEqual(['trend', 'rect'])
    const r = shapes()[1].querySelector('rect')!
    expect(Number(r.getAttribute('x'))).toBeCloseTo(200, 0)
    expect(Number(r.getAttribute('width'))).toBeCloseTo(100, 0)
    // la hora del punto es la de la vela bajo el cursor (anclado a la hora, no a píxeles)
    const saved = JSON.parse(localStorage.getItem(KEY)!) as Drawing[]
    expect(Date.parse(saved[1].a.t)).toBeGreaterThan(Date.parse(saved[1].b.t))
  })

  it('siguen a las velas al hacer zoom (anclados a hora y precio)', () => {
    render(<Harness initial="trend" />)
    dragOn(surface(), 300, 300, 500, 150)
    const svg = screen.getByRole('img', { name: /US100/ })
    const x1 = () => Number(shapes()[0].querySelector('line')!.getAttribute('x1'))
    const before = x1()
    svg.focus()
    act(() => {
      fireEvent.keyDown(svg, { key: '+' })
    })
    // con el foco, + acerca: el dibujo se mueve con las velas
    expect(x1()).not.toBeCloseTo(before, 0)
  })

  it('seleccionar y borrar: con Supr o con el botón; un clic afuera deselecciona', () => {
    drawingStore.add('US100', hline('a', Number(chartBars(DATA).at(-1)!.close)))
    drawingStore.add('US100', hline('b', Number(chartBars(DATA).at(-1)!.close) + 1))
    render(<Harness initial="none" />)
    const svg = screen.getByRole('img', { name: /US100/ })

    // con navegar, un clic sobre el dibujo lo selecciona (y no desplaza el gráfico)
    fireEvent.pointerDown(shapes()[0], { clientX: 100, clientY: 100, button: 0, pointerId: 1 })
    expect(shapes()[0]).toHaveAttribute('data-selected', 'true')
    // un clic en otro lado del gráfico deselecciona
    fireEvent.pointerDown(svg, { clientX: 100, clientY: 400, button: 0, pointerId: 2 })
    fireEvent.pointerUp(svg, { clientX: 100, clientY: 400, pointerId: 2 })
    expect(document.querySelector('[data-selected]')).toBeNull()

    fireEvent.pointerDown(shapes()[0], { clientX: 100, clientY: 100, button: 0, pointerId: 3 })
    fireEvent.keyDown(svg, { key: 'Delete' })
    expect(shapes().map((s) => s.getAttribute('data-drawing'))).toHaveLength(1)
    expect(drawingStore.get('US100').map((d) => d.id)).toEqual(['b'])

    fireEvent.pointerDown(shapes()[0], { clientX: 100, clientY: 100, button: 0, pointerId: 4 })
    fireEvent.click(screen.getByRole('button', { name: 'Borrar dibujo' }))
    expect(shapes()).toHaveLength(0)
    expect(localStorage.getItem(KEY)).toBeNull()
  })

  it('con una herramienta de dibujo o la regla, los dibujos no se seleccionan', () => {
    drawingStore.add('US100', hline('a', 1))
    render(<Harness initial="rect" />)
    expect(shapes()[0]).toHaveAttribute('pointer-events', 'none')
  })
})

describe('dibujos en el Multigráfico (#543)', () => {
  beforeEach(() => {
    drawingStore.reset()
    box(500)
  })
  afterEach(() => {
    localStorage.clear()
    drawingStore.reset()
    vi.restoreAllMocks()
  })
  const drawBar = () => screen.getByRole('toolbar', { name: /Herramientas de dibujo/ })

  it('la barra tiene las herramientas de dibujo y "borrar todo" actúa sobre el instrumento del activo', async () => {
    render(<MultichartView />)
    await exampleChart()
    const bar = drawBar()
    for (const name of ['Línea horizontal', 'Línea de tendencia', 'Rectángulo'])
      expect(within(bar).getByRole('button', { name })).toHaveAttribute('aria-pressed', 'false')
    const clear = within(bar).getByRole('button', { name: /Borrar todos los dibujos de US100/ })
    expect(clear).toBeDisabled()

    fireEvent.click(within(bar).getByRole('button', { name: 'Línea horizontal' }))
    expect(within(bar).getByRole('button', { name: 'Línea horizontal' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(within(bar).getByRole('button', { name: 'Navegar' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
    const first = document.querySelector('[data-chart-slot="0"]') as HTMLElement
    const input = first.querySelector('[data-drawing-input] rect')!
    fireEvent.pointerDown(input, { clientX: 200, clientY: 200, button: 0, pointerId: 1 })
    fireEvent.pointerUp(input, { clientX: 200, clientY: 200, pointerId: 1 })
    expect(first.querySelectorAll('[data-drawing]')).toHaveLength(1)
    // al terminar vuelve a navegar
    expect(within(bar).getByRole('button', { name: 'Navegar' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )

    // Medir apaga la herramienta de dibujo
    fireEvent.click(within(bar).getByRole('button', { name: 'Rectángulo' }))
    fireEvent.click(within(bar).getByRole('button', { name: 'Medir' }))
    expect(within(bar).getByRole('button', { name: 'Rectángulo' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )

    expect(clear).toBeEnabled()
    fireEvent.click(clear)
    expect(first.querySelectorAll('[data-drawing]')).toHaveLength(0)
    expect(localStorage.getItem(KEY)).toBeNull()
  })
})
