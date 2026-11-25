/**
 * #570: los popovers (menú "Herramientas", definiciones del glosario) van en una capa sobre los
 * gráficos, y el gráfico activo del multigráfico se marca sin anillo alrededor del panel.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { FLOATING_Z, placeFloating } from '@/lib/floating'
import { MultichartView } from '@/views/multichart'

const KEY = 'cfdstrader.multichart'
const slots = () => [...document.querySelectorAll<HTMLElement>('[data-chart-slot]')]
const bar = () => screen.getByRole('toolbar', { name: 'Gráfico activo' })
const openTools = () => {
  fireEvent.click(within(bar()).getByRole('button', { name: /Herramientas/ }))
  return screen.getByRole('dialog', { name: 'Herramientas de análisis' })
}
const rect = (left: number, top: number, w: number, h: number) =>
  ({ left, top, right: left + w, bottom: top + h, width: w, height: h }) as DOMRect

describe('capa de popovers (#570)', () => {
  afterEach(() => localStorage.clear())

  for (const layout of ['1', '2', '2v', '4'] as const) {
    it(`el menú Herramientas se dibuja en la capa superior, fuera de las tarjetas (distribución ${layout})`, () => {
      localStorage.setItem(KEY, JSON.stringify({ layout, active: 0 }))
      render(<MultichartView />)
      const menu = openTools()
      // en un portal a body: ninguna tarjeta `.glass` (con su contexto de apilamiento) lo contiene
      expect(menu.parentElement).toBe(document.body)
      expect(menu.closest('.glass')).toBeNull()
      expect(menu.style.position).toBe('fixed')
      expect(Number(menu.style.zIndex)).toBe(FLOATING_Z)
      for (const s of slots()) expect(s.contains(menu)).toBe(false)
    })
  }

  it('se cierra con un clic afuera y con Esc (el foco vuelve al botón)', () => {
    render(<MultichartView />)
    openTools()
    fireEvent.pointerDown(slots()[0])
    expect(screen.queryByRole('dialog', { name: 'Herramientas de análisis' })).toBeNull()

    const menu = openTools()
    // un clic adentro no lo cierra
    fireEvent.pointerDown(within(menu).getAllByRole('checkbox')[0])
    expect(screen.getByRole('dialog', { name: 'Herramientas de análisis' })).toBeInTheDocument()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('dialog', { name: 'Herramientas de análisis' })).toBeNull()
    expect(within(bar()).getByRole('button', { name: /Herramientas/ })).toHaveFocus()
  })

  it('la definición de un término también va en la capa, y tocarla no cierra el menú', () => {
    render(<MultichartView />)
    const menu = openTools()
    fireEvent.click(within(menu).getByRole('button', { name: 'Precio de fondo' }))
    const def = screen.getByRole('dialog', { name: 'Precio de fondo' })
    expect(def.parentElement).toBe(document.body)
    expect(def.style.position).toBe('fixed')
    fireEvent.pointerDown(def)
    expect(screen.getByRole('dialog', { name: 'Herramientas de análisis' })).toBeInTheDocument()
  })
})

describe('posición de la capa (#570)', () => {
  const view = { width: 1000, height: 800 }
  it('abre debajo del botón, alineada a su borde derecho, sin salirse de la ventana', () => {
    const p = placeFloating(rect(700, 100, 120, 32), { width: 288, height: 300 }, view, 'end')
    expect(p.top).toBe(140)
    expect(p.left).toBe(820 - 288)
    // pegado al borde izquierdo: se corre adentro
    const q = placeFloating(rect(0, 100, 60, 32), { width: 288, height: 300 }, view, 'end')
    expect(q.left).toBeGreaterThanOrEqual(8)
  })
  it('si abajo no entra y arriba hay más lugar, abre hacia arriba', () => {
    const p = placeFloating(rect(100, 700, 120, 32), { width: 288, height: 300 }, view, 'start')
    expect(p.top + 300).toBeLessThanOrEqual(700)
    expect(p.left).toBe(100)
  })
  it('en el celular (375 px) el ancho se ajusta a la ventana', () => {
    const p = placeFloating(
      rect(300, 100, 60, 32),
      { width: 288, height: 200 },
      { width: 375, height: 700 },
      'end',
    )
    expect(p.left).toBeGreaterThanOrEqual(8)
    expect(p.left + 288).toBeLessThanOrEqual(375 - 8)
  })
})

describe('gráfico activo sin anillo (#570)', () => {
  afterEach(() => localStorage.clear())
  it('el activo lleva un punto en el encabezado y aria-current; ningún panel tiene anillo', () => {
    localStorage.setItem(KEY, JSON.stringify({ layout: '4', active: 1 }))
    render(<MultichartView />)
    const all = slots()
    expect(all).toHaveLength(4)
    for (const s of all) expect(s.className).not.toMatch(/\bring-/)
    expect(all[1]).toHaveAttribute('aria-current', 'true')
    expect(all.filter((s) => s.hasAttribute('aria-current'))).toHaveLength(1)
    expect(all[1].querySelector('[data-active-dot]')?.className).toMatch(/chart-accent/)
    expect(all[0].querySelector('[data-active-dot]')?.className).not.toMatch(/chart-accent/)
    expect(within(all[1]).getByRole('button', { name: /Gráfico 2/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })
})
