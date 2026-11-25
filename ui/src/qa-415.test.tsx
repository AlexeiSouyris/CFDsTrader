/**
 * Revisión de calidad de todas las vistas (#415): un test por cada arreglo.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { NotFound } from '@/components/ui/not-found'
import { timeTickLabel } from '@/lib/format'
import { useElementWidth } from '@/lib/use-element-width'
import { useTabSwipe } from '@/lib/use-tab-swipe'

afterEach(() => {
  vi.restoreAllMocks()
  history.replaceState(null, '', '/')
})

function Swipe({ onStep }: { onStep: (d: 1 | -1) => void }) {
  useTabSwipe(onStep, true, { cooldownMs: 0 })
  return (
    <main>
      <div data-testid="chart" className="touch-none" />
      <div data-testid="owner" />
      <div data-testid="free" />
    </main>
  )
}

describe('gestos: el gráfico contra el swipe de pestañas (#415)', () => {
  it('el desplazamiento horizontal que ya usó un gráfico no cambia de pestaña', () => {
    const onStep = vi.fn()
    render(<Swipe onStep={onStep} />)
    // un gráfico que se desplaza con la rueda cancela el evento, como `useChartNav`
    const owner = screen.getByTestId('owner')
    owner.addEventListener('wheel', (e) => e.preventDefault())
    fireEvent.wheel(owner, { deltaX: 300, deltaY: 0 })
    expect(onStep).not.toHaveBeenCalled()
    // sobre un gráfico (`touch-none`) tampoco, aunque no lo haya cancelado
    fireEvent.wheel(screen.getByTestId('chart'), { deltaX: 300, deltaY: 0 })
    expect(onStep).not.toHaveBeenCalled()
    // fuera de los gráficos, el gesto sigue pasando de pestaña
    fireEvent.wheel(screen.getByTestId('free'), { deltaX: 300, deltaY: 0 })
    expect(onStep).toHaveBeenCalledWith(1)
  })
})

describe('barra de vistas del celular (#415)', () => {
  it('cada pestaña crece hasta su rótulo y la activa se trae a la vista', () => {
    const scrollTo = vi.fn()
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
      configurable: true,
      value: scrollTo,
    })
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(712)
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(349)
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(60)
    vi.spyOn(HTMLElement.prototype, 'offsetLeft', 'get').mockReturnValue(640)
    history.replaceState(null, '', '/#glosario')
    render(<App initialStage="app" />)
    const nav = screen.getByRole('navigation', { name: 'Vistas' })
    // la activa (Glosario) queda centrada en la barra
    expect(scrollTo).toHaveBeenCalledWith({ left: 640 - (349 - 60) / 2 })
    const link = within(nav).getByRole('link', { name: 'Performance' })
    // el rótulo no se corta: sin ancho fijo, sin achicarse y en una línea
    expect(link).toHaveClass('flex-[1_0_auto]', 'whitespace-nowrap')
    expect(link).not.toHaveClass('flex-1')
    delete (HTMLElement.prototype as { scrollTo?: unknown }).scrollTo
  })
})

describe('eje de tiempo de los gráficos (#415)', () => {
  it('fecha en la primera marca del día y hora UTC en las demás', () => {
    expect(timeTickLabel('2026-11-20T00:00:00Z')).toBe('20 nov')
    expect(timeTickLabel('2026-11-20T12:00:00Z', '2026-11-20T00:00:00Z')).toBe('12:00')
    expect(timeTickLabel('2026-11-21T00:00:00Z', '2026-11-20T12:00:00Z')).toBe('21 nov')
    // velas diarias: cada marca es otro día, así que siempre fecha
    expect(timeTickLabel('2026-07-02T00:00:00Z', '2026-06-02T00:00:00Z')).toBe('02 jul')
  })
})

function Width() {
  const [ref, width] = useElementWidth<HTMLDivElement>(720, 260)
  return (
    <div ref={ref} data-testid="box">
      {width}
    </div>
  )
}

describe('ancho de los gráficos SVG (#415)', () => {
  it('el primer cuadro ya usa el ancho real, no el inicial', () => {
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(283)
    render(<Width />)
    expect(screen.getByTestId('box')).toHaveTextContent('283')
  })

  it('sin ancho medible (jsdom, oculto) queda el inicial', () => {
    render(<Width />)
    expect(screen.getByTestId('box')).toHaveTextContent('720')
  })
})

describe('404 y aviso del pie (#415)', () => {
  it('los botones del 404 no parten su texto y bajan de línea si no entran', () => {
    render(<NotFound onHome={() => {}} onExplore={() => {}} />)
    const home = screen.getByRole('button', { name: 'Ir al inicio' })
    expect(home).toHaveClass('whitespace-nowrap')
    expect(home.parentElement).toHaveClass('flex-wrap')
  })

  it('el aviso del pie no dice que la app está desconectada del motor', () => {
    render(<App initialStage="app" />)
    const note = screen
      .getAllByRole('note')
      .find((n) => n.textContent?.includes('simulación (paper)'))!
    expect(note).not.toHaveTextContent('no está conectado al motor')
    expect(note).toHaveTextContent('simulación (paper)')
    expect(note).toHaveTextContent('no calcula nada ni envía órdenes reales')
  })
})
