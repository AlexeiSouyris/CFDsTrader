import { act, fireEvent, render, renderHook, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  RETRY_MIN_MS,
  liveLabel,
  nextLiveState,
  useIbkrLive,
  withLiveBars,
  type IbkrLiveMessage,
  type IbkrLiveState,
} from '@/lib/ibkr-live'
import { ibkrChartBars } from '@/lib/ibkr-api'
import type { ChartBar } from '@/lib/liquidity-bars'
import { CURRENT_MEMBER } from '@/mocks'
import { IBKR_WATCHLIST, ibkrBars } from '@/mocks/ibkr'
import { TerminalView } from '@/views/terminal'

/** `EventSource` falso: guarda las conexiones abiertas para mandarles mensajes a mano. */
class FakeEventSource {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSED = 2
  static all: FakeEventSource[] = []
  readyState = FakeEventSource.OPEN
  onmessage: ((ev: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null
  constructor(public url: string) {
    FakeEventSource.all.push(this)
  }
  close() {
    this.readyState = FakeEventSource.CLOSED
  }
  emit(msg: unknown) {
    this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(msg) }))
  }
}

const open = () => FakeEventSource.all.filter((s) => s.readyState !== FakeEventSource.CLOSED)

/** Un mensaje de `ibkr_live_bar.v1` (como los de las pruebas del gateway). */
function message(over: Partial<IbkrLiveMessage> = {}): IbkrLiveMessage {
  return {
    instrument_id: 'MNQ',
    timeframe: '15m',
    source: 'ibkr',
    status: 'streaming',
    reason: null,
    data_type: 'delayed',
    delay_seconds: 600,
    contract: 'MNQZ6',
    forming: true,
    bar: {
      start: '2026-11-24T15:00:00Z',
      end: '2026-11-24T15:15:00Z',
      open: '21003.25',
      high: '21009',
      low: '21001.5',
      close: '21007.75',
      partial: false,
      ticks: 12,
    },
    generated_at: '2026-11-24T15:16:00Z',
    ...over,
  }
}

const history = (): ChartBar[] => ibkrChartBars(ibkrBars('15m').bars) // 14:15, 14:30, 14:45

beforeEach(() => {
  FakeEventSource.all = []
  vi.stubGlobal('EventSource', FakeEventSource)
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('vela en formación de IBKR: armado puro (#558)', () => {
  it('la vela en formación va al final, tal cual llega', () => {
    const live: IbkrLiveState = { message: message(), closed: [] }
    const out = withLiveBars(history(), 'MNQZ6', live)
    expect(out.bars).toHaveLength(4)
    expect(out.bars[3]).toEqual({
      start: '2026-11-24T15:00:00Z',
      end: '2026-11-24T15:15:00Z',
      open: '21003.25',
      high: '21009',
      low: '21001.5',
      close: '21007.75',
    })
    expect(out.forming?.bar?.start).toBe('2026-11-24T15:00:00Z')
  })

  it('al cerrar queda fija hasta que llega la histórica, que manda', () => {
    let s: IbkrLiveState = { message: null, closed: [] }
    s = nextLiveState(s, message({ forming: false }))
    s = nextLiveState(s, message({ status: 'waiting', forming: false }))
    expect(s.closed.map((b) => b.start)).toEqual(['2026-11-24T15:00:00Z'])
    let out = withLiveBars(history(), 'MNQZ6', s)
    expect(out.bars.map((b) => b.start).at(-1)).toBe('2026-11-24T15:00:00Z')
    expect(out.forming).toBeNull()
    // Llega la histórica de 15:00 (con otros precios): queda la histórica, sin repetir.
    const withIt = ibkrChartBars(ibkrBars('15m', { end: '2026-11-24T15:15:00Z' }).bars)
    out = withLiveBars(withIt, 'MNQZ6', s)
    expect(out.bars.filter((b) => b.start === '2026-11-24T15:00:00Z')).toEqual([withIt.at(-1)])
  })

  it('una vela en formación vieja o de otro contrato no se dibuja', () => {
    const old = message({
      bar: { ...message().bar!, start: '2026-11-24T14:45:00Z', end: '2026-11-24T15:00:00Z' },
    })
    expect(withLiveBars(history(), 'MNQZ6', { message: old, closed: [] }).forming).toBeNull()
    const other = message({ contract: 'MNQH7' })
    const out = withLiveBars(history(), 'MNQZ6', { message: other, closed: [] })
    expect(out.bars).toEqual(history())
    // Al cambiar de contrato se olvidan las cerradas del anterior.
    let s = nextLiveState({ message: null, closed: [] }, message({ forming: false }))
    s = nextLiveState(s, message({ contract: 'MNQH7', bar: null, forming: false }))
    expect(s.closed).toEqual([])
  })

  it('la etiqueta dice el tipo de dato y el atraso que informa el motor', () => {
    expect(liveLabel({ data_type: 'delayed', delay_seconds: 600 })).toBe('IBKR · delayed ~10 min')
    expect(liveLabel({ data_type: 'delayed', delay_seconds: null })).toBe('IBKR · delayed')
    expect(liveLabel({ data_type: 'live', delay_seconds: 0 })).toBe('en vivo')
    expect(liveLabel({ data_type: 'delayed_frozen', delay_seconds: 30 })).toBe(
      'IBKR · delayed frozen ~1 min',
    )
  })
})

describe('vela en formación de IBKR: conexión SSE (#558)', () => {
  it('abre la ruta del instrumento y timeframe, y la cierra al cambiar', () => {
    const { result, rerender, unmount } = renderHook(
      ({ sel }: { sel: { instrument: string; timeframe: string } | null }) => useIbkrLive(sel),
      { initialProps: { sel: { instrument: 'MNQ', timeframe: '1m' } } },
    )
    expect(open().map((s) => s.url)).toEqual(['/api/v1/ibkr/live?instrument=MNQ&timeframe=1m'])
    act(() => open()[0].emit(message({ timeframe: '1m' })))
    expect(result.current.message?.bar?.close).toBe('21007.75')
    rerender({ sel: { instrument: 'MES', timeframe: '1m' } })
    expect(open().map((s) => s.url)).toEqual(['/api/v1/ibkr/live?instrument=MES&timeframe=1m'])
    expect(result.current.message).toBeNull()
    rerender({ sel: null })
    expect(open()).toEqual([])
    unmount()
  })

  it('si el navegador da la conexión por cerrada, vuelve a abrirla con espera', () => {
    vi.useFakeTimers()
    renderHook(() => useIbkrLive({ instrument: 'MNQ', timeframe: '15m' }))
    const first = open()[0]
    // Un corte común: el navegador reintenta solo, no se abre otra.
    act(() => first.onerror?.())
    expect(FakeEventSource.all).toHaveLength(1)
    first.readyState = FakeEventSource.CLOSED
    act(() => first.onerror?.())
    expect(open()).toEqual([])
    act(() => vi.advanceTimersByTime(RETRY_MIN_MS))
    expect(open()).toHaveLength(1)
    expect(FakeEventSource.all).toHaveLength(2)
  })

  it('sin EventSource no hay vela en formación', () => {
    vi.stubGlobal('EventSource', undefined)
    const { result } = renderHook(() => useIbkrLive({ instrument: 'MNQ', timeframe: '15m' }))
    expect(result.current).toEqual({ message: null, closed: [] })
  })
})

describe('vela en formación en el gráfico de la Terminal (#558)', () => {
  function gateway() {
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: string | URL | Request) => {
        const url = String(input)
        const json = (b: unknown) => new Response(JSON.stringify(b), { status: 200 })
        if (url === '/api/v1/ibkr/watchlist') return json(IBKR_WATCHLIST)
        if (url.startsWith('/api/v1/ibkr/bars?')) return json(ibkrBars('15m'))
        return new Response('', { status: 502 })
      }),
    )
  }

  it('dibuja la última vela marcada "en formación" con su atraso y la fija al cerrar', async () => {
    gateway()
    render(<TerminalView member={CURRENT_MEMBER} />)
    const markets = screen.getByRole('region', { name: 'Mercados' })
    fireEvent.click(
      await within(within(markets).getByRole('region', { name: 'IBKR' })).findByText('MNQ'),
    )
    await waitFor(() =>
      expect(open().map((s) => s.url)).toContain('/api/v1/ibkr/live?instrument=MNQ&timeframe=15m'),
    )
    await screen.findByText('MNQ · 15 m · IBKR · MNQZ6 · solo datos')
    const sse = open().find((s) => s.url.includes('instrument=MNQ'))!
    act(() => sse.emit(message({ bar: { ...message().bar!, partial: true } })))
    expect(await screen.findByText('En formación · IBKR · delayed ~10 min · parcial')).toBeVisible()
    expect(
      document.querySelector('[data-forming][data-candle="2026-11-24T15:00:00Z"]'),
    ).not.toBeNull()

    // Cierra: la vela queda (sin la marca) y la etiqueta se va.
    act(() => sse.emit(message({ forming: false })))
    await waitFor(() => expect(document.querySelector('[data-forming-label]')).toBeNull())
    expect(document.querySelector('[data-candle="2026-11-24T15:00:00Z"]')).not.toBeNull()
    expect(document.querySelector('[data-forming]')).toBeNull()

    // En vivo (si el dueño contrata datos), sin cambiar código.
    act(() =>
      sse.emit(
        message({
          data_type: 'live',
          delay_seconds: 0,
          bar: { ...message().bar!, start: '2026-11-24T15:15:00Z', end: '2026-11-24T15:30:00Z' },
        }),
      ),
    )
    expect(await screen.findByText('En formación · en vivo')).toBeVisible()
  })
})
