/**
 * Vista en vivo del laboratorio (#854): las curvas llegan hechas del motor; la UI las junta sin
 * repetir variante, con tope, y las pinta del color de cómo terminaron. El PBO sale del informe.
 */
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { LiveTrials, LuckBar } from '@/components/ui/lab-live'
import { MAX_LIVE_CURVES, mergeCurves } from '@/lib/lab-api'
import type { LabJobEvent, LabReport, TrialCurve } from '@/lib/lab-contracts'
import reportJson from '@/mocks/lab-report.json'

const REPORT = reportJson as unknown as LabReport

const curve = (variant: number, last: string): TrialCurve => ({
  variant,
  months: 2,
  equity: ['1', '1.05', last],
})

const running: LabJobEvent = {
  job_id: 'a'.repeat(64),
  seq: 5,
  state: 'running',
  stage: 'running_variants',
  percent: 40,
  variants_done: 2,
  variants_total: 20,
  report_ids: [],
  error: null,
  recorded_at: '2026-11-27T03:00:00Z',
}

describe('las curvas en vivo', () => {
  it('se juntan sin repetir variante y con tope', () => {
    const a = mergeCurves([], [curve(0, '1.1'), curve(1, '0.9')])
    expect(mergeCurves(a, [curve(0, '1.2')]).map((c) => [c.variant, c.equity[2]])).toEqual([
      [1, '0.9'],
      [0, '1.2'],
    ])
    const many = Array.from({ length: MAX_LIVE_CURVES + 50 }, (_, i) => curve(i, '1'))
    const capped = mergeCurves([], many)
    expect(capped).toHaveLength(MAX_LIVE_CURVES)
    expect(capped[0].variant).toBe(50)
  })

  it('cuentan grande las variantes y pintan cada curva por cómo terminó', () => {
    render(
      <LiveTrials event={running} curves={[curve(0, '1.2'), curve(1, '0.7'), curve(2, '1')]} />,
    )
    const live = screen.getByRole('region', { name: 'Variantes en vivo' })
    expect(live.querySelector('[data-live-count]')?.textContent).toMatch(/^2 de 20/)
    const result = (v: number) =>
      live.querySelector(`path[data-variant="${v}"]`)?.getAttribute('data-result')
    expect([result(0), result(1), result(2)]).toEqual(['up', 'down', 'flat'])
    expect(live.querySelector('path[data-variant="0"]')).toHaveAttribute(
      'stroke',
      'var(--chart-up)',
    )
    expect(within(live).getByRole('img').getAttribute('aria-label')).toMatch(/3 variantes/)
    expect(live.textContent).toMatch(/exploratorio, no vota/i)
    expect(live.textContent).toMatch(/no promesas/)
    expect(live.textContent).not.toMatch(/ganancia (asegurada|garantizada)|vas a ganar/i)
  })

  it('sin curvas todavía muestra el contador y el gráfico vacío', () => {
    render(<LiveTrials event={{ ...running, variants_done: 0 }} curves={[]} />)
    const live = screen.getByRole('region', { name: 'Variantes en vivo' })
    expect(live.querySelector('[data-live-count]')?.textContent).toMatch(/^0 de 20/)
    expect(live.querySelectorAll('path')).toHaveLength(0)
  })
})

describe('suerte o ventaja', () => {
  it('muestra el PBO del informe como barra, sin calcular nada nuevo', () => {
    const pbo = {
      variants: 6,
      partitions: 8,
      pbo: '0.25',
      combinations: 70,
      logits: { mean: '0', p5: '0', p50: '0', p95: '0' },
      oos_loss: '0.1',
    }
    render(<LuckBar reports={[REPORT, { ...REPORT, pbo }]} />)
    const luck = screen.getByRole('region', { name: 'Suerte o ventaja' })
    expect(within(luck).getByRole('meter')).toHaveAttribute('aria-valuenow', '25')
    expect(luck.textContent).toMatch(/Sobre 6 variantes/)
  })

  it('sin PBO dice por qué y no dibuja barra', () => {
    render(<LuckBar reports={[REPORT]} />)
    const luck = screen.getByRole('region', { name: 'Suerte o ventaja' })
    expect(within(luck).queryByRole('meter')).toBeNull()
    expect(luck.textContent).toMatch(/desde 4 variantes/)
  })
})
