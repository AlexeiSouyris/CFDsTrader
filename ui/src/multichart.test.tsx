import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import App from '@/App'
import { MultichartView } from '@/views/multichart'

const KEY = 'cfdstrader.multichart'
const layoutBtn = (name: string) =>
  within(screen.getByRole('group', { name: 'Distribución' })).getByRole('button', { name })
const slots = () => [...document.querySelectorAll('[data-chart-slot]')]
const hidden = () => slots().map((el) => el.classList.contains('max-lg:hidden'))
const drawBar = () => screen.getByRole('toolbar', { name: /Herramientas de dibujo/ })

describe('multigráfico (#540)', () => {
  afterEach(() => localStorage.clear())

  it('se abre desde la navegación', () => {
    render(<App initialStage="app" />)
    fireEvent.click(within(screen.getByRole('navigation', { name: 'Vistas' })).getByText('Multi'))
    expect(location.hash).toBe('#multigrafico')
    expect(drawBar()).toBeInTheDocument()
  })

  it('por defecto, dos gráficos lado a lado; con 4, una cuadrícula de 2 × 2', () => {
    render(<MultichartView />)
    const grid = () => document.querySelector('[data-chart-layout]')!
    expect(grid()).toHaveAttribute('data-chart-layout', '2')
    expect(grid()).toHaveClass('lg:grid-cols-2')
    expect(slots()).toHaveLength(2)

    fireEvent.click(layoutBtn('4'))
    expect(slots()).toHaveLength(4)
    expect(grid()).toHaveClass('lg:grid-cols-2')

    // apilados: solo si se elige
    fireEvent.click(layoutBtn('2 apilados'))
    expect(slots()).toHaveLength(2)
    expect(grid()).not.toHaveClass('lg:grid-cols-2')

    fireEvent.click(layoutBtn('1'))
    expect(slots()).toHaveLength(1)
  })

  it('recuerda la distribución, los instrumentos y el gráfico activo', () => {
    const { unmount } = render(<MultichartView />)
    fireEvent.click(layoutBtn('4'))
    fireEvent.click(within(slots()[2] as HTMLElement).getByRole('button', { name: /Gráfico 3/ }))
    fireEvent.change(screen.getByLabelText('Instrumento del gráfico activo'), {
      target: { value: 'DE40' },
    })
    const saved = JSON.parse(localStorage.getItem(KEY)!)
    expect(saved.layout).toBe('4')
    expect(saved.active).toBe(2)
    expect(saved.slots[2].instrument).toBe('DE40')
    unmount()

    render(<MultichartView />)
    expect(slots()).toHaveLength(4)
    expect(slots()[2]).toHaveAttribute('data-active', 'true')
    expect(slots()[2]).toHaveAccessibleName(/DE40/)
  })

  it('con lo guardado roto o sin poder leer, abre la de siempre', () => {
    localStorage.setItem(KEY, '{roto')
    render(<MultichartView />)
    expect(slots()).toHaveLength(2)
  })

  it('el panel activo se resalta y la barra de dibujo actúa solo sobre él', () => {
    render(<MultichartView />)
    expect(slots()[0]).toHaveAttribute('data-active', 'true')
    expect(slots()[0]).toHaveClass('ring-primary')

    // US100 15 m tiene datos de ejemplo: medir y línea se aplican al gráfico 1
    const bar = drawBar()
    fireEvent.click(within(bar).getByRole('button', { name: 'Medir' }))
    fireEvent.click(within(bar).getByRole('button', { name: 'Línea' }))
    const first = slots()[0] as HTMLElement
    expect(within(first).getByRole('button', { name: /regla|medir/i })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(
      within(within(first).getByRole('group', { name: 'Tipo de gráfico' })).getByRole('button', {
        name: 'Línea',
      }),
    ).toHaveAttribute('aria-pressed', 'true')

    // otro panel activo: la barra muestra sus herramientas (las de siempre)
    fireEvent.click(within(slots()[1] as HTMLElement).getByRole('button', { name: /Gráfico 2/ }))
    expect(slots()[1]).toHaveAttribute('data-active', 'true')
    expect(within(drawBar()).getByRole('button', { name: 'Navegar' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    expect(screen.getByRole('complementary', { name: 'Información del gráfico 2' })).toBeVisible()
  })

  it('en el celular se ve un gráfico por vez, elegido con un selector', () => {
    render(<MultichartView />)
    fireEvent.click(layoutBtn('4'))
    expect(hidden()).toEqual([false, true, true, true])
    const picker = screen.getByRole('group', { name: 'Gráfico a la vista' })
    fireEvent.click(within(picker).getByRole('button', { name: '3' }))
    expect(hidden()).toEqual([true, true, false, true])
  })

  it('no tiene botones de órdenes ni promete ganancias', () => {
    render(<MultichartView />)
    expect(screen.queryByRole('button', { name: /comprar|vender|orden|enviar/i })).toBeNull()
    expect(document.body.textContent).not.toMatch(/ganancia segura|garantiza|sin riesgo/i)
  })
})
