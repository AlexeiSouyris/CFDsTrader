import { render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { StaleNote } from '@/components/ui/data-honesty'
import { dateTimeUtc, dateUtc, STALE_AFTER_DAYS, staleDays } from '@/lib/format'
import { ChartsView, CompetitionView, JournalView, NewsView, OverviewView } from '@/views'
import { MultichartView } from '@/views/multichart'

/** Un gateway falso de prueba (no son datos reales): catálogo, fondo y vistas sin velas. */
function gateway() {
  const fetch = vi.fn(async (input: string | URL | Request) => {
    const url = String(input)
    const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
    if (url === '/api/v1/instruments')
      return json({
        default_instrument: 'US100',
        default_timeframe: '15m',
        timeframes: ['15m', '1h', '1d'],
        instruments: [
          {
            id: 'US100',
            name: 'EEUU Tech 100',
            asset_class: 'index',
            currency: 'EUR',
            exchange_timezone: 'America/New_York',
          },
        ],
      })
    if (url === '/api/v1/fund')
      return json({
        currency: 'EUR',
        basis: 'last_settlement',
        valued_at: null,
        nav: '1',
        total_value: '0',
        total_units: '0',
        members: [],
      })
    if (url === '/api/v1/fund/movements') return json([])
    if (url.startsWith('/api/v1/liquidity?')) {
      const q = new URLSearchParams(url.split('?')[1])
      return json({
        instrument_id: q.get('instrument'),
        timeframe: q.get('timeframe'),
        generated_at: '2026-11-24T10:00:00Z',
        as_of: null,
        candles: [],
        levels: [],
        fvgs: [],
        imbalance: [],
        stats: [],
      })
    }
    return new Response('', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
}

describe('honestidad de los datos (#577)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('con el gateway sin velas, el gráfico queda vacío: no muestra el ejemplo', async () => {
    gateway()
    render(<MultichartView />)
    await waitFor(() =>
      expect(screen.getAllByText(/El motor todavía no tiene velas de US100 en 15m/).length).toBe(1),
    )
    expect(document.querySelector('[data-example]')).toBeNull()
    expect(document.querySelector('svg[role="img"]')).toBeNull()
  })

  it('el panel marca cada tarjeta de ejemplo; el valor de la unidad y los nombres, del motor', async () => {
    gateway()
    render(<OverviewView onOpen={() => {}} />)
    const nav = (await screen.findByText(/del motor, sin liquidaciones/)).closest('section')!
    expect(nav).toHaveTextContent('1,0000')
    expect(within(nav).queryByText('EJEMPLO')).toBeNull()
    // P/L, drawdown y propuestas, índices, mapa, operaciones y competencia: de ejemplo
    expect(document.querySelectorAll('section[data-example]')).toHaveLength(7)
    for (const card of document.querySelectorAll('section[data-example]'))
      expect(card.querySelector('[data-example-badge]')).not.toBeNull()
    // un solo nombre por instrumento: el del catálogo
    const indices = screen.getByRole('heading', { name: 'Índices' }).closest('section')!
    await waitFor(() => expect(indices).toHaveTextContent('EEUU Tech 100'))
    expect(indices).not.toHaveTextContent('US Tech 100 ')
  })

  it('Gráficos, Diario, Trofeos y Noticias avisan que son de ejemplo; sin libro de órdenes', () => {
    for (const View of [ChartsView, JournalView, CompetitionView, NewsView]) {
      const { unmount } = render(<View />)
      const note = screen.getByRole('note')
      expect(note).toHaveTextContent('EJEMPLO')
      unmount()
    }
    render(<ChartsView />)
    expect(screen.queryByText(/profundidad|libro de órdenes/i)).toBeNull()
  })

  it('las fechas de otro año muestran el año; las de este año, no', () => {
    const now = Date.parse('2026-11-25T12:00:00Z')
    expect(dateTimeUtc('2024-11-21T23:00:00Z', now)).toMatch(/21 nov 2024.*23:00 UTC/)
    expect(dateUtc('2024-11-21T23:00:00Z', now)).toMatch(/21 nov 2024/)
    expect(dateTimeUtc('2026-11-21T23:00:00Z', now)).not.toMatch(/2026/)
  })

  it('avisa "datos de hace N días" solo pasado el umbral', () => {
    const now = Date.parse('2026-11-25T12:00:00Z')
    expect(staleDays('2026-11-24T12:00:00Z', now)).toBeNull()
    expect(staleDays(new Date(now - STALE_AFTER_DAYS * 86_400_000).toISOString(), now)).toBe(
      STALE_AFTER_DAYS,
    )
    expect(staleDays('2024-11-21T23:00:00Z', now)).toBeGreaterThan(365)
    expect(staleDays(null, now)).toBeNull()
    const { container } = render(<StaleNote at="2020-01-01T00:00:00Z" />)
    expect(container).toHaveTextContent(/datos de hace \d+ días/)
  })
})
