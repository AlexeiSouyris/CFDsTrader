import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { LearningReportView } from '@/components/ui/learning-report'
import type { LearningReport, PaperComparison } from '@/lib/learning-api'
import { LearningSection, PaperVsBacktestSection } from '@/views/phase8'

/** Documentos de ejemplo con la forma de `paper_comparison.v1` y `learning_report.v1`. */
const leg = (entry: string, exit: string, pnl: string) => ({
  entry_time: '2026-11-10T15:00:00Z',
  entry_price: entry,
  exit_time: '2026-11-10T18:00:00Z',
  exit_price: exit,
  size: '2',
  commission: '0',
  swap: '0',
  net_pnl: pnl,
})

const comparison = (instrument: string, difference = '-9'): PaperComparison => ({
  comparison_id: `paper-vs-backtest:${instrument}:1`,
  instrument_id: instrument,
  from: '2026-11-02T00:00:00Z',
  to: '2026-11-21T00:00:00Z',
  created_at: '2026-11-23T10:00:00Z',
  paper: { trades: 2, net_pnl: '36' },
  backtest: { trades: 2, net_pnl: '45' },
  difference,
  causes: {
    fill_price: '-1',
    size: '0',
    commission: '0',
    swap: '0',
    weekend_close: '0',
    risk_rejection: '0',
    missing_in_paper: '-20',
    extra_in_paper: '12',
  },
  matched: [
    {
      decision_at: '2026-11-10T15:00:00Z',
      side: 'long',
      order_id: 'runner:1',
      paper: leg('100.5', '112.5', '24'),
      backtest: leg('100', '112.5', '25'),
      difference: { entry_price: '0.5', exit_price: '0', size: '0', net_pnl: '-1' },
      weekend_close: false,
    },
  ],
  missing: [
    {
      decision_at: '2026-11-12T10:00:00Z',
      side: 'short',
      backtest_net_pnl: '20',
      paper: 'no_order',
    },
  ],
  extra: [
    {
      decision_at: '2026-11-14T10:00:00Z',
      side: 'long',
      order_id: 'runner:2',
      paper_net_pnl: '12',
      backtest: 'no_trade',
    },
  ],
  paper_rejections: { stop_too_close: 1 },
})

const REPORT: LearningReport = {
  report_id: 'learning:x',
  created_at: '2026-11-23T10:00:00Z',
  from: '2026-11-01T00:00:00Z',
  to: '2026-11-21T00:00:00Z',
  sources: ['paper'],
  proposals: 40,
  calibration: {
    rows: [
      {
        from: '0.5',
        to: '0.6',
        cases: 34,
        hits: 15,
        mean_shown: '0.55',
        frequency: '0.4412',
        low: '0.2884',
        high: '0.6063',
        insufficient: false,
      },
      {
        from: '0.6',
        to: '1',
        cases: 6,
        hits: 4,
        mean_shown: '0.64',
        frequency: '0.6667',
        low: '0.3',
        high: '0.9032',
        insufficient: true,
      },
    ],
    without_probability: 2,
    sample: [],
  },
  signals: [
    {
      signal: 'sweep',
      with: { cases: 12, hits: 6, hit_rate: '0.5', mean_r: '0.3' },
      without: { cases: 28, hits: 13, hit_rate: '0.4643', mean_r: '-0.1' },
      difference_r: '0.4',
      small_sample: true,
    },
    {
      signal: 'trend',
      with: { cases: 30, hits: 14, hit_rate: '0.4667', mean_r: '0.1' },
      without: { cases: 10, hits: 5, hit_rate: '0.5', mean_r: '0' },
      difference_r: '0.1',
      small_sample: false,
    },
  ],
  change_proposals: [
    {
      change_id: 'learning:x:recalibrate:0',
      kind: 'recalibrate',
      text: 'Recalibrar el rango 50–60 %: se mostró 55 % y la frecuencia real fue 44 %.',
      evidence: 'calibration.rows[0]',
      status: 'pendiente_de_voto',
    },
  ],
}

function gateway(comparisons: PaperComparison[] | null, report: LearningReport | null) {
  const fetch = vi.fn(async (input: string | URL | Request) => {
    const url = String(input)
    const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
    if (url === '/api/v1/paper_comparisons' && comparisons) return json(comparisons)
    if (url === '/api/v1/learning_report' && comparisons) return json(report)
    return new Response('', { status: 502 })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

describe('paper vs backtest (fase 8, #331)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('muestra emparejadas, faltantes y sobrantes y la diferencia por causa tal como llegan', async () => {
    gateway([comparison('US100.FUT'), comparison('US500.FUT', '-3')], null)
    render(<PaperVsBacktestSection />)
    const causes = await screen.findByRole('list', { name: 'Diferencia de P/L por causa' })
    // Cada causa con el monto del motor (la UI no las suma ni las recalcula).
    expect(causes.querySelector('[data-cause="missing_in_paper"]')).toHaveTextContent(
      'Faltan en paper−20',
    )
    expect(causes.querySelector('[data-cause="extra_in_paper"]')).toHaveTextContent('+12')
    expect(causes.querySelector('[data-cause="size"]')).toHaveTextContent('+0')
    // La barra más larga es la causa más grande, del lado negativo.
    const bar = (k: string) =>
      causes.querySelector<HTMLElement>(`[data-cause="${k}"] [style]`)!.style.width
    expect(bar('missing_in_paper')).toBe('50%')
    expect(bar('size')).toBe('0%')

    const matched = screen.getByRole('table', { name: 'Operaciones emparejadas' })
    expect(within(matched).getAllByRole('row')).toHaveLength(2)
    expect(matched).toHaveTextContent('100,5 → 112,5, +24')
    expect(screen.getByRole('list', { name: 'Operaciones que faltan en paper' })).toHaveTextContent(
      'paper no mandó orden',
    )
    expect(screen.getByRole('list', { name: 'Operaciones que sobran en paper' })).toHaveTextContent(
      'el backtest no operó esa vela',
    )
    expect(screen.getByText(/stop_too_close ×1/)).toBeInTheDocument()

    // Otro instrumento.
    fireEvent.click(screen.getByRole('button', { name: 'US500.FUT' }))
    expect(
      document.querySelector('[data-comparison="paper-vs-backtest:US500.FUT:1"]'),
    ).not.toBeNull()
  })

  it('sin comparaciones o sin gateway lo dice, sin inventar datos', async () => {
    gateway([], null)
    const { unmount } = render(<PaperVsBacktestSection />)
    expect(await screen.findByText('Todavía no hay comparaciones guardadas.')).toBeInTheDocument()
    unmount()
    vi.unstubAllGlobals()
    gateway(null, null)
    render(<PaperVsBacktestSection />)
    expect(await screen.findByText(/Sin conexión con el gateway/)).toBeInTheDocument()
  })
})

describe('aprendizaje (fase 8b, #331)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('tabla de confiabilidad con Wilson y datos insuficientes marcados', () => {
    render(<LearningReportView report={REPORT} />)
    const table = screen.getByRole('table', { name: 'Tabla de confiabilidad' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows[0]).toHaveTextContent('50–60 %')
    expect(rows[0]).toHaveTextContent('15/34')
    expect(rows[0]).toHaveTextContent('55,0 %')
    expect(rows[0]).toHaveTextContent('44,1 %')
    expect(rows[0]).toHaveTextContent('28,8 % – 60,6 %')
    expect(rows[0]).not.toHaveTextContent('datos insuficientes')
    expect(rows[1]).toHaveAttribute('data-insufficient', 'true')
    expect(rows[1]).toHaveTextContent('datos insuficientes')
    expect(screen.getByText(/2 sin probabilidad mostrada/)).toBeInTheDocument()
  })

  it('aporte de cada señal con la muestra chica marcada', () => {
    render(<LearningReportView report={REPORT} />)
    const table = screen.getByRole('table', { name: 'Aporte de cada señal' })
    const [sweep, trend] = within(table).getAllByRole('row').slice(1)
    expect(sweep).toHaveTextContent('Sweep')
    expect(sweep).toHaveTextContent('12 · 50,0 % · 0,30 R')
    expect(sweep).toHaveTextContent('28 · 46,4 % · −0,10 R')
    expect(sweep).toHaveTextContent('muestra chica')
    expect(trend).not.toHaveTextContent('muestra chica')
  })

  it('propuestas "pendiente de voto", sin ningún control para votar', () => {
    render(<LearningReportView report={REPORT} />)
    const list = screen.getByRole('list', { name: 'Propuestas de cambio' })
    expect(list).toHaveTextContent('Recalibrar')
    expect(list).toHaveTextContent('pendiente de voto')
    expect(list).toHaveTextContent('calibration.rows[0]')
    expect(screen.queryAllByRole('button')).toHaveLength(0)
  })

  it('cuando llegue el estado del voto (#330), se muestra en lugar de "pendiente"', () => {
    render(
      <LearningReportView
        report={REPORT}
        voteStatus={() => ({ label: 'aprobada', detail: '4 de 6 a favor' })}
      />,
    )
    const list = screen.getByRole('list', { name: 'Propuestas de cambio' })
    expect(list).toHaveTextContent('aprobada')
    expect(list).toHaveTextContent('4 de 6 a favor')
    expect(list).not.toHaveTextContent('pendiente de voto')
  })

  it('pide el informe al gateway; sin informe guardado lo dice', async () => {
    const fetch = gateway([], REPORT)
    const { unmount } = render(<LearningSection />)
    expect(await screen.findByRole('table', { name: 'Tabla de confiabilidad' })).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith('/api/v1/learning_report', expect.anything())
    unmount()
    vi.unstubAllGlobals()
    gateway([], null)
    render(<LearningSection />)
    expect(
      await screen.findByText('Todavía no hay un informe de aprendizaje guardado.'),
    ).toBeInTheDocument()
  })
})
