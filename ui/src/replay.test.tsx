/**
 * Replay de práctica (#772): la UI pide sesión y pasos al motor y dibuja solo las velas que
 * vuelven, sin la fecha real. Velas **de prueba** con la forma de `replay_session.v1`.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { keep } from '@/lib/replay-api'
import type { ReplayBar, ReplaySession, ReplayStep } from '@/lib/replay-contracts'
import { ReplayView } from '@/views/replay'

const bar = (index: number): ReplayBar => ({
  index,
  day: 1 + Math.floor(index / 96),
  time: `${String(Math.floor((index % 96) / 4)).padStart(2, '0')}:${String((index % 4) * 15).padStart(2, '0')}`,
  open: '100.0',
  high: '101.5',
  low: '99.0',
  close: index % 2 ? '100.5' : '99.5',
})

const SESSION: ReplaySession = {
  session_id: 'rp-0123456789abcdef',
  kind: 'practice',
  lesson_id: null,
  seed: '7',
  instrument_id: 'US500',
  timeframe: '15m',
  source: 'futures',
  warmup_bars: 3,
  exam_bars: 2,
  cursor: 2,
  bars: [bar(0), bar(1), bar(2)],
  rules: {
    trades_per_exam: 10,
    level_tolerance_atr: '0.25',
    stop_min_atr: '0.5',
    stop_max_atr: '3',
    max_risk_pct: '1',
    provisional: true,
  },
}

function gateway(reply: (path: string, body: unknown) => Response) {
  const posts: unknown[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      const path = new URL(String(input), 'http://ui.test').pathname
      const body = JSON.parse(String(init?.body ?? 'null'))
      posts.push([path, body])
      return reply(path, body)
    }),
  )
  return posts
}

const step = (cursor: number, bars: ReplayBar[], done = false): ReplayStep => ({
  session_id: SESSION.session_id,
  cursor,
  bars,
  done,
})

afterEach(() => {
  vi.unstubAllGlobals()
  keep(null)
})

describe('replay de práctica (#772)', () => {
  it('pide la sesión y avanza: solo se suman las velas que manda el motor', async () => {
    const posts = gateway((path) =>
      path.endsWith('/step')
        ? Response.json(step(4, [bar(2), bar(3), bar(4)], true))
        : Response.json(SESSION),
    )
    render(<ReplayView />)
    fireEvent.change(screen.getByRole('textbox', { name: /Semilla/ }), { target: { value: '7a' } })
    fireEvent.click(screen.getByRole('button', { name: 'Empezar práctica' }))
    const session = await screen.findByRole('region', { name: 'Sesión de replay' })
    expect(session).toHaveTextContent('InstrumentoUS500 · 15m')
    expect(session).toHaveTextContent('Vela3 de 5')
    expect(screen.getByRole('img')).toHaveAccessibleName(/3 a la vista de 3 recibidas del motor/)
    // sin fecha real: día de la sesión y hora UTC
    expect(session).toHaveTextContent('día 1 · 00:30 UTC')
    expect(session).not.toHaveTextContent(/\d{4}-\d{2}-\d{2}/)
    fireEvent.click(within(session).getByRole('button', { name: 'Avanzar 5' }))
    expect(await screen.findByRole('status')).toHaveTextContent('Llegaste a la última vela')
    expect(session).toHaveTextContent('Vela5 de 5')
    // la vela 2 ya estaba: no se duplica
    expect(screen.getByRole('img')).toHaveAccessibleName(/5 a la vista de 5 recibidas/)
    expect(within(session).getByRole('button', { name: 'Avanzar 1' })).toBeDisabled()
    expect(posts).toEqual([
      ['/api/v1/academy/replay', { kind: 'practice', seed: '7' }],
      ['/api/v1/academy/replay/rp-0123456789abcdef/step', { bars: 5 }],
    ])
    fireEvent.click(within(session).getByRole('button', { name: 'Nueva práctica' }))
    expect(screen.getByRole('button', { name: 'Empezar práctica' })).toBeInTheDocument()
  })

  it('sin datos (503) o con la sesión cerrada (404) lo dice claro', async () => {
    gateway(() => new Response('no hay datos suficientes', { status: 503 }))
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Empezar práctica' }))
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'El motor no pudo armar una sesión con velas completas ahora',
    )
    vi.unstubAllGlobals()
    gateway((path) =>
      path.endsWith('/step')
        ? new Response('no hay una sesión abierta con ese id', { status: 404 })
        : Response.json(SESSION),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Empezar práctica' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Avanzar 1' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('La sesión ya no está en el gateway')
  })
})

describe('marcas (#773)', () => {
  const reply = (path: string, body: unknown) => {
    const b = body as Record<string, unknown>
    if (path.endsWith('/marks'))
      return Response.json({ mark_id: 1, bar_index: null, cursor: 2, ...b })
    return Response.json(SESSION)
  }

  it('un pivote va con la vela tocada y su máximo; el motor pone el cursor', async () => {
    const posts = gateway(reply)
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Empezar práctica' }))
    const form = await screen.findByRole('form', { name: 'Marcar en el gráfico' })
    fireEvent.click(within(form).getByRole('button', { name: 'Pivote alto' }))
    expect(form).toHaveTextContent('Tocá la vela del pivote en el gráfico.')
    expect(within(form).getByRole('button', { name: 'Marcar' })).toBeDisabled()
    // jsdom no mide: la vela tocada es la primera columna
    fireEvent.click(screen.getByRole('img'), { clientX: 1, clientY: 1 })
    expect(form).toHaveTextContent('Vela: día 1 · 00:00 UTC')
    expect(within(form).getByRole('textbox', { name: 'Precio' })).toHaveValue('101.5')
    fireEvent.click(within(form).getByRole('button', { name: 'Marcar' }))
    const log = await screen.findByRole('list', { name: 'Guardado por el motor' })
    expect(log).toHaveTextContent(
      'M1 · Pivote alto 101,5 en día 1 · 00:00 · marcado en día 1 · 00:30',
    )
    expect(posts.at(-1)).toEqual([
      '/api/v1/academy/replay/rp-0123456789abcdef/marks',
      { kind: 'pivot_high', price: '101.5', bar_index: 0 },
    ])
  })
})
