/**
 * Gráficos y textos (#580): un test por cada arreglo de la revisión de QA del 25-11.
 */
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { ChartNavBar } from '@/components/ui/chart-nav-bar'
import { groupLabels, LABEL_GAP, MAX_AXIS_LABELS } from '@/components/ui/chart-measure'
import { useViewport } from '@/components/ui/chart-nav'
import { ChartToolsMenu } from '@/components/ui/chart-tools'
import { LiquidityChart } from '@/components/ui/liquidity-chart'
import { OpsEventFeed } from '@/components/ui/ops-status'
import { DEFAULT_TOOLS } from '@/lib/analysis-layers'
import { closedAt, price, priceDigits, signed } from '@/lib/format'
import { liveLabel } from '@/lib/ibkr-live'
import { chartBars } from '@/lib/liquidity-bars'
import { LIQUIDITY_US100 as DATA } from '@/mocks'
import { LiquidityContent } from '@/views/liquidity'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
  history.replaceState(null, '', '/')
})

const item = (key: string, y: number, rank = 0) => ({ key, text: key, title: key, y, rank })

describe('rótulos del eje de precio', () => {
  it('agrupa los que se pisan con "+N", encabeza el más importante y nunca pasa del máximo', () => {
    const out = groupLabels(
      [item('Redondo', 100, 2), item('PDH', 104), item('NY máx', 300), item('PWH', 306)],
      6,
      400,
    )
    expect(out.map((l) => l.text)).toEqual(['PDH +1', 'NY máx +1'])
    expect(out[0].title).toBe('Redondo\nPDH')
    expect(out[0].count).toBe(2)
    const many = Array.from({ length: 40 }, (_, k) => item(`n${k}`, 10 + k * (LABEL_GAP + 1)))
    const capped = groupLabels(many, 6, 600)
    expect(capped.length).toBeLessThanOrEqual(MAX_AXIS_LABELS)
    expect(capped.reduce((s, l) => s + l.count, 0)).toBe(40)
  })

  it('de los números redondos solo van al eje los dos más cercanos al último cierre', () => {
    const bars = chartBars(DATA)
    const base = DATA.levels.find((l) => l.state === 'intact' && !l.expired_at)!
    const close = Number(bars.at(-1)!.close)
    const rounds = [-3, -2, -1, 1, 2, 3].map((k) => ({
      ...base,
      kind: k > 0 ? ('round_high' as const) : ('round_low' as const),
      session: null,
      price: String(Math.round(close / 100) * 100 + k * 100),
      exists_from: bars[0].start,
    }))
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={bars}
        levels={rounds}
        fvgs={[]}
        imbalance={[]}
        height={440}
      />,
    )
    const labels = [...container.querySelectorAll('[data-level-label]')]
    const total = labels.reduce((s, g) => s + Number(g.getAttribute('data-label-count') ?? 1), 0)
    expect(total).toBe(2)
    expect(container.textContent).not.toMatch(/red↑|red↓/)
    expect(labels[0].querySelector('text')!.textContent).toMatch(/Redondo$/)
  })
})

describe('herramientas sin capas', () => {
  it('el botón se ve deshabilitado, sin contador, y el menú explica por qué', () => {
    render(<ChartToolsMenu tools={DEFAULT_TOOLS} onToggle={() => {}} available={false} />)
    const button = screen.getByRole('button', { name: /Herramientas/ })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    expect(button.textContent).toBe('Herramientas')
    fireEvent.click(button)
    const menu = screen.getByRole('dialog', { name: 'Herramientas de análisis' })
    expect(within(menu).getByRole('note')).toHaveTextContent('no trae herramientas')
    for (const box of within(menu).getAllByRole('checkbox')) {
      expect(box).toBeDisabled()
      expect(box).not.toBeChecked()
    }
  })
})

function NavBar({ history }: { history: 'idle' | 'done' }) {
  const vp = useViewport({
    starts: ['2026-11-20T10:00:00Z'],
    lows: [1],
    highs: [2],
    initial: 10,
    plotW: 600,
    top: 6,
    h: 400,
  })
  return (
    <ChartNavBar
      vp={vp}
      ruler={false}
      onRuler={() => {}}
      right={70}
      bottom={406}
      history={history}
    />
  )
}

describe('avisos sobre el gráfico', () => {
  it('"No hay velas más antiguas" se va solo', () => {
    vi.useFakeTimers()
    render(<NavBar history="done" />)
    expect(screen.getByText('No hay velas más antiguas guardadas')).toBeInTheDocument()
    act(() => vi.advanceTimersByTime(3100))
    expect(screen.queryByText('No hay velas más antiguas guardadas')).toBeNull()
  })

  it('"En formación" y "Borrar dibujo" van uno debajo del otro, no encimados', () => {
    const bars = chartBars(DATA)
    const { container } = render(
      <LiquidityChart
        instrument="US100"
        bars={bars}
        forming={{ start: bars.at(-1)!.start, label: 'IBKR · ~10 min de atraso', partial: false }}
        levels={[]}
        fvgs={[]}
        imbalance={[]}
        drawTool="none"
      />,
    )
    const label = container.querySelector('[data-forming-label]')!
    expect(label).toHaveTextContent('En formación · IBKR · ~10 min de atraso')
    expect(label.textContent).not.toMatch(/delayed/)
    // la etiqueta va en la misma columna que el botón de borrar (que aparece al elegir un dibujo)
    expect(label.parentElement!.className).toMatch(/flex-col/)
  })
})

describe('formatos', () => {
  it('un precio siempre con los decimales del instrumento', () => {
    const digits = priceDigits(['26006', '26007.75', '26010.5'])
    expect(digits).toBe(2)
    expect(price('26006', digits)).toBe('26.006,00')
    expect(price('26007.75', digits)).toBe('26.007,75')
  })

  it('el cero va sin signo y la vela cerrada dice su hora de cierre', () => {
    expect(signed('0')).toBe('0')
    expect(signed('-0.00')).toBe('0,00')
    expect(signed('1.5')).toBe('+1,5')
    expect(closedAt('2026-11-25T04:00:00Z')).toBe('cerró el 25 nov, 04:00 UTC')
  })

  it('el atraso de IBKR va en español', () => {
    expect(liveLabel({ data_type: 'delayed', delay_seconds: 600 })).toBe('IBKR · ~10 min de atraso')
    expect(liveLabel({ data_type: 'frozen', delay_seconds: null })).toBe('IBKR · congelado')
  })
})

describe('textos para el usuario', () => {
  it('un instrumento omitido se explica en palabras y la nota técnica queda plegada', () => {
    render(
      <OpsEventFeed
        events={[
          {
            ts: '2026-11-25T04:00:00Z',
            level: 'warn',
            target: 'gateway',
            event: 'instrument_skipped',
            fields: {
              detail:
                'XAUUSD: value_per_point pendiente: Confirmar con GET /markets/{epic} (valueOfOnePip) (ADR 0013 §3, #67)',
            },
          },
        ]}
      />,
    )
    expect(
      screen.getByText(/Se omite XAUUSD en el broker simulado: faltan datos del instrumento\./),
    ).toBeVisible()
    expect(screen.getByText('Instrumento omitido')).toBeInTheDocument()
    expect(screen.getByText('Detalle técnico').closest('details')).not.toHaveAttribute('open')
  })

  it('futuros: sin doble paréntesis cuando no hay velas', () => {
    const { container } = render(
      <LiquidityContent
        data={{
          ...DATA,
          timeframe: '1d',
          as_of: null,
          candles: [],
          source: { id: 'futures', label: 'futuros CME/Eurex (base distinta al CFD)' },
        }}
      />,
    )
    expect(container.textContent).not.toContain('))')
    expect(container.textContent).toContain('Fuente: futuros CME/Eurex (base distinta al CFD).')
  })

  it('el título de la pestaña cambia con la vista', () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('null', { status: 503 })),
    )
    history.replaceState(null, '', '#glosario')
    render(<App initialStage="app" />)
    expect(document.title).toBe('Glosario · CFDsTrader')
  })
})
