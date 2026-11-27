/**
 * Terminal y Multigráfico con la fuente automática (#865): los lugares del motor piden
 * `source=auto`, dicen qué fuente se ve cuando no es IG y piden las velas viejas a esa misma fuente.
 */
import { render, renderHook, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SlotChart } from '@/components/ui/chart-slot'
import { useChartSlot, type ChartSel } from '@/lib/chart-slots'
import type { LiquidityViewData } from '@/lib/liquidity-contracts'
import { LIQUIDITY_US100 } from '@/mocks'

const FUTURES_LABEL = 'futuros CME/Eurex (base distinta al CFD)'

const view = (fallback: boolean): LiquidityViewData =>
  ({
    ...LIQUIDITY_US100,
    timeframe: '1d',
    source: fallback ? { id: 'futures', label: FUTURES_LABEL } : { id: 'ig', label: 'IG CFD' },
    ...(fallback
      ? { fallback: { from: 'ig', note: 'IG no tiene velas recientes: se muestran futuros.' } }
      : {}),
  }) as LiquidityViewData

function gateway(fallback: boolean) {
  const calls: string[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const url = String(input)
      calls.push(url)
      if (url.startsWith('/api/v1/liquidity?'))
        return new Response(JSON.stringify(view(fallback)), { status: 200 })
      return new Response('', { status: 502 })
    }),
  )
  return calls
}

afterEach(() => {
  vi.unstubAllGlobals()
})

const SEL: ChartSel = { instrument: 'US100', timeframe: '1d' }

describe('lugares de gráfico con la fuente automática (#865)', () => {
  it('piden source=auto y las velas viejas a la fuente que se ve', async () => {
    const calls = gateway(true)
    const { result } = renderHook(() => useChartSlot(SEL))
    await waitFor(() => expect(result.current.source).toBe('motor'))
    expect(calls[0]).toBe('/api/v1/liquidity?instrument=US100&timeframe=1d&source=auto')
    await result.current.loadOlder?.('2026-01-01T00:00:00Z')
    expect(calls.at(-1)).toBe(
      '/api/v1/liquidity?instrument=US100&timeframe=1d&source=futures&before=2026-01-01T00%3A00%3A00Z',
    )
  })

  it('con IG, las velas viejas se piden sin source (IG)', async () => {
    const calls = gateway(false)
    const { result } = renderHook(() => useChartSlot(SEL))
    await waitFor(() => expect(result.current.source).toBe('motor'))
    await result.current.loadOlder?.('2026-01-01T00:00:00Z')
    expect(calls.at(-1)).toBe(
      '/api/v1/liquidity?instrument=US100&timeframe=1d&before=2026-01-01T00%3A00%3A00Z',
    )
  })

  it('el gráfico dice la fuente cuando no es IG', async () => {
    gateway(true)
    function Slot() {
      return <SlotChart slot={useChartSlot(SEL)} height={300} />
    }
    render(<Slot />)
    expect(
      await screen.findByText(new RegExp(FUTURES_LABEL.replace(/[()]/g, '\\$&'))),
    ).toBeVisible()
  })
})
