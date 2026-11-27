import { act, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { CURRENT_MEMBER } from '@/mocks'
import { MultichartView } from '@/views/multichart'
import { TerminalView } from '@/views/terminal'

/** QA de Gráficos y Terminal después del rediseño (#617). */
describe('contraste, objetivos y pestañas (#617)', () => {
  it('la barra flotante atenuada no baja del 85 % (AA en su texto)', () => {
    // en escritorio: en el celular la barra no flota y no se atenúa
    const media = vi
      .spyOn(window, 'matchMedia')
      .mockImplementation(
        (q) =>
          ({ matches: true, media: q, addEventListener() {}, removeEventListener() {} }) as never,
      )
    vi.useFakeTimers()
    try {
      render(<MultichartView />)
      const bar = screen.getByRole('toolbar', { name: 'Gráfico activo' })
      act(() => vi.advanceTimersByTime(10_000))
      expect(bar).toHaveAttribute('data-idle', 'true')
      expect(bar.className).toContain('opacity-85')
      expect(bar.className).not.toContain('opacity-35')
    } finally {
      vi.useRealTimers()
      media.mockRestore()
    }
  })

  it('los botones de cada panel del Multigráfico miden al menos 24 px de alto', () => {
    render(<MultichartView />)
    const panelButton = screen.getByRole('button', { name: /Gráfico 1/ })
    expect(panelButton.className).toContain('min-h-6')
  })

  it('en el celular, la distribución y sincronizar cursor no se muestran', () => {
    render(<MultichartView />)
    const picker = screen
      .getByRole('group', { name: 'Distribución' })
      .closest('div[data-layout-picker]')
    expect(picker?.className).toContain('max-lg:hidden')
  })

  it('las pestañas de la Terminal se recorren con las flechas, Inicio y Fin', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const tab = (name: RegExp) => screen.getByRole('tab', { name })
    expect(tab(/Posiciones/)).toHaveAttribute('tabindex', '0')
    expect(tab(/Historial/)).toHaveAttribute('tabindex', '-1')

    fireEvent.keyDown(tab(/Posiciones/), { key: 'ArrowRight' })
    expect(tab(/Órdenes/)).toHaveAttribute('aria-selected', 'true')
    expect(tab(/Órdenes/)).toHaveFocus()

    fireEvent.keyDown(tab(/Órdenes/), { key: 'End' })
    expect(tab(/Propuestas/)).toHaveAttribute('aria-selected', 'true')

    fireEvent.keyDown(tab(/Propuestas/), { key: 'ArrowRight' })
    expect(tab(/Posiciones/)).toHaveAttribute('aria-selected', 'true')

    fireEvent.keyDown(tab(/Posiciones/), { key: 'ArrowLeft' })
    expect(tab(/Propuestas/)).toHaveAttribute('aria-selected', 'true')

    fireEvent.keyDown(tab(/Propuestas/), { key: 'Home' })
    expect(tab(/Posiciones/)).toHaveAttribute('aria-selected', 'true')
    expect(tab(/Posiciones/)).toHaveFocus()
  })
})
