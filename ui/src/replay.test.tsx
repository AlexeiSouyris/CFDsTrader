/**
 * Replay de práctica (#772): la UI pide sesión y pasos al motor y dibuja solo las velas que
 * vuelven, sin la fecha real. Velas **de prueba** con la forma de `replay_session.v1`. El examen de
 * una lección (#826) muestra el aprobado que dice el progreso del motor, no el de la UI.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress, LessonProgress } from '@/lib/academy-contracts'
import { fresh, keep } from '@/lib/replay-api'
import type { ExamResult, ReplayBar, ReplaySession, ReplayStep } from '@/lib/replay-contracts'
import { ReplayView } from '@/views/replay'

/** El catálogo y el progreso de las pruebas de la UI; la 2.2 se usa como lección de prueba. */
const CATALOG = catalogFixture as AcademyCatalog
const PROGRESS = progressFixture as AcademyProgress

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
      // solo los POST del replay: el candado de la Academia (#827) también lee catálogo y progreso
      if (init?.method === 'POST') posts.push([path, body])
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
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
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
    fireEvent.click(within(session).getByRole('button', { name: 'Nueva sesión' }))
    expect(screen.getByRole('button', { name: 'Empezar' })).toBeInTheDocument()
  })

  it('sin datos (503) o con la sesión cerrada (404) lo dice claro', async () => {
    gateway(() => new Response('no hay datos suficientes', { status: 503 }))
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'El motor no pudo armar una sesión con velas completas ahora',
    )
    vi.unstubAllGlobals()
    gateway((path) =>
      path.endsWith('/step')
        ? new Response('no hay una sesión abierta con ese id', { status: 404 })
        : Response.json(SESSION),
    )
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Avanzar 1' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('La sesión ya no está en el gateway')
  })
})

describe('marcas (#773) y ticket de estudio (#791)', () => {
  const reply = (path: string, body: unknown) => {
    const b = body as Record<string, unknown>
    if (path.endsWith('/marks'))
      return Response.json({ mark_id: 1, bar_index: null, cursor: 2, ...b })
    if (path.endsWith('/orders'))
      return Response.json({ order_id: 1, stop: null, target: null, cursor: 2, ...b })
    return Response.json(SESSION)
  }

  it('un pivote va con la vela tocada y su máximo; el motor pone el cursor', async () => {
    const posts = gateway(reply)
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
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

  it('el ticket manda lo escrito; sin stop ni plan avisa que el proceso lo reprueba', async () => {
    const posts = gateway(reply)
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
    const ticket = await screen.findByRole('form', { name: 'Ticket de estudio' })
    expect(ticket).toHaveTextContent('Se decide al cierre de día 1 · 00:30 UTC')
    expect(ticket).toHaveTextContent('Sin stop: el proceso lo reprueba.')
    expect(ticket).toHaveTextContent('Sin plan escrito: el proceso lo reprueba.')
    const send = within(ticket).getByRole('button', { name: 'Mandar al motor' })
    expect(send).toBeDisabled()
    fireEvent.click(within(ticket).getByRole('button', { name: 'Corto' }))
    fireEvent.change(within(ticket).getByRole('textbox', { name: 'Objetivo' }), {
      target: { value: '98,5' },
    })
    fireEvent.change(within(ticket).getByRole('textbox', { name: 'Riesgo %' }), {
      target: { value: '0,5' },
    })
    fireEvent.click(send)
    const log = await screen.findByRole('list', { name: 'Guardado por el motor' })
    expect(log).toHaveTextContent('E1 · corto · stop — · obj. 98,5 · riesgo 0,5 %')
    expect(posts.at(-1)).toEqual([
      '/api/v1/academy/replay/rp-0123456789abcdef/orders',
      { side: 'short', risk_pct: '0.5', plan: '', target: '98.5' },
    ])
  })
})

describe('resultado del examen (#790)', () => {
  const RESULT: ExamResult = {
    session_id: SESSION.session_id,
    kind: 'trades',
    lesson_id: null,
    catalog_version: 'e1820742fdc5ff963b7abdae5d8f8da067dabefaa03efd8ef9323a1335121936',
    passed: false,
    rules: [{ rule: 'enough_trades', passed: false }],
    trades: [
      {
        order_id: 1,
        rules: [
          { rule: 'stop_present', passed: true },
          { rule: 'risk_within_limit', passed: true },
        ],
        outcome: {
          entry_index: 3,
          entry_price: '100.0',
          exit_index: 4,
          exit_price: '99.0',
          exit: 'stop',
          r_gross: '-1.00',
          costs_r: '0.08',
          r_net: '-1.08',
        },
      },
    ],
    levels: null,
    revealed: {
      instrument_id: 'US500',
      timeframe: '15m',
      source: 'futures',
      first_bar_at: '2019-03-12T00:00:00Z',
      last_bar_at: '2019-03-12T01:00:00Z',
    },
    graded_at: '2026-11-26T05:00:00Z',
  }

  it('elige el examen, termina y muestra proceso, R y recién ahí la fecha', async () => {
    const posts = gateway((path) =>
      path.endsWith('/finish')
        ? Response.json(RESULT)
        : Response.json({ ...SESSION, kind: 'trades' }),
    )
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Operaciones' }))
    expect(screen.getByText(/Examen libre \(sin lección\)/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Terminar y calificar' }))
    const result = await screen.findByRole('article', { name: 'Proceso no aprobado' })
    const titles = within(result)
      .getAllByRole('heading', { level: 3 })
      .map((h) => h.textContent)
    expect(titles).toEqual([
      '1. Proceso',
      '2. Resultado en R (no aprueba ni reprueba)',
      '3. La fecha real',
    ])
    expect(result).toHaveTextContent('Operaciones suficientes para el examenno cumplida')
    expect(result).toHaveTextContent('Tiene stopcumplida')
    expect(result).toHaveTextContent(
      'E1 · entró en la vela 3 a 100,0, salió por stop a 99,0 · bruto −1,00 R · costos +0,08 R · neto −1,08 R',
    )
    expect(result).toHaveTextContent('del 12 mar 2019, 00:00 UTC al 12 mar 2019, 01:00 UTC')
    expect(posts.map((p) => (p as [string])[0])).toEqual([
      '/api/v1/academy/replay',
      '/api/v1/academy/replay/rp-0123456789abcdef/finish',
    ])
    expect(posts[0]).toEqual(['/api/v1/academy/replay', { kind: 'trades' }])
    // terminada: no se avanza ni se marca
    expect(screen.getByRole('button', { name: 'Avanzar 1' })).toBeDisabled()
    expect(screen.queryByRole('form', { name: 'Ticket de estudio' })).toBeNull()
  })

  it('una práctica no tiene nota', async () => {
    gateway((path) =>
      path.endsWith('/finish')
        ? Response.json({ ...RESULT, kind: 'practice', passed: null, rules: [], trades: [] })
        : Response.json(SESSION),
    )
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Terminar y calificar' }))
    const result = await screen.findByRole('article', { name: 'Práctica: sin nota' })
    expect(result).toHaveTextContent('No hubo entradas.')
  })

  it('examen de pivotes (#800): aciertos, adelantadas y sin pivote, como los calificó el motor', async () => {
    const pivots: ExamResult = {
      ...RESULT,
      kind: 'pivots',
      rules: [
        { rule: 'enough_pivots', passed: false },
        { rule: 'pivot_precision', passed: false },
      ],
      trades: [],
      pivots: {
        strength: 3,
        marked: 3,
        hits: 1,
        early: 1,
        engine: 2,
        covered: 1,
        precision_pct: '33.33',
        coverage_pct: '50',
        marks: [
          { mark_id: 1, status: 'hit', confirmed_at: 4 },
          { mark_id: 2, status: 'early', confirmed_at: 4 },
          { mark_id: 3, status: 'miss', confirmed_at: null },
        ],
        engine_pivots: [
          { bar_index: 1, kind: 'pivot_high', confirmed_at: 4 },
          { bar_index: 1, kind: 'pivot_low', confirmed_at: 4 },
        ],
      },
    }
    const posts = gateway((path) =>
      path.endsWith('/finish')
        ? Response.json(pivots)
        : Response.json({ ...SESSION, kind: 'pivots' }),
    )
    render(<ReplayView />)
    fireEvent.click(screen.getByRole('button', { name: 'Pivotes' }))
    fireEvent.click(screen.getByRole('button', { name: 'Empezar' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Terminar y calificar' }))
    expect(posts[0]).toEqual(['/api/v1/academy/replay', { kind: 'pivots' }])
    const result = await screen.findByRole('article', { name: 'Proceso no aprobado' })
    const titles = within(result)
      .getAllByRole('heading', { level: 3 })
      .map((h) => h.textContent)
    expect(titles).toEqual([
      '1. Proceso',
      '2. Pivotes',
      '3. Resultado en R (no aprueba ni reprueba)',
      '4. La fecha real',
    ])
    expect(result).toHaveTextContent('Pivotes suficientes para el examenno cumplida')
    expect(result).toHaveTextContent('Precisión de pivotes suficienteno cumplida')
    expect(result).toHaveTextContent('Aciertan1Adelantados1Del motor2Precisión33,33 %Cobertura50 %')
    expect(result).toHaveTextContent('P1 · acierta')
    expect(result).toHaveTextContent('P2 · adelantada (el motor lo confirmó recién en la vela 4)')
    expect(result).toHaveTextContent('P3 · sin pivote del motor')
    expect(result).toHaveTextContent('Un pivote existe recién 3 velas después')
  })
})

describe('examen de una lección (#826)', () => {
  const LESSON_SESSION: ReplaySession = { ...SESSION, kind: 'pivots', lesson_id: '2.2' }
  const RESULT: ExamResult = {
    session_id: SESSION.session_id,
    kind: 'pivots',
    lesson_id: '2.2',
    catalog_version: CATALOG.catalog_version,
    passed: true,
    rules: [{ rule: 'enough_pivots', passed: true }],
    trades: [],
    levels: null,
    revealed: {
      instrument_id: 'US500',
      timeframe: '15m',
      source: 'futures',
      first_bar_at: '2019-03-12T00:00:00Z',
      last_bar_at: '2019-03-12T01:00:00Z',
    },
    graded_at: '2026-11-26T05:00:00Z',
  }
  /** El progreso que manda el motor: antes de terminar (`before`) y después (`after`). */
  const progress = (lesson: Partial<LessonProgress>, as_of: string): AcademyProgress => ({
    ...PROGRESS,
    as_of,
    lessons: PROGRESS.lessons.map((l) => (l.lesson_id === '2.2' ? { ...l, ...lesson } : l)),
  })

  /** Gateway de prueba de la Academia: catálogo, progreso (uno por pedido) y el replay. */
  function academy(
    progresses: AcademyProgress[],
    finish: () => Response = () => Response.json(RESULT),
  ) {
    const calls: string[] = []
    const opened: unknown[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
        const path = new URL(String(input), 'http://ui.test').pathname
        calls.push(`${init?.method ?? 'GET'} ${path}`)
        if (path === '/api/v1/academy/catalog') return Response.json(CATALOG)
        if (path === '/api/v1/academy/progress') {
          const p = progresses.shift()
          return p ? Response.json(p) : new Response('sin progreso', { status: 500 })
        }
        if (path.endsWith('/finish')) return finish()
        opened.push(JSON.parse(String(init?.body)))
        return Response.json(LESSON_SESSION)
      }),
    )
    return { calls, opened }
  }

  beforeEach(() => resetAcademy())
  afterEach(() => resetAcademy())

  it('dice de qué lección es y, al terminar, muestra el aprobado del progreso del motor', async () => {
    const { calls } = academy([
      progress({ passed: false, attempts: 0 }, '2026-11-26T04:00:00Z'),
      progress(
        { passed: true, passed_at: '2026-11-26T05:00:00Z', attempts: 1, best_score_pct: '100' },
        '2026-11-26T05:00:01Z',
      ),
    ])
    keep(fresh(LESSON_SESSION))
    render(<ReplayView />)
    const session = screen.getByRole('region', { name: 'Sesión de replay' })
    expect(
      await within(session).findByText(/Examen de la lección 2\.2 · Financiación nocturna/),
    ).toBeInTheDocument()
    expect(session).toHaveTextContent('Cuenta para tu progreso')
    fireEvent.click(within(session).getByRole('button', { name: 'Terminar y calificar' }))
    const verdict = await screen.findByRole('region', { name: 'Progreso de la lección 2.2' })
    // la fecha lleva el año solo si no es el de hoy
    expect(verdict).toHaveTextContent(
      /Lección 2\.2 aprobada, según el progreso del motor \(el 26 nov( 2026)?, 05:00 UTC\)\./,
    )
    expect(within(verdict).getByRole('link', { name: 'Volver a la lección' })).toHaveAttribute(
      'href',
      '#academia',
    )
    // el progreso se vuelve a pedir después de calificar, no antes
    expect(calls.filter((c) => c.endsWith('/progress'))).toHaveLength(2)
    expect(calls.indexOf('POST /api/v1/academy/replay/rp-0123456789abcdef/finish')).toBeLessThan(
      calls.lastIndexOf('GET /api/v1/academy/progress'),
    )
  })

  it('reprobado: lo dice con los intentos guardados y deja otro intento de la misma lección', async () => {
    const { opened } = academy(
      [
        progress({ passed: false, attempts: 0 }, '2026-11-26T04:00:00Z'),
        progress({ passed: false, attempts: 2, best_score_pct: '50' }, '2026-11-26T05:00:01Z'),
      ],
      () => Response.json({ ...RESULT, passed: false }),
    )
    keep(fresh(LESSON_SESSION))
    render(<ReplayView />)
    await screen.findByText(/Examen de la lección 2\.2 · /)
    fireEvent.click(screen.getByRole('button', { name: 'Terminar y calificar' }))
    const verdict = await screen.findByRole('region', { name: 'Progreso de la lección 2.2' })
    expect(verdict).toHaveTextContent(
      'Lección 2.2 todavía no aprobada, según el progreso del motor · 2 intentos guardados.',
    )
    fireEvent.click(screen.getByRole('button', { name: 'Otro intento del examen' }))
    await vi.waitFor(() => expect(opened).toEqual([{ kind: 'pivots', lesson_id: '2.2' }]))
    expect(await screen.findByRole('button', { name: 'Terminar y calificar' })).toBeEnabled()
  })

  it('si el progreso es de antes del intento, no inventa el aprobado: deja pedirlo otra vez', async () => {
    academy([
      progress({ passed: false, attempts: 0 }, '2026-11-26T04:00:00Z'),
      progress({ passed: false, attempts: 0 }, '2026-11-26T04:00:00Z'),
      progress(
        { passed: true, passed_at: '2026-11-26T05:00:00Z', attempts: 1 },
        '2026-11-26T05:00:02Z',
      ),
    ])
    keep(fresh(LESSON_SESSION))
    render(<ReplayView />)
    await screen.findByText(/Examen de la lección 2\.2 · /)
    fireEvent.click(screen.getByRole('button', { name: 'Terminar y calificar' }))
    const verdict = await screen.findByRole('region', { name: 'Progreso de la lección 2.2' })
    expect(verdict).toHaveTextContent('El progreso del motor es de antes de este intento')
    expect(verdict).not.toHaveTextContent('aprobada, según')
    fireEvent.click(within(verdict).getByRole('button', { name: 'Pedir el progreso otra vez' }))
    expect(
      await within(verdict).findByText(/Lección 2\.2 aprobada, según el progreso del motor/),
    ).toBeInTheDocument()
  })
})
