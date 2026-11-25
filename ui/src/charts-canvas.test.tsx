/**
 * Gráficos y Multigráfico (#581, etapa 5): lienzo a pantalla completa con `data-section="charts"`,
 * sin vidrio, con la barra flotante que se atenúa y la paleta de comandos aplicada ahí mismo.
 */
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { TOOLS_KEY } from '@/lib/analysis-layers'
import { IDLE_MS } from '@/lib/use-chart-canvas'
import { exampleChart } from '@/test/example-chart'
import { ChartsView } from '@/views'
import { MultichartView } from '@/views/multichart'

const bar = () => document.querySelector('[data-floating-bar]')!
const tfGroup = () => screen.getByRole('group', { name: 'Timeframe del gráfico activo' })
const pressed = (name: string) =>
  within(tfGroup()).getByRole('button', { name }).getAttribute('aria-pressed')

beforeEach(() => localStorage.clear())
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  history.replaceState(null, '', '#')
})

describe('lienzo de gráficos (#581, etapa 5)', () => {
  it('Gráficos y Multigráfico son la sección "charts", sin vidrio', () => {
    for (const View of [ChartsView, MultichartView]) {
      const { container, unmount } = render(<View />)
      const section = container.querySelector('[data-section="charts"]')
      expect(section).not.toBeNull()
      expect(section!.querySelector('.glass, .glass-strong')).toBeNull()
      unmount()
    }
  })

  it('Gráficos muestra un solo gráfico, sin distribuciones ni panel de información', async () => {
    render(<ChartsView />)
    await exampleChart()
    expect(document.querySelectorAll('[data-chart-slot]')).toHaveLength(1)
    expect(screen.queryByRole('group', { name: 'Distribución' })).toBeNull()
    expect(screen.queryByRole('complementary', { name: /Información/ })).toBeNull()
    // guarda lo suyo aparte del Multigráfico
    fireEvent.click(within(tfGroup()).getByRole('button', { name: '1h' }))
    expect(JSON.parse(localStorage.getItem('cfdstrader.charts')!).slots[0].timeframe).toBe('1h')
    expect(localStorage.getItem('cfdstrader.multichart')).toBeNull()
  })

  it('el gráfico llena el alto de la pantalla', async () => {
    render(<ChartsView />)
    await exampleChart()
    const svgs = [...document.querySelectorAll('[data-chart-slot] svg[height]')]
    const tallest = Math.max(...svgs.map((s) => Number(s.getAttribute('height'))))
    // la ventana de jsdom mide 768 px: menos el dock del celular y la cabecera del gráfico
    expect(tallest).toBeGreaterThan(450)
  })

  it('la barra flotante se atenúa con el mouse quieto y vuelve al moverlo o con el teclado', () => {
    // en escritorio: en el celular la barra no flota y no se atenúa
    vi.spyOn(window, 'matchMedia').mockImplementation(
      (media) =>
        ({ matches: true, media, addEventListener() {}, removeEventListener() {} }) as never,
    )
    vi.useFakeTimers()
    render(<MultichartView />)
    expect(bar()).not.toHaveAttribute('data-idle')
    act(() => vi.advanceTimersByTime(IDLE_MS + 10))
    expect(bar()).toHaveAttribute('data-idle', 'true')
    fireEvent.pointerMove(document.querySelector('[data-section="charts"]')!)
    expect(bar()).not.toHaveAttribute('data-idle')
    act(() => vi.advanceTimersByTime(IDLE_MS + 10))
    expect(bar()).toHaveAttribute('data-idle', 'true')
    fireEvent.keyDown(window, { key: 'ArrowLeft' })
    expect(bar()).not.toHaveAttribute('data-idle')
  })

  it('la paleta cambia el timeframe del gráfico activo sin salir de la vista', async () => {
    // un gateway falso de prueba: solo responde el catálogo; lo demás falla (velas de ejemplo)
    const catalog = {
      default_instrument: 'US100',
      default_timeframe: '15m',
      timeframes: ['15m', '1h'],
      instruments: [],
    }
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: string | URL | Request) =>
        new URL(String(input), 'http://localhost').pathname === '/api/v1/instruments'
          ? new Response(JSON.stringify(catalog), { status: 200 })
          : new Response('sin gateway', { status: 503 }),
      ),
    )
    history.replaceState(null, '', '#multigrafico')
    render(<App initialStage="app" />)
    expect(pressed('15m')).toBe('true')
    fireEvent.keyDown(window, { key: 'k', metaKey: true })
    const option = await screen.findByRole('option', { name: /^Timeframe 1h/ })
    expect(option).toHaveTextContent('gráfico activo de esta vista')
    fireEvent.click(option)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(location.hash).toBe('#multigrafico')
    expect(pressed('1h')).toBe('true')
  })

  it('las capas de la paleta se aplican sin salir de Gráficos', () => {
    history.replaceState(null, '', '#graficos')
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: 'k', metaKey: true })
    fireEvent.change(screen.getByRole('combobox', { name: 'Buscar' }), { target: { value: 'ema' } })
    fireEvent.click(screen.getByRole('option', { name: /^Apagar EMA diaria/ }))
    expect(location.hash).toBe('#graficos')
    expect(JSON.parse(localStorage.getItem(TOOLS_KEY)!)).toMatchObject({ ema: false })
  })
})
