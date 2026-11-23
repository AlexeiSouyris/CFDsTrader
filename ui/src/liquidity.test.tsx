import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import App from '@/App'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { ImbalanceGauge, LevelsTable, Rate } from '@/components/ui/liquidity-panels'
import { decimal, percent, signed } from '@/lib/format'
import { levelName, STATE_LABEL } from '@/lib/liquidity-labels'
import { LIQUIDITY_US100 as DATA, type LiquiditySnapshot } from '@/mocks'
import { LiquidityView } from '@/views/liquidity'

describe('vista de liquidez en vivo (#231)', () => {
  it('se abre desde la navegación', () => {
    render(<App initialStage="app" />)
    const nav = screen.getByRole('navigation', { name: 'Vistas' })
    fireEvent.click(within(nav).getByText('Liquidez'))
    expect(screen.getByRole('heading', { level: 1, name: 'Liquidez' })).toBeInTheDocument()
    expect(location.hash).toBe('#liquidez')
    expect(screen.getByRole('img', { name: /US100, \d+ velas de 15 minutos/ })).toBeInTheDocument()
  })

  it('muestra cada nivel con el estado y el precio que manda el motor', () => {
    render(<LiquidityView />)
    const table = screen.getByRole('table', { name: 'Niveles de liquidez y su estado' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(DATA.levels.length)
    DATA.levels.forEach((l, i) => {
      expect(rows[i]).toHaveTextContent(levelName(l))
      expect(rows[i]).toHaveTextContent(decimal(l.price))
      expect(rows[i]).toHaveTextContent(STATE_LABEL[l.state])
    })
    // el ejemplo cubre todos los tipos y los tres estados
    const kinds = new Set(DATA.levels.map((l) => l.kind))
    for (const k of ['eqh', 'eql', 'pdh', 'pdl', 'pwh', 'pwl', 'session_high', 'session_low'])
      expect(kinds.has(k as never)).toBe(true)
    expect(new Set(DATA.levels.map((l) => l.state))).toEqual(new Set(['intact', 'swept', 'broken']))
  })

  it('separa los FVG abiertos de los llenos, tal como vienen', () => {
    render(<LiquidityView />)
    const open = DATA.fvgs.filter((f) => f.status === 'open').length
    const filled = DATA.fvgs.filter((f) => f.status !== 'open').length
    expect(open).toBeGreaterThan(0)
    expect(filled).toBeGreaterThan(0)
    expect(screen.getByRole('heading', { name: `FVG abiertos (${open})` })).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: `FVG llenos o vencidos (${filled})` }),
    ).toBeInTheDocument()
  })

  it('las tasas muestran el IC 95 % y n, o "datos insuficientes"', () => {
    const { rerender } = render(
      <Rate
        minCases={30}
        stat={{
          metric: 'touch',
          successes: 36,
          n: 58,
          insufficient: false,
          rate: '0.6207',
          ci_low: '0.4921',
          ci_high: '0.7344',
        }}
      />,
    )
    expect(screen.getByText('62,0 %')).toBeInTheDocument()
    expect(screen.getByText(/IC 95 %: 49,2–73,4 · n = 58/)).toBeInTheDocument()
    rerender(
      <Rate
        minCases={30}
        stat={{
          metric: 'fill',
          successes: 5,
          n: 12,
          insufficient: true,
          rate: null,
          ci_low: null,
          ci_high: null,
        }}
      />,
    )
    expect(screen.getByText('Datos insuficientes')).toBeInTheDocument()
    expect(screen.getByText('n = 12 de 30')).toBeInTheDocument()
    expect(screen.queryByText(/%/)).not.toBeInTheDocument()
  })

  it('el panel de estadísticas usa los números del motor, con y sin casos suficientes', () => {
    render(<LiquidityView />)
    const ok = DATA.stats.flatMap((s) => s.rates).find((r) => !r.insufficient && r.rate)!
    expect(screen.getAllByText(`${percent(ok.rate!)} %`).length).toBeGreaterThan(0)
    expect(screen.getAllByText('Datos insuficientes').length).toBeGreaterThan(0)
    expect(screen.getByText(/Gaps/)).toBeInTheDocument()
  })

  it('el desequilibrio muestra el último valor con el rótulo del contrato', () => {
    const current = DATA.imbalance.at(-1)!
    render(<ImbalanceGauge imbalance={current} />)
    expect(screen.getByText('aproximación desde el precio')).toBeInTheDocument()
    expect(screen.getByText(signed(current.index))).toBeInTheDocument()
    expect(screen.getByRole('meter', { name: /aproximación desde el precio/ })).toHaveAttribute(
      'aria-valuetext',
      signed(current.index),
    )
    expect(screen.getByText('no es flujo de órdenes')).toBeInTheDocument()
  })

  it('el ejemplo trae un nivel vencido y la tabla lo muestra atenuado', () => {
    const expired = DATA.levels.filter((l) => l.expired_at)
    expect(expired).toHaveLength(1)
    render(<LiquidityView />)
    const table = screen.getByRole('table', { name: 'Niveles de liquidez y su estado' })
    const rows = within(table)
      .getAllByRole('row')
      .filter((r) => r.hasAttribute('data-expired'))
    expect(rows).toHaveLength(1)
    expect(rows[0]).toHaveTextContent(decimal(expired[0].price))
  })

  it('un nivel vencido (expired_at) se muestra atenuado y fuera de los activos', () => {
    const base = DATA.levels.find((l) => l.state === 'intact')!
    const expired = { ...base, expired_at: DATA.as_of }
    render(<LevelsTable levels={[expired]} />)
    const row = screen.getByText(levelName(expired)).closest('tr')!
    expect(row).toHaveAttribute('data-expired', 'true')
    expect(row).toHaveTextContent('vencido')
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={DATA.candles}
        levels={[expired]}
        fvgs={[]}
        imbalance={[]}
      />,
    )
    expect(container.querySelector('[data-expired]')).not.toBeNull()
    fireEvent.click(within(container).getByRole('button', { name: 'Activos' }))
    expect(container.querySelector('[data-level-state]')).toBeNull()
  })

  it('no dibuja un nivel antes de que exista', () => {
    const level = DATA.levels.find((l) => l.kind === 'eqh')!
    const before = DATA.candles.filter((b) => b.start < level.exists_from)
    const data: LiquiditySnapshot = { ...DATA, candles: before.slice(0, -1) }
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={data.candles}
        levels={[level]}
        fvgs={[]}
        imbalance={[]}
      />,
    )
    expect(container.querySelectorAll('[data-level-state]')).toHaveLength(0)
  })

  it('el porcentaje corre la coma en el texto, sin float', () => {
    expect(percent('0.6207')).toBe('62,0')
    expect(percent('1')).toBe('100,0')
    expect(percent('0.0874', 2)).toBe('8,74')
  })

  it('ningún texto de la vista promete ganancias', () => {
    const { container } = render(<LiquidityView />)
    expect(container.textContent).not.toMatch(/ganancia (segura|garantizada)|sin riesgo|asegurad/i)
  })
})
