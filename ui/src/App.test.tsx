import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { squarify } from '@/lib/squarify'
import { SignIn } from '@/components/ui/sign-in'
import { TradingChart } from '@/components/ui/trading-chart'
import { decimal, signed } from '@/lib/format'
import { HEATMAP, US100_1H } from '@/mocks'

describe('la app', () => {
  it('arranca con la pantalla de carga y pasa al ingreso', () => {
    vi.useFakeTimers()
    render(<App />)
    expect(screen.getByRole('status')).toHaveTextContent('Cargando')
    act(() => vi.advanceTimersByTime(1000))
    expect(screen.getByRole('button', { name: 'Entrar' })).toBeInTheDocument()
    vi.useRealTimers()
  })

  it('avisa siempre que los datos son de ejemplo y que no hay promesas de ganancia', () => {
    render(<App initialStage="app" />)
    // el aviso del pie (el Inicio tiene además su propio aviso de ejemplo)
    const note = screen.getAllByRole('note').find((n) => n.textContent?.includes('sello EJEMPLO'))!
    expect(note).toHaveTextContent('sello EJEMPLO')
    expect(note).toHaveTextContent('ni una promesa de ganancia')
  })

  it('navega entre vistas desde la barra de vistas', async () => {
    render(<App initialStage="app" />)
    const nav = screen.getByRole('navigation', { name: 'Vistas' })
    fireEvent.click(within(nav).getByText('Gráficos'))
    expect(screen.getByRole('heading', { level: 1, name: 'Gráficos' })).toBeInTheDocument()
    expect(location.hash).toBe('#graficos')
    expect((await screen.findAllByRole('img', { name: /US100/ })).length).toBeGreaterThan(0)
  })

  it('muestra el 404 para una dirección que no existe', () => {
    history.replaceState(null, '', '/#cualquier-cosa')
    render(<App initialStage="app" />)
    expect(
      screen.getByRole('heading', { level: 1, name: 'Página no encontrada' }),
    ).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Ir al inicio' }))
    expect(screen.getByRole('heading', { level: 1, name: 'Inicio' })).toBeInTheDocument()
  })
})

describe('ingreso', () => {
  it('pide el segundo factor de 6 dígitos y no entra con datos incompletos', () => {
    const onSignIn = vi.fn()
    render(<SignIn onSignIn={onSignIn} />)
    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'a@b.co' } })
    fireEvent.change(screen.getByLabelText('Contraseña'), { target: { value: 'x' } })
    fireEvent.change(screen.getByLabelText('Código del segundo factor'), {
      target: { value: '12a3' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    expect(screen.getByRole('alert')).toHaveTextContent('6 dígitos')
    expect(onSignIn).not.toHaveBeenCalled()
    fireEvent.change(screen.getByLabelText('Código del segundo factor'), {
      target: { value: '123456' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    expect(onSignIn).toHaveBeenCalledOnce()
  })
})

describe('gráfico de precio', () => {
  it('muestra los indicadores tal como llegan en los datos, sin calcularlos', () => {
    render(<TradingChart instrument="US100" bars={US100_1H.candles} />)
    const last = US100_1H.candles[US100_1H.candles.length - 1]
    expect(screen.getByText(decimal(last.rsi14!))).toBeInTheDocument()
    expect(screen.getByText(decimal(last.macd!))).toBeInTheDocument()
    expect(screen.getByText('RSI 14'.toUpperCase())).toBeInTheDocument()
  })
})

describe('utilidades de pantalla', () => {
  it('formatea los decimales sin pasar por float', () => {
    expect(decimal('28365.3')).toBe('28.365,3')
    expect(decimal('-18.4', 2)).toBe('−18,40')
    expect(signed('41.25')).toBe('+41,25')
  })

  it('el mapa de mercado reparte el espacio en proporción a la capitalización', () => {
    const tiles = squarify(HEATMAP, 400, 240)
    const area = tiles.reduce((s, t) => s + t.w * t.h, 0)
    expect(area).toBeCloseTo(400 * 240, 3)
    const total = HEATMAP.reduce((s, c) => s + Number(c.cap), 0)
    for (const t of tiles) {
      expect((t.w * t.h) / area).toBeCloseTo(Number(t.co.cap) / total, 6)
      expect(t.x + t.w).toBeLessThanOrEqual(400 + 1e-6)
      expect(t.y + t.h).toBeLessThanOrEqual(240 + 1e-6)
    }
  })
})
