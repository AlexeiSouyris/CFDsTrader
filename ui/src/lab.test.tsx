/**
 * Laboratorio (#778): el constructor se dibuja desde el catálogo del motor, la UI solo arma la
 * `strategy_spec.v1`, el motor la valida y la corre (progreso por SSE) y el informe se muestra tal
 * cual, siempre "exploratorio, no vota". El catálogo y el informe son los reales del motor
 * (`src/mocks/lab-*.json`).
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { LabReportView } from '@/components/ui/lab-report'
import { gatewayTokenFor } from '@/lib/gateway-token'
import type {
  LabJobEvent,
  LabReport,
  SpecValidation,
  StrategyCatalog,
  StrategySpec,
} from '@/lib/lab-contracts'
import { equityText, errorsAt, initialDraft, toSpec } from '@/lib/lab-spec'
import catalogJson from '@/mocks/lab-catalog.json'
import reportJson from '@/mocks/lab-report.json'
import { LabView } from '@/views/lab'

const CATALOG = catalogJson as unknown as StrategyCatalog
const REPORT = reportJson as unknown as LabReport
const JOB = 'a'.repeat(64)

class FakeEventSource {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSED = 2
  static all: FakeEventSource[] = []
  readyState = FakeEventSource.OPEN
  onmessage: ((ev: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null
  url: string
  constructor(url: string) {
    this.url = url
    FakeEventSource.all.push(this)
  }
  close() {
    this.readyState = FakeEventSource.CLOSED
  }
  emit(msg: unknown) {
    this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(msg) }))
  }
}

const event = (over: Partial<LabJobEvent> = {}): LabJobEvent => ({
  job_id: JOB,
  seq: 1,
  state: 'queued',
  stage: null,
  percent: 0,
  variants_done: 0,
  variants_total: 1,
  report_ids: [],
  error: null,
  recorded_at: '2026-11-27T03:00:00Z',
  ...over,
})

type Call = { url: string; method: string; body: unknown }

/** El gateway simulado: responde según la ruta y anota cada pedido. */
function gateway(validation: (spec: StrategySpec) => SpecValidation) {
  const calls: Call[] = []
  const fetchMock = vi.fn(async (url: string, init: RequestInit = {}) => {
    const method = init.method ?? 'GET'
    const body = init.body ? JSON.parse(String(init.body)) : null
    calls.push({ url, method, body })
    const json = (status: number, data: unknown) =>
      new Response(JSON.stringify(data), {
        status,
        headers: { 'Content-Type': 'application/json' },
      })
    if (url === '/api/v1/lab/catalog') return json(200, CATALOG)
    if (url === '/api/v1/lab/validate') return json(200, validation(body as StrategySpec))
    if (url === '/api/v1/lab/jobs') return json(202, event())
    if (url === `/api/v1/lab/jobs/${JOB}` && method === 'DELETE')
      return json(200, event({ seq: 9, state: 'cancelled' }))
    if (url === `/api/v1/lab/reports/${REPORT.report_id}`) return json(200, REPORT)
    return new Response('sin fixture', { status: 501 })
  })
  vi.stubGlobal('fetch', fetchMock)
  return calls
}

const valid = (): SpecValidation => ({
  valid: true,
  catalog_version: 1,
  spec_sha256: 'b'.repeat(64),
  family_sha256: 'c'.repeat(64),
  errors: [],
})

beforeEach(() => {
  FakeEventSource.all = []
  vi.stubGlobal('EventSource', FakeEventSource)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('la spec que arma la UI', () => {
  it('sale del catálogo, sin rellenar lo apagado ni juzgar rangos', () => {
    const d = initialDraft(CATALOG)
    expect(d.instrument_id).toBe(CATALOG.markets[0].instrument_id)
    expect(d.risk_pct).toBe(CATALOG.limits.max_risk_pct)
    expect(toSpec(CATALOG, d)).toEqual({ spec: null, missing: ['desde', 'hasta'] })
    const exit = { ...d.exit, params: { ...d.exit.params } }
    exit.params.target_r = { on: false, value: '2' }
    exit.params.max_candles = { on: true, value: '12' }
    const { spec } = toSpec(CATALOG, { ...d, exit, from: '2020-01-01', to: '2023-01-01' })
    expect(spec?.period).toEqual({ from: '2020-01-01T00:00:00Z', to: '2023-01-01T00:00:00Z' })
    expect(spec?.mode).toBe('exploratory')
    expect(spec?.exit.params).not.toHaveProperty('target_r')
    expect(spec?.exit.params.max_candles).toBe(12)
    expect(spec?.exit.params.session_close).toBe(true)
    // Un entero mal escrito va tal cual: lo rechaza el motor (param_type), no la UI.
    const entry = {
      ...d.entry[0],
      params: { ...d.entry[0].params, fast: { on: true, value: '2x' } },
    }
    expect(
      toSpec(CATALOG, { ...d, entry: [entry], from: '2020-01-01', to: '2021-01-01' }).spec?.entry[0]
        .params.fast,
    ).toBe('2x')
  })

  it('el capital y los errores por campo', () => {
    expect(equityText(' 100000 ')).toBe('100000')
    expect(equityText('2500,50')).toBe('2500.50')
    for (const bad of ['', '0', '-5', 'mucho']) expect(equityText(bad)).toBeNull()
    const v: SpecValidation = {
      ...valid(),
      valid: false,
      errors: [
        { code: 'param_out_of_range', path: '/entry/0/params/fast', message: 'fuera de rango' },
        { code: 'risk_out_of_range', path: '/risk_pct', message: 'más que config' },
      ],
    }
    expect(errorsAt(v, '/entry/0')).toEqual(['fuera de rango'])
    expect(errorsAt(v, '/entry/1')).toEqual([])
    expect(errorsAt(v, '/risk_pct')).toEqual(['más que config'])
    expect(errorsAt(null, '/risk_pct')).toEqual([])
  })

  it('el proxy pone el token en las escrituras del laboratorio y en nada más', () => {
    const base = { origin: 'http://localhost:5173', host: 'localhost:5173' }
    const with_ = (method: string, url: string) => gatewayTokenFor({ ...base, method, url }, 't')
    expect(with_('POST', '/api/v1/lab/jobs')).toBe('t')
    expect(with_('POST', '/api/v1/lab/validate')).toBe('t')
    expect(with_('DELETE', `/api/v1/lab/jobs/${JOB}`)).toBe('t')
    expect(with_('GET', '/api/v1/lab/catalog')).toBeNull()
    expect(with_('DELETE', '/api/v1/paper/orders')).toBeNull()
  })
})

async function fillPeriod() {
  fireEvent.change(await screen.findByLabelText('Desde (UTC)'), { target: { value: '2020-01-01' } })
  fireEvent.change(screen.getByLabelText('Hasta, sin incluir (UTC)'), {
    target: { value: '2023-01-01' },
  })
}

describe('la vista Laboratorio', () => {
  it('dibuja el constructor desde el catálogo y respeta sus topes', async () => {
    gateway(valid)
    render(<LabView />)
    const form = await screen.findByRole('form', { name: 'Constructor de estrategias' })
    const markets = within(form).getByRole('combobox', { name: 'Instrumento' })
    expect(within(markets).getAllByRole('option')).toHaveLength(CATALOG.markets.length)
    expect(screen.getAllByText(/exploratorio, no vota/i).length).toBeGreaterThan(0)
    const add = within(form).getByRole('button', { name: 'Agregar condición' })
    for (let i = 1; i < CATALOG.limits.max_entry_conditions; i++) fireEvent.click(add)
    expect(within(form).getAllByLabelText(/^Condición \d$/)).toHaveLength(
      CATALOG.limits.max_entry_conditions,
    )
    expect(add).toBeDisabled()
    // Nada se calcula ni se encola sin pedirlo.
    expect(FakeEventSource.all).toHaveLength(0)
  })

  it('muestra en cada campo los errores del motor', async () => {
    const calls = gateway(() => ({
      ...valid(),
      valid: false,
      spec_sha256: null,
      family_sha256: null,
      errors: [
        { code: 'risk_out_of_range', path: '/risk_pct', message: 'el riesgo pasa el de config' },
        {
          code: 'period_touches_holdout',
          path: '/period/to',
          message: 'el período llega al fuera de muestra',
        },
      ],
    }))
    render(<LabView />)
    await fillPeriod()
    fireEvent.change(screen.getByLabelText(/Riesgo por operación/), { target: { value: '5' } })
    fireEvent.click(screen.getByRole('button', { name: 'Validar' }))
    expect(await screen.findByText('el riesgo pasa el de config')).toBeInTheDocument()
    expect(screen.getByText('el período llega al fuera de muestra')).toBeInTheDocument()
    expect(screen.getByLabelText(/Riesgo por operación/)).toHaveAttribute('aria-invalid', 'true')
    const sent = calls.find((c) => c.url === '/api/v1/lab/validate')!.body as StrategySpec
    expect(sent.risk_pct).toBe('5')
    expect(sent.market.source).toBe('futures')
  })

  it('corre, sigue el progreso por SSE y muestra el informe del motor', async () => {
    const calls = gateway(valid)
    render(<LabView />)
    await fillPeriod()
    fireEvent.change(screen.getByLabelText(/Capital hipotético/), { target: { value: '100000' } })
    fireEvent.click(screen.getByRole('button', { name: 'Correr' }))
    await waitFor(() => expect(FakeEventSource.all).toHaveLength(1))
    const job = calls.find((c) => c.url === '/api/v1/lab/jobs')!
    expect(job.method).toBe('POST')
    expect(job.body).toMatchObject({ initial_equity: '100000' })
    expect(FakeEventSource.all[0].url).toBe(`/api/v1/lab/jobs/${JOB}/events`)
    act(() =>
      FakeEventSource.all[0].emit(
        event({ seq: 4, state: 'running', stage: 'running_variants', percent: 35 }),
      ),
    )
    expect(screen.getByRole('progressbar', { name: 'Avance de la corrida' })).toHaveAttribute(
      'aria-valuenow',
      '35',
    )
    expect(screen.getByText(/Corriendo variantes/)).toBeInTheDocument()
    // Un evento viejo que llega tarde no vuelve atrás.
    act(() => FakeEventSource.all[0].emit(event({ seq: 2, state: 'running', stage: 'started' })))
    expect(screen.getByText(/Corriendo variantes/)).toBeInTheDocument()
    act(() =>
      FakeEventSource.all[0].emit(
        event({
          seq: 7,
          state: 'done',
          percent: 100,
          variants_done: 1,
          report_ids: [REPORT.report_id],
        }),
      ),
    )
    expect(FakeEventSource.all[0].readyState).toBe(FakeEventSource.CLOSED)
    const report = await screen.findByRole('article', { name: /^Informe / })
    expect(report.querySelector('[data-lab-label]')?.textContent).toMatch(/exploratorio, no vota/)
    expect(within(report).getByRole('table', { name: 'Bandas de Monte Carlo' })).toBeInTheDocument()
  })

  it('cancela el trabajo en curso', async () => {
    const calls = gateway(valid)
    render(<LabView />)
    await fillPeriod()
    fireEvent.change(screen.getByLabelText(/Capital hipotético/), { target: { value: '5000' } })
    fireEvent.click(screen.getByRole('button', { name: 'Correr' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Cancelar' }))
    await waitFor(() =>
      expect(calls.some((c) => c.method === 'DELETE' && c.url === `/api/v1/lab/jobs/${JOB}`)).toBe(
        true,
      ),
    )
    expect(await screen.findByText('Cancelado')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Cancelar' })).not.toBeInTheDocument()
  })

  it('sin capital no encola nada', async () => {
    const calls = gateway(valid)
    render(<LabView />)
    await fillPeriod()
    fireEvent.click(screen.getByRole('button', { name: 'Correr' }))
    expect(await screen.findByText(/capital hipotético inicial/)).toBeInTheDocument()
    expect(calls.some((c) => c.url === '/api/v1/lab/jobs')).toBe(false)
  })
})

describe('el informe', () => {
  it('muestra lo que manda el motor, sin calcular nada', () => {
    render(<LabReportView report={REPORT} />)
    const r = screen.getByRole('article')
    expect(r.querySelector('[data-lab-label]')?.textContent).toBe(
      `${REPORT.label} · ninguna cifra vota`,
    )
    // N del Sharpe deflactado con su origen, PBO ausente con el motivo.
    expect(
      within(r).getByText(
        `${REPORT.sharpe.trials} (proyecto ${REPORT.sharpe.project_trials} + familia ${REPORT.sharpe.family_variants})`,
      ),
    ).toBeInTheDocument()
    expect(within(r).getByText(/El PBO aparece desde 4 variantes/)).toBeInTheDocument()
    const wf = within(r).getByRole('table', { name: 'Ventanas walk-forward del informe' })
    expect(within(wf).getAllByRole('row')).toHaveLength(REPORT.walk_forward.length + 1)
    expect(within(r).getByRole('img', { name: /Capital mensual/ })).toBeInTheDocument()
    expect(within(r).getByRole('img', { name: /Histograma del R/ })).toBeInTheDocument()
    expect(within(r).getByText(`${REPORT.luck!.simulations} simulaciones`)).toBeInTheDocument()
    for (const n of REPORT.notes) expect(within(r).getByText(`· ${n}`)).toBeInTheDocument()
    // Ningún texto promete ganancias.
    expect(r.textContent).not.toMatch(/ganancia (asegurada|garantizada)|vas a ganar/i)
  })
})
