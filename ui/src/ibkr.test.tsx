import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { parseSlots, withInstrumentSel } from '@/lib/chart-slots'
import { fetchOlderIbkrBars, ibkrChartBars } from '@/lib/ibkr-api'
import { ibkrSummaries } from '@/lib/market-summary'
import { CURRENT_MEMBER } from '@/mocks'
import { IBKR_WATCHLIST, ibkrBars } from '@/mocks/ibkr'
import { MultichartView } from '@/views/multichart'
import { TerminalView } from '@/views/terminal'

/** Catálogo del broker simulado, con la forma de `gateway_instruments.v1`. */
const CATALOG = {
  default_instrument: 'DE40',
  default_timeframe: '15m',
  timeframes: ['15m', '1h', '1d'],
  instruments: [
    {
      id: 'DE40',
      name: 'Alemania 40',
      asset_class: 'index',
      currency: 'EUR',
      exchange_timezone: 'Europe/Berlin',
    },
  ],
}

/** Un gateway falso de solo prueba: nada sale al broker simulado real. */
function gateway(opts: { ibkrDown?: boolean } = {}) {
  const urls: string[] = []
  const posts: string[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      const url = String(input)
      urls.push(url)
      if (init?.method === 'POST') posts.push(url)
      const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
      if (url.startsWith('/api/v1/ibkr/') && opts.ibkrDown) return new Response('', { status: 502 })
      if (url === '/api/v1/ibkr/watchlist') return json(IBKR_WATCHLIST)
      if (url.startsWith('/api/v1/ibkr/bars?')) {
        const q = new URLSearchParams(url.split('?')[1])
        return json(
          ibkrBars(q.get('timeframe') as '1m' | '15m' | '1h', {
            instrument: q.get('instrument')!,
          }),
        )
      }
      if (url === '/api/v1/instruments') return json(CATALOG)
      if (url === '/api/v1/paper/account') return new Response('', { status: 502 })
      return new Response('', { status: 404 })
    }),
  )
  return { urls, posts }
}

const markets = () => screen.getByRole('region', { name: 'Mercados' })
const ibkrGroup = () => within(markets()).getByRole('region', { name: 'IBKR' })

describe('datos de IBKR: mapeo puro (#556)', () => {
  it('las velas de IBKR pasan al gráfico con sus precios tal cual y el cierre de cada una', () => {
    const one = ibkrBars('1m', { count: 1 }).bars
    expect(ibkrChartBars(one)).toEqual([
      {
        start: '2026-11-24T14:59:00Z',
        end: '2026-11-24T15:00:00Z',
        open: '21000.25',
        high: '21002.5',
        low: '20999.75',
        close: '21001',
      },
    ])
    expect(ibkrChartBars(ibkrBars('15m', { count: 1 }).bars)[0]).toMatchObject({
      start: '2026-11-24T14:45:00Z',
      end: '2026-11-24T15:00:00Z',
    })
    expect(ibkrChartBars(ibkrBars('1h', { count: 1 }).bars)[0]).toMatchObject({
      start: '2026-11-24T14:00:00Z',
      end: '2026-11-24T15:00:00Z',
    })
    expect(ibkrChartBars([])).toEqual([])
  })

  it('el resumen de IBKR es el del gateway, sin calcular nada; sin velas queda vacío', async () => {
    gateway()
    const all = await ibkrSummaries.load([], new AbortController().signal)
    expect(all).toEqual([
      {
        instrument_id: 'MNQ',
        last: '21000.25',
        at: '2026-11-24T15:01:00Z',
        change_pct: '-0.48',
        delay_s: 840,
      },
      {
        instrument_id: 'MES',
        last: '6000.5',
        at: '2026-11-24T15:01:00Z',
        change_pct: null,
        delay_s: 840,
      },
      { instrument_id: 'QQQ', last: null, at: null, change_pct: null, delay_s: null },
    ])
    const some = await ibkrSummaries.load(['QQQ', 'MNQ'], new AbortController().signal)
    expect(some.map((r) => r.instrument_id)).toEqual(['QQQ', 'MNQ'])
    vi.unstubAllGlobals()
  })

  it('al ir al pasado pide con before y no mezcla contratos', async () => {
    const urls: string[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: string | URL | Request) => {
        urls.push(String(input))
        const contract = urls.length === 1 ? 'MNQZ6' : 'MNQU6'
        return new Response(JSON.stringify(ibkrBars('15m', { contract })), { status: 200 })
      }),
    )
    const same = await fetchOlderIbkrBars('MNQ', '15m', 'MNQZ6', '2026-11-24T14:00:00Z')
    expect(same).toHaveLength(3)
    expect(urls[0]).toBe(
      '/api/v1/ibkr/bars?instrument=MNQ&timeframe=15m&before=2026-11-24T14%3A00%3A00Z',
    )
    // del otro lado del vencimiento hay otro contrato: no se suma
    expect(await fetchOlderIbkrBars('MNQ', '15m', 'MNQZ6', '2026-11-20T14:00:00Z')).toEqual([])
    vi.unstubAllGlobals()
  })

  it('un lugar de IBKR guarda su fuente y sus timeframes; al cambiar de fuente se ajustan', () => {
    const [ibkr, broker1m, broken] = parseSlots([
      { instrument: 'MNQ', timeframe: '1m', feed: 'ibkr' },
      { instrument: 'DE40', timeframe: '1m' },
      { instrument: 'MNQ', timeframe: '1d', feed: 'ibkr' },
    ])
    expect(ibkr).toEqual({ instrument: 'MNQ', timeframe: '1m', feed: 'ibkr' })
    expect(broker1m).toEqual({ instrument: 'DE40', timeframe: '15m' })
    expect(broken).toEqual({ instrument: 'US500', timeframe: '1h' })
    expect(withInstrumentSel(ibkr, 'DE40')).toEqual({ instrument: 'DE40', timeframe: '15m' })
    expect(withInstrumentSel({ instrument: 'DE40', timeframe: '1d' }, 'MES', 'ibkr')).toEqual({
      instrument: 'MES',
      timeframe: '15m',
      feed: 'ibkr',
    })
    expect(withInstrumentSel({ instrument: 'DE40', timeframe: '1h' }, 'MES', 'ibkr')).toEqual({
      instrument: 'MES',
      timeframe: '1h',
      feed: 'ibkr',
    })
  })
})

describe('Terminal con IBKR (#556)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('la lista suma el grupo IBKR con último, variación, hora y atraso tal cual', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    const mnq = (await within(ibkrGroup()).findByText('MNQ')).closest('button')!
    expect(mnq).toHaveTextContent('21.000,25')
    expect(mnq).toHaveTextContent('−0,48 %')
    // la hora del cierre de la vela, de otro día: con la fecha
    expect(mnq).toHaveTextContent(/24\/11 .*UTC/)
    expect(within(mnq).getByText('IBKR · cerró hace ~14 min')).toBeInTheDocument()
    const mes = within(ibkrGroup()).getByText('MES').closest('button')!
    expect(mes).not.toHaveTextContent('%')
    const qqq = within(ibkrGroup()).getByText('QQQ').closest('button')!
    expect(qqq).toHaveTextContent('sin velas')
    expect(ibkrGroup()).toHaveTextContent('solo datos, no operable')
    // los del broker simulado siguen en su grupo
    expect(within(markets()).getByRole('region', { name: 'Broker simulado' })).toHaveTextContent(
      'DE40',
    )
  })

  it('tocar un mercado IBKR abre sus velas en el gráfico; el ticket no lo toma', async () => {
    const gw = gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(await within(ibkrGroup()).findByText('MNQ'))
    await waitFor(() => expect(gw.urls).toContain('/api/v1/ibkr/bars?instrument=MNQ&timeframe=15m'))
    expect(
      await screen.findByText(/IBKR · MNQZ6 · última vela hace ~14 min · solo datos/),
    ).toBeVisible()
    expect(screen.getByText('MNQ · 15 m · IBKR · MNQZ6 · solo datos')).toBeInTheDocument()

    const note = document.querySelector('[data-ibkr-not-tradable]')!
    expect(note).toHaveTextContent('MNQ (IBKR): solo datos, no operable.')
    const ticket = screen.getByRole('form', { name: 'Ticket de orden' })
    expect(within(ticket).getByLabelText('Instrumento')).not.toHaveValue('MNQ')
    expect(
      within(within(ticket).getByLabelText('Instrumento')).queryByRole('option', { name: 'MNQ' }),
    ).toBeNull()

    // timeframes de IBKR: 1m, 15m y 1h
    const tf = screen.getByRole('group', { name: 'Timeframe del gráfico 1' })
    expect(
      within(tf)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['1m', '15m', '1h'])
    fireEvent.click(within(tf).getByRole('button', { name: '1m' }))
    await waitFor(() => expect(gw.urls).toContain('/api/v1/ibkr/bars?instrument=MNQ&timeframe=1m'))
    expect(await screen.findByText('MNQ · 1 m · IBKR · MNQZ6 · solo datos')).toBeInTheDocument()

    // volver a un instrumento del broker: 1m no existe ahí, pasa a 15 m; el aviso se va
    const pick = screen.getByRole('group', { name: 'Instrumento del gráfico 1' })
    fireEvent.click(within(pick).getByRole('button', { name: 'DE40' }))
    expect(within(tf).getByRole('button', { name: '15m' })).toHaveAttribute('aria-pressed', 'true')
    expect(document.querySelector('[data-ibkr-not-tradable]')).toBeNull()
    expect(gw.posts).toEqual([])
  })

  it('sin las rutas de IBKR lo dice, sin datos de ejemplo', async () => {
    gateway({ ibkrDown: true })
    render(<TerminalView member={CURRENT_MEMBER} />)
    await waitFor(() =>
      expect(ibkrGroup()).toHaveTextContent('Sin conexión con el gateway: no hay datos de IBKR.'),
    )
    expect(within(ibkrGroup()).queryAllByRole('button')).toHaveLength(0)
  })
})

describe('Multigráfico con IBKR (#556)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('el selector de instrumento suma los de IBKR y abre sus velas', async () => {
    const gw = gateway()
    render(<MultichartView />)
    const select = screen.getByLabelText('Instrumento del gráfico activo')
    await within(select).findByRole('option', { name: 'MNQ · IBKR' })
    fireEvent.change(select, { target: { value: 'ibkr:MNQ' } })
    await waitFor(() => expect(gw.urls).toContain('/api/v1/ibkr/bars?instrument=MNQ&timeframe=15m'))
    const info = await screen.findByRole('complementary', { name: 'Información del gráfico 1' })
    await waitFor(() => expect(info).toHaveTextContent('MNQZ6'))
    expect(info).toHaveTextContent('no, solo datos')
    expect(info).not.toHaveTextContent('Niveles de liquidez')
    const tf = screen.getByRole('group', { name: 'Timeframe del gráfico activo' })
    expect(within(tf).getByRole('button', { name: '1m' })).toBeInTheDocument()
    expect(JSON.parse(localStorage.getItem('cfdstrader.multichart')!).slots[0]).toEqual({
      instrument: 'MNQ',
      timeframe: '15m',
      feed: 'ibkr',
    })
  })
})
