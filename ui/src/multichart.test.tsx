import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { exampleChart } from '@/test/example-chart'
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

  it('se abre desde la paleta (vive dentro de Gráficos)', () => {
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: 'k', metaKey: true })
    fireEvent.click(screen.getByRole('option', { name: /^Multigráfico/ }))
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

  it('el panel activo se resalta y la barra de dibujo actúa solo sobre él', async () => {
    render(<MultichartView />)
    await exampleChart()
    expect(slots()[0]).toHaveAttribute('data-active', 'true')
    // marca sutil (#570): punto en el encabezado y aria-current, sin anillo alrededor del panel
    expect(slots()[0]).toHaveAttribute('aria-current', 'true')
    expect(slots()[0].className).not.toMatch(/\bring-/)

    // US100 15 m tiene datos de ejemplo: medir y línea se aplican al gráfico 1
    const bar = drawBar()
    fireEvent.click(within(bar).getByRole('button', { name: 'Medir' }))
    fireEvent.click(within(bar).getByRole('button', { name: 'Línea' }))
    const first = slots()[0] as HTMLElement
    expect(within(first).getByRole('button', { name: /regla|medir/i })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    // en modo compacto el tipo de gráfico no se repite en el panel: se ve la línea de cierre
    expect(within(first).queryByRole('group', { name: 'Tipo de gráfico' })).toBeNull()
    expect(first.querySelector('[data-line-area]')).not.toBeNull()

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

  it('con más de un gráfico, modo compacto: los controles del activo van a la barra de arriba (#545)', async () => {
    render(<MultichartView />)
    await exampleChart()
    const bar = () => screen.getByRole('toolbar', { name: 'Gráfico activo' })
    const first = () => slots()[0] as HTMLElement
    // el panel: solo el gráfico con su encabezado mínimo (US100 15 m trae datos de ejemplo)
    expect(first().querySelector('[data-chart-compact]')).not.toBeNull()
    expect(within(first()).queryByRole('group', { name: 'Niveles' })).toBeNull()
    expect(within(first()).queryByRole('group', { name: 'Rango' })).toBeNull()
    const heading = first().querySelector('[data-compact-heading]') as HTMLElement
    expect(heading).toHaveTextContent(/US100 · 15m/)
    expect(heading).toHaveTextContent(/Último cierre/)

    // los controles del activo, en la barra de arriba, una sola vez
    expect(within(bar()).getAllByRole('group', { name: 'Niveles' })).toHaveLength(1)
    expect(within(bar()).getAllByRole('group', { name: 'Rango' })).toHaveLength(1)
    expect(within(bar()).getAllByRole('button', { name: /Herramientas/ })).toHaveLength(1)
    // el filtro de niveles sigue andando sobre el gráfico activo
    const levels = () => within(bar()).getByRole('group', { name: 'Niveles' })
    fireEvent.click(within(levels()).getByRole('button', { name: 'Activos' }))
    expect(within(levels()).getByRole('button', { name: 'Activos' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )

    // otro activo sin velas: la barra no muestra controles de un gráfico que no está
    fireEvent.click(within(slots()[1] as HTMLElement).getByRole('button', { name: /Gráfico 2/ }))
    expect(within(bar()).queryByRole('group', { name: 'Niveles' })).toBeNull()
    // volver al 1: sus controles vuelven, con lo que tenían
    fireEvent.click(within(first()).getByRole('button', { name: /Gráfico 1/ }))
    expect(within(levels()).getByRole('button', { name: 'Activos' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  it('con un solo gráfico, la vista completa de siempre', async () => {
    render(<MultichartView />)
    await exampleChart()
    fireEvent.click(layoutBtn('1'))
    const only = slots()[0] as HTMLElement
    expect(only.querySelector('[data-chart-compact]')).toBeNull()
    expect(only.querySelector('[data-compact-heading]')).toBeNull()
    expect(within(only).getByRole('group', { name: 'Niveles' })).toBeInTheDocument()
    expect(within(only).getByRole('group', { name: 'Rango' })).toBeInTheDocument()
  })

  it('no tiene botones de órdenes ni promete ganancias', () => {
    render(<MultichartView />)
    expect(screen.queryByRole('button', { name: /comprar|vender|orden|enviar/i })).toBeNull()
    expect(document.body.textContent).not.toMatch(/ganancia segura|garantiza|sin riesgo/i)
  })
})
