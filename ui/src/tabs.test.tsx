import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { exampleChart } from '@/test/example-chart'
import App from '@/App'
import { AnimatedNumber } from '@/components/ui/animated-number'
import { decimal } from '@/lib/format'
import { CURRENT_MEMBER, BACKTEST_RUNS } from '@/mocks'
import { TerminalView } from '@/views/terminal'

const heading = () => screen.getByRole('heading', { level: 1 })
const tabs = () => screen.getByRole('navigation', { name: 'Secciones' })

/** Deslizamiento horizontal con dos dedos: varios `wheel` con deltaX dominante. */
function swipe(el: Element | Window, dx: number, steps = 3) {
  for (let i = 0; i < steps; i++) fireEvent.wheel(el, { deltaX: dx / steps, deltaY: 0 })
}

describe('pestañas principales (#286)', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
    localStorage.clear()
  })

  it('Performance, Inicio y Terminal, con su hash y en el historial', async () => {
    render(<App initialStage="app" />)
    const links = within(tabs()).getAllByRole('link')
    expect(links.map((a) => a.textContent)).toEqual(['Performance', 'Inicio', 'Terminal'])
    expect(heading()).toHaveTextContent('Inicio')
    expect(within(tabs()).getByRole('link', { name: 'Inicio' })).toHaveAttribute(
      'aria-current',
      'page',
    )

    const before = history.length
    fireEvent.click(within(tabs()).getByRole('link', { name: 'Performance' }))
    expect(location.hash).toBe('#performance')
    expect(heading()).toHaveTextContent('Performance')
    expect(history.length).toBe(before + 1)

    fireEvent.click(within(tabs()).getByRole('link', { name: 'Terminal' }))
    expect(location.hash).toBe('#terminal')
    // atrás del navegador (o el gesto de macOS) vuelve a la pestaña anterior
    act(() => history.back())
    await waitFor(() => expect(heading()).toHaveTextContent('Performance'))
  })

  it('el Panel reúne el panel de siempre y la liquidez; las demás vistas siguen', () => {
    history.replaceState(null, '', '/#panel')
    render(<App initialStage="app" />)
    expect(screen.getByRole('heading', { level: 2, name: 'Liquidez' })).toBeInTheDocument()
    expect(screen.getByText('Competencia de la semana')).toBeInTheDocument()
    const nav = screen.getByRole('navigation', { name: 'Vistas' })
    fireEvent.click(within(nav).getByText('Liquidez'))
    expect(location.hash).toBe('#liquidez')
    expect(heading()).toHaveTextContent('Liquidez')
  })

  it('Performance muestra las corridas de backtest tal como llegan, sin prometer nada', () => {
    history.replaceState(null, '', '/#performance')
    render(<App initialStage="app" />)
    const table = screen.getByRole('table', { name: 'Corridas de backtest' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(BACKTEST_RUNS.length)
    expect(rows[0]).toHaveTextContent('+4,12 %')
    expect(rows[1]).toHaveTextContent('−1,25 %')
    expect(rows[2]).toHaveTextContent('0,00 %')
    expect(rows[2]).not.toHaveTextContent('+0,00')
    expect(document.body).toHaveTextContent('no anticipan lo que va a pasar')
    expect(document.body).not.toHaveTextContent(/ganancia asegurada|garantizad/i)
  })

  it('el deslizamiento horizontal pasa de pestaña, con umbral y tiempo de espera', () => {
    vi.useFakeTimers({ toFake: ['Date'] })
    render(<App initialStage="app" />)
    const main = screen.getByRole('main')

    // poco movimiento: no alcanza el umbral
    fireEvent.wheel(main, { deltaX: 40, deltaY: 0 })
    expect(heading()).toHaveTextContent('Inicio')
    // vertical dominante: es scroll de la página
    fireEvent.wheel(main, { deltaX: 200, deltaY: 400 })
    expect(heading()).toHaveTextContent('Inicio')

    vi.advanceTimersByTime(1000)
    swipe(main, 180)
    expect(heading()).toHaveTextContent('Terminal')
    expect(location.hash).toBe('#terminal')
    // la inercia del mismo gesto no salta otra (y Terminal es la última)
    swipe(main, -180)
    expect(heading()).toHaveTextContent('Terminal')

    vi.advanceTimersByTime(1000)
    swipe(main, -180)
    expect(heading()).toHaveTextContent('Inicio')
    vi.advanceTimersByTime(1000)
    swipe(main, -180)
    expect(heading()).toHaveTextContent('Performance')
  })

  it('no se dispara dentro de algo con scroll horizontal propio ni con Ctrl (zoom)', () => {
    vi.useFakeTimers({ toFake: ['Date'] })
    history.replaceState(null, '', '/#performance')
    render(<App initialStage="app" />)
    const table = screen.getByRole('table', { name: 'Corridas de backtest' })
    const scroller = table.parentElement!
    // jsdom no carga el CSS de Tailwind: la clase overflow-x-auto se replica en línea
    scroller.style.overflowX = 'auto'
    Object.defineProperty(scroller, 'scrollWidth', { configurable: true, value: 900 })
    Object.defineProperty(scroller, 'clientWidth', { configurable: true, value: 400 })
    swipe(within(table).getAllByRole('row')[1], 300)
    expect(heading()).toHaveTextContent('Performance')

    fireEvent.wheel(screen.getByRole('main'), { deltaX: 300, deltaY: 0, ctrlKey: true })
    expect(heading()).toHaveTextContent('Performance')

    const chart = document.createElement('div')
    chart.setAttribute('data-swipe-ignore', '')
    screen.getByRole('main').append(chart)
    swipe(chart, 300)
    expect(heading()).toHaveTextContent('Performance')
  })

  it('atajos: Ctrl+←/→ y ⌘+[ / ⌘+], pero no mientras se escribe', () => {
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: 'ArrowRight', ctrlKey: true })
    expect(heading()).toHaveTextContent('Terminal')
    fireEvent.keyDown(window, { key: '[', metaKey: true })
    expect(heading()).toHaveTextContent('Inicio')
    fireEvent.keyDown(window, { key: '[', metaKey: true })
    expect(heading()).toHaveTextContent('Performance')
    // sin pestaña anterior: no hace nada
    fireEvent.keyDown(window, { key: 'ArrowLeft', ctrlKey: true })
    expect(heading()).toHaveTextContent('Performance')

    fireEvent.keyDown(window, { key: ']', metaKey: true })
    expect(heading()).toHaveTextContent('Inicio')
    const input = document.createElement('input')
    document.body.append(input)
    fireEvent.keyDown(input, { key: 'ArrowRight', ctrlKey: true })
    expect(heading()).toHaveTextContent('Inicio')
    input.remove()
  })

  it('la vista nueva entra desde el lado hacia donde se fue', () => {
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: 'ArrowRight', ctrlKey: true })
    expect(document.querySelector('.view-enter')).toHaveAttribute('data-dir', '1')
    fireEvent.keyDown(window, { key: 'ArrowLeft', ctrlKey: true })
    expect(document.querySelector('.view-enter')).toHaveAttribute('data-dir', '-1')
  })
})

describe('terminal con 1, 2 o 4 gráficos (#286)', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    localStorage.clear()
  })

  const charts = () => screen.queryAllByRole('img', { name: /velas de/ })
  const layout = () => screen.getByRole('group', { name: 'Gráficos en pantalla' })

  it('arranca con uno y pasa a 4, cada uno con su instrumento y timeframe', async () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    await exampleChart()
    expect(charts()).toHaveLength(1)
    fireEvent.click(within(layout()).getByRole('button', { name: '4' }))
    expect(document.querySelector('[data-chart-layout="4"]')).not.toBeNull()
    for (let i = 1; i <= 4; i++) {
      const pick = screen.getByRole('group', { name: `Instrumento del gráfico ${i}` })
      expect(
        within(pick)
          .getAllByRole('button')
          .map((b) => b.textContent),
      ).toEqual(expect.arrayContaining(['DE40', 'US100', 'US500', 'US30']))
      expect(screen.getByRole('group', { name: `Timeframe del gráfico ${i}` })).toBeInTheDocument()
    }
    // sin gateway, el de US100 en 15 m usa los datos de ejemplo y los demás lo avisan
    expect(charts()).toHaveLength(1)
    expect(screen.getAllByRole('status').length).toBeGreaterThanOrEqual(3)

    // cambiar el gráfico 2 a US100 15 m también lo dibuja
    fireEvent.click(
      within(screen.getByRole('group', { name: 'Instrumento del gráfico 2' })).getByRole('button', {
        name: 'US100',
      }),
    )
    await waitFor(() => expect(charts()).toHaveLength(2))
  })

  it('la herramienta de medición (#281) anda en cada gráfico de la grilla, por separado', async () => {
    const us100 = { instrument: 'US100', timeframe: '15m' }
    localStorage.setItem(
      'cfdstrader.terminal.charts',
      JSON.stringify({ layout: '4', slots: [us100, us100, us100, us100] }),
    )
    // jsdom no tiene layout: cada SVG mide lo que dibuja (720 × 300 con 4 gráficos)
    vi.spyOn(SVGElement.prototype, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      x: 0,
      y: 0,
      width: 720,
      height: 300,
      right: 720,
      bottom: 300,
      toJSON: () => ({}),
    } as DOMRect)
    render(<TerminalView member={CURRENT_MEMBER} />)
    await exampleChart()
    const svgs = charts()
    expect(svgs).toHaveLength(4)
    const measureOf = (svg: Element) => svg.querySelector('[data-measure]')

    // los puntos se pegan al cierre de las velas (#355): entre estas dos velas el cierre sube,
    // así que de izquierda a derecha es subida y al revés es bajada
    svgs.forEach((svg, i) => {
      const [x0, x1] = i % 2 === 0 ? [200, 400] : [400, 200]
      fireEvent.pointerDown(svg, {
        clientX: x0,
        clientY: 150,
        button: 0,
        pointerId: i + 1,
        shiftKey: true,
      })
      fireEvent.pointerMove(svg, { clientX: x1, clientY: 150, pointerId: i + 1 })
      fireEvent.pointerUp(svg, { clientX: x1, clientY: 150, pointerId: i + 1 })
    })
    // cada gráfico tiene su propia medición: subida en 1 y 3, bajada en 2 y 4
    expect(svgs.map((s) => measureOf(s)?.getAttribute('data-measure'))).toEqual([
      'up',
      'down',
      'up',
      'down',
    ])
    expect(measureOf(svgs[0])!.querySelector('[data-measure-box]')).toHaveAttribute(
      'fill',
      'var(--chart-up)',
    )
    expect(measureOf(svgs[1])!.querySelector('[data-measure-box]')).toHaveAttribute(
      'fill',
      'var(--chart-down)',
    )
    expect(measureOf(svgs[0])!.textContent).toMatch(/ · \+\d+,\d{2} % · /)
    expect(measureOf(svgs[1])!.textContent).toMatch(/ · −\d+,\d{2} % · /)

    // un clic en el gráfico 2 borra solo la suya
    fireEvent.pointerDown(svgs[1], {
      clientX: 300,
      clientY: 150,
      button: 0,
      pointerId: 9,
      shiftKey: true,
    })
    fireEvent.pointerUp(svgs[1], { clientX: 300, clientY: 150, pointerId: 9 })
    expect(svgs.map((s) => measureOf(s) !== null)).toEqual([true, false, true, true])

    // Esc borra las que quedan
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(svgs.every((s) => measureOf(s) === null)).toBe(true)
  })

  it('recuerda la disposición en localStorage', () => {
    const { unmount } = render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(within(layout()).getByRole('button', { name: '2' }))
    unmount()
    render(<TerminalView member={CURRENT_MEMBER} />)
    expect(within(layout()).getByRole('button', { name: '2' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(document.querySelector('[data-chart-layout="2"]')).not.toBeNull()
  })

  it('si el almacenamiento falla o está roto, la grilla anda igual', async () => {
    localStorage.setItem('cfdstrader.terminal.charts', '{roto')
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    render(<TerminalView member={CURRENT_MEMBER} />)
    await exampleChart()
    expect(charts()).toHaveLength(1)
    fireEvent.click(within(layout()).getByRole('button', { name: '4' }))
    expect(document.querySelector('[data-chart-layout="4"]')).not.toBeNull()
  })
})

describe('contador suave (#286)', () => {
  it('muestra siempre el valor real al final y para los lectores de pantalla', () => {
    const { rerender, container } = render(<AnimatedNumber value="100.00" format={decimal} />)
    expect(container).toHaveTextContent(/^100,00$/)
    rerender(<AnimatedNumber value="250.50" format={decimal} />)
    expect(screen.getByText('250,50')).toBeInTheDocument()
  })

  it('con movimiento reducido cambia directo', () => {
    vi.spyOn(window, 'matchMedia').mockImplementation(
      (q: string) => ({ matches: q.includes('reduce') }) as MediaQueryList,
    )
    const { rerender, container } = render(<AnimatedNumber value="1.0" format={decimal} />)
    rerender(<AnimatedNumber value="2.0" format={decimal} />)
    expect(container).toHaveTextContent(/^2,0$/)
    vi.restoreAllMocks()
  })
})
