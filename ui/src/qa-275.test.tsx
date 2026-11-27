import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Watchlist } from '@/components/ui/watchlist'
import { DEFAULT_SLOTS, preferIbkr } from '@/lib/chart-slots'
import { leverageText } from '@/lib/format'
import { CURRENT_MEMBER } from '@/mocks'
import { IBKR_WATCHLIST, ibkrBars } from '@/mocks/ibkr'
import { MultichartView } from '@/views/multichart'
import { TerminalView } from '@/views/terminal'

/** Datos de prueba con la forma de los contratos (no son noticias ni precios reales). */
const NEWS = {
  as_of: '2026-11-25T11:00:00Z',
  generated_at: '2026-11-25T11:00:00Z',
  feed: null,
  next_before: null,
  items: [
    {
      item: {
        source: 'ecb',
        feed: 'press',
        item_id: 'n1',
        title: 'Titular de prueba del BCE',
        url: 'https://example.test/n1',
        published_at: '2026-11-25T10:00:00Z',
        retrieved_at: '2026-11-25T10:15:00Z',
      },
      interpretations: [],
    },
  ],
}
const event = (name: string, at: string, impact: 'low' | 'medium' | 'high') => ({
  source: 'forex_factory',
  event: name,
  country: 'US',
  instrument_id: null,
  impact,
  scheduled_at: at,
  published_at: '2026-11-20T00:00:00Z',
})
const CALENDAR = {
  from: '2026-11-24T11:00:00Z',
  to: '2026-12-09T11:00:00Z',
  as_of: '2026-11-25T11:00:00Z',
  generated_at: '2026-11-25T11:00:00Z',
  economic_events: [
    event('Evento que ya pasó', '2026-11-25T09:00:00Z', 'high'),
    event('Evento más lejano', '2026-11-27T13:30:00Z', 'medium'),
    event('Evento más cercano', '2026-11-25T13:30:00Z', 'high'),
  ],
  official_releases: [],
}

/** Un gateway falso de solo prueba (solo lectura): nada sale al broker simulado real. */
function gateway(opts: { newsDown?: boolean } = {}) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const url = String(input)
      const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
      if (url === '/api/v1/ibkr/watchlist') return json(IBKR_WATCHLIST)
      if (url.startsWith('/api/v1/ibkr/bars?')) {
        const q = new URLSearchParams(url.split('?')[1])
        return json(
          ibkrBars(q.get('timeframe') as '1m' | '15m' | '1h', { instrument: q.get('instrument')! }),
        )
      }
      if (url === '/api/v1/news')
        return opts.newsDown ? new Response('', { status: 502 }) : json(NEWS)
      if (url === '/api/v1/calendar')
        return opts.newsDown ? new Response('', { status: 502 }) : json(CALENDAR)
      return new Response('', { status: 502 })
    }),
  )
}

afterEach(() => {
  vi.unstubAllGlobals()
  localStorage.clear()
})

describe('Terminal con datos reales (#275)', () => {
  it('los lugares sin tocar pasan a los de IBKR; los que alguien eligió, no', () => {
    expect(preferIbkr(DEFAULT_SLOTS, ['MNQ', 'MES', 'QQQ'])).toEqual([
      { instrument: 'MNQ', timeframe: '15m', feed: 'ibkr' },
      { instrument: 'MES', timeframe: '15m', feed: 'ibkr' },
      { instrument: 'QQQ', timeframe: '15m', feed: 'ibkr' },
      { instrument: 'MNQ', timeframe: '1h', feed: 'ibkr' },
    ])
    const touched = [{ instrument: 'DE40', timeframe: '15m' as const }, ...DEFAULT_SLOTS.slice(1)]
    expect(preferIbkr(touched, ['MNQ'])).toBeNull()
    expect(preferIbkr(DEFAULT_SLOTS, [])).toBeNull()
  })

  it('con IBKR en vivo, la Terminal abre MNQ de IBKR en vez de un gráfico vacío', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    expect(
      await screen.findByText(/IBKR · MNQZ6 · última vela hace ~14 min · solo datos/),
    ).toBeVisible()
  })

  it('el Multigráfico también arranca con los de IBKR', async () => {
    gateway()
    render(<MultichartView />)
    expect(await screen.findByRole('region', { name: 'Gráfico 1: MNQ 15m' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: 'Gráfico 2: MES 15m' })).toBeInTheDocument()
  })

  it('la lista de mercados: hora en 24 h, precios con 2 decimales y el atraso dicho como es', () => {
    render(
      <Watchlist
        groups={[
          {
            id: 'ibkr',
            title: 'IBKR',
            sourceLabel: 'solo datos',
            feed: 'ibkr',
            badge: 'IBKR',
            rows: [
              {
                instrument_id: 'MNQ',
                last: '25590',
                at: `${new Date().toISOString().slice(0, 10)}T15:13:00Z`,
                change_pct: '-2.5',
                delay_s: 900,
              },
            ],
          },
        ]}
        names={{}}
        selected=""
        onSelect={() => {}}
      />,
    )
    const row = screen.getByRole('button', { name: /MNQ/ })
    expect(row).toHaveTextContent('25.590,00')
    expect(row).toHaveTextContent('−2,50 %')
    expect(row).toHaveTextContent('contra el cierre de la sesión anterior')
    expect(row).toHaveTextContent('15:13 UTC')
    expect(row).not.toHaveTextContent(/[ap]\.\s?m\./)
    expect(within(row).getByText('IBKR · cerró hace ~15 min')).toBeInTheDocument()
  })

  it('el apalancamiento sin posiciones es "—", no "0:1"', () => {
    expect(leverageText('0')).toBe('—')
    expect(leverageText('0.00')).toBe('—')
    expect(leverageText('5.15')).toBe('5,15:1')
  })

  it('la pestaña Noticias muestra las del gateway y solo lo que viene del calendario', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Noticias' }))
    const link = await screen.findByRole('link', { name: /Titular de prueba del BCE/ })
    expect(link).toHaveAttribute('href', 'https://example.test/n1')
    expect(link).toHaveAttribute('rel', 'noopener noreferrer')
    const panel = screen.getByRole('tabpanel')
    expect(panel).toHaveTextContent('25 nov, 10:00 UTC')
    const agenda = within(panel).getByRole('region', { name: 'Próximos en el calendario' })
    await within(agenda).findByText('Evento más cercano')
    const names = within(agenda)
      .getAllByRole('listitem')
      .map((li) => li.textContent)
    expect(names[0]).toContain('Evento más cercano')
    expect(names[1]).toContain('Evento más lejano')
    expect(agenda).not.toHaveTextContent('Evento que ya pasó')
    expect(panel).not.toHaveTextContent(/ganancia|garantiz|seguro que/i)
  })

  it('sin gateway, Noticias lo dice y no inventa nada', async () => {
    gateway({ newsDown: true })
    render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Noticias' }))
    expect(
      await screen.findByText('Sin conexión con el gateway: no hay noticias para mostrar.'),
    ).toBeInTheDocument()
    expect(
      await screen.findByText('Sin conexión con el gateway: no hay calendario para mostrar.'),
    ).toBeInTheDocument()
  })
})
