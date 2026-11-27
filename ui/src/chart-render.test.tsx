import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { groupLabels } from '@/components/ui/chart-measure'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { chartBars } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA } from '@/mocks'

// `groupLabels` se llama una vez por cada dibujo del gráfico entero: contarla dice si mover el
// cursor vuelve a dibujar todo el SVG o solo la capa del punto (#377).
vi.mock('@/components/ui/chart-measure', async (importOriginal) => {
  const m = await importOriginal<typeof import('@/components/ui/chart-measure')>()
  return { ...m, groupLabels: vi.fn(m.groupLabels) }
})

describe('rendimiento del gráfico al mover el cursor (#377)', () => {
  afterEach(() => vi.restoreAllMocks())

  it('mover el mouse, arrastrar y soltar no vuelven a dibujar las velas ni los niveles', () => {
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
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={chartBars(DATA)}
        levels={DATA.levels}
        fvgs={DATA.fvgs}
        imbalance={DATA.imbalance}
      />,
    )
    const svg = screen.getByRole('img', { name: /US100, \d+ velas/ })
    const calls = vi.mocked(groupLabels).mock.calls.length
    expect(calls).toBeGreaterThan(0)
    const candle = container.querySelector('[data-candle]')

    for (let x = 5; x < 640; x += 7) fireEvent.pointerMove(svg, { clientX: x, clientY: 200 })
    fireEvent.pointerDown(svg, {
      clientX: 100,
      clientY: 200,
      button: 0,
      pointerId: 1,
      shiftKey: true,
    })
    for (let x = 100; x < 500; x += 9)
      fireEvent.pointerMove(svg, { clientX: x, clientY: 150, pointerId: 1 })
    fireEvent.pointerUp(svg, { clientX: 500, clientY: 150, pointerId: 1 })

    // la capa del punto y la medición sí se movieron...
    expect(container.querySelector('[data-hover-dot]')).not.toBeNull()
    expect(container.querySelector('[data-measure]')).not.toBeNull()
    // ...pero el gráfico no se volvió a dibujar
    expect(vi.mocked(groupLabels).mock.calls.length).toBe(calls)
    expect(container.querySelector('[data-candle]')).toBe(candle)
    // en cambio, cambiar el rango sí lo vuelve a dibujar (la cuenta mide lo que dice medir)
    fireEvent.click(screen.getByRole('button', { name: 'Todo' }))
    expect(vi.mocked(groupLabels).mock.calls.length).toBeGreaterThan(calls)
  })
})
