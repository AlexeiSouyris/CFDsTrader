import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { barAt, CursorSync } from '@/lib/cursor-sync'
import { chartBars, type ChartBar } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA } from '@/mocks'
import { MultichartView } from '@/views/multichart'

const H = Date.parse('2026-01-05T10:00:00Z')
const M15 = 15 * 60_000
const HOUR = 4 * M15
// tres velas de 15 m seguidas y una después de un hueco de una hora
const starts = [H, H + M15, H + 2 * M15, H + 4 * M15 + HOUR]
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

describe('cursor sincronizado: la vela de una hora (#544)', () => {
  it('la vela que contiene la hora, o ninguna', () => {
    expect(barAt(starts, ends, H)).toBe(0)
    expect(barAt(starts, ends, H + M15 - 1)).toBe(0)
    expect(barAt(starts, ends, H + M15 + 5 * 60_000)).toBe(1)
    // antes de la primera, en el hueco y después de la última: nada
    expect(barAt(starts, ends, H - 1)).toBeNull()
    expect(barAt(starts, ends, H + 3 * M15 + 1)).toBeNull()
    expect(barAt(starts, ends, ends[3])).toBeNull()
    expect(barAt([], [], H)).toBeNull()
    expect(barAt(starts, ends, NaN)).toBeNull()
  })

  it('el que saca el cursor solo borra si era suyo', () => {
    const hub = new CursorSync()
    const seen: unknown[] = []
    hub.subscribe(() => seen.push(hub.get()))
    hub.set('a', H)
    hub.set('a', H) // igual: no avisa de nuevo
    hub.set('b', H + M15)
    hub.set('a', null) // el de b sigue
    expect(hub.get()).toEqual({ from: 'b', t: H + M15 })
    hub.set('b', null)
    expect(hub.get()).toBeNull()
    expect(seen).toHaveLength(3)
  })
})

describe('cursor sincronizado entre gráficos (#544)', () => {
  beforeEach(() => {
    box(440)
  })
  afterEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  const bars15 = chartBars(DATA)
  // velas de 1 h armadas con las de 15 m que empiezan en la hora (solo para ubicar el cursor)
  const bars1h: ChartBar[] = bars15
    .filter((b) => new Date(b.start).getUTCMinutes() === 0)
    .map((b) => ({ ...b, end: new Date(Date.parse(b.start) + HOUR).toISOString() }))

  const twoCharts = (hub: CursorSync | undefined) =>
    render(
      <>
        <div data-testid="m15">
          <LiquidityChart
            instrument="US100"
            bars={bars15}
            levels={[]}
            fvgs={[]}
            imbalance={[]}
            syncCursor={hub}
          />
        </div>
        <div data-testid="h1">
          <LiquidityChart
            instrument="US100"
            timeframe="1h"
            bars={bars1h}
            levels={[]}
            fvgs={[]}
            imbalance={[]}
            syncCursor={hub}
          />
        </div>
      </>,
    )
  const svgOf = (id: string) => within(screen.getByTestId(id)).getByRole('img')
  const lineIn = (id: string) => screen.getByTestId(id).querySelector('[data-sync-cursor]')

  it('al mover el cursor en uno, el otro marca la vela que contiene esa hora', () => {
    twoCharts(new CursorSync())
    expect(lineIn('h1')).toBeNull()
    fireEvent.pointerMove(svgOf('m15'), { clientX: 400, clientY: 200, pointerId: 1 })
    // la hora de la vela bajo el cursor (15 m) cae en una vela de 1 h del otro gráfico
    const hovered = screen
      .getByTestId('m15')
      .querySelector('[data-hover-dot]')!
      .getAttribute('data-hover-dot')!
    const line = lineIn('h1')!
    expect(line).not.toBeNull()
    const t = Date.parse(hovered)
    const hourStart = Date.parse(line.getAttribute('data-sync-cursor')!)
    expect(hourStart).toBeLessThanOrEqual(t)
    expect(t).toBeLessThan(hourStart + HOUR)
    // el propio gráfico no marca su cursor
    expect(lineIn('m15')).toBeNull()

    // al salir, se borra
    fireEvent.pointerLeave(svgOf('m15'))
    expect(lineIn('h1')).toBeNull()

    // y al revés: el de 1 h marca en el de 15 m la vela que empieza a esa hora
    fireEvent.pointerMove(svgOf('h1'), { clientX: 300, clientY: 200, pointerId: 2 })
    const back = lineIn('m15')!
    expect(back).not.toBeNull()
    expect(new Date(Date.parse(back.getAttribute('data-sync-cursor')!)).getUTCMinutes()).toBe(0)
  })

  it('sin la opción, cada gráfico tiene su cursor', () => {
    twoCharts(undefined)
    fireEvent.pointerMove(svgOf('m15'), { clientX: 400, clientY: 200, pointerId: 1 })
    expect(lineIn('h1')).toBeNull()
  })

  it('en el Multigráfico la opción está apagada por defecto y se recuerda', () => {
    const { unmount } = render(<MultichartView />)
    const btn = () => screen.getByRole('button', { name: 'Sincronizar cursor' })
    expect(btn()).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(btn())
    expect(btn()).toHaveAttribute('aria-pressed', 'true')
    expect(JSON.parse(localStorage.getItem('cfdstrader.multichart')!).sync).toBe(true)
    unmount()
    render(<MultichartView />)
    expect(btn()).toHaveAttribute('aria-pressed', 'true')
    // con un solo gráfico no hay nada que sincronizar
    fireEvent.click(
      within(screen.getByRole('group', { name: 'Distribución' })).getByRole('button', {
        name: '1',
      }),
    )
    expect(screen.queryByRole('button', { name: 'Sincronizar cursor' })).toBeNull()
  })
})
