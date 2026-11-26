/**
 * El gateway simulado de las pruebas e2e (#661): cada `GET /api/v1/...` que pide la UI (y los `POST`
 * de la Academia, #770) se contesta desde `e2e/fixtures/` (o desde un ejemplo de `src/mocks/`), sin
 * red y sin gateway real.
 *
 * Cada ruta dice qué contrato de `contracts/` cumple su respuesta; `contracts.spec.ts` valida todas
 * las respuestas contra su esquema, así un fixture nunca inventa campos (CLAUDE.md §1).
 *
 * Una ruta que no está en la tabla se contesta con 501 y queda anotada: la prueba de humo falla y
 * dice cuál falta (hay que agregarla acá, con su contrato).
 */
import type { Page } from '@playwright/test'

import labCatalog from '../src/mocks/lab-catalog.json' with { type: 'json' }
import labReport from '../src/mocks/lab-report.json' with { type: 'json' }
import liquidityUs100 from '../src/mocks/liquidity-us100-15m.json' with { type: 'json' }
import academyCatalog from './fixtures/academy-catalog.json' with { type: 'json' }
import academyProgress from './fixtures/academy-progress.json' with { type: 'json' }
import analogs from './fixtures/analogs.json' with { type: 'json' }
import backtestRuns from './fixtures/backtest-runs.json' with { type: 'json' }
import calendar from './fixtures/calendar.json' with { type: 'json' }
import emotions from './fixtures/emotions.json' with { type: 'json' }
import fundMovements from './fixtures/fund-movements.json' with { type: 'json' }
import fund from './fixtures/fund.json' with { type: 'json' }
import ibkrBarsMes from './fixtures/ibkr-bars-mes-15m.json' with { type: 'json' }
import ibkrBarsMnq from './fixtures/ibkr-bars-mnq-15m.json' with { type: 'json' }
import ibkrWatchlist from './fixtures/ibkr-watchlist.json' with { type: 'json' }
import instruments from './fixtures/instruments.json' with { type: 'json' }
import journal from './fixtures/journal.json' with { type: 'json' }
import mlExperiments from './fixtures/ml-experiments.json' with { type: 'json' }
import news from './fixtures/news.json' with { type: 'json' }
import opsEvents from './fixtures/ops-events.json' with { type: 'json' }
import opsStatus from './fixtures/ops-status.json' with { type: 'json' }
import paperAccount from './fixtures/paper-account.json' with { type: 'json' }
import sentiment from './fixtures/sentiment.json' with { type: 'json' }

/** La hora congelada de las pruebas: los fixtures están armados para este momento (UTC). */
export const NOW = '2026-11-25T16:00:00Z'

/** Una respuesta del gateway simulado y el contrato que cumple. */
export type MockRoute = {
  /** ruta sin query, por ejemplo `/api/v1/fund` */
  path: string
  /** `GET` si no se dice */
  method?: 'POST'
  /** archivo de `contracts/` que cumple la respuesta */
  schema: string
  /** `true` si la respuesta es una lista de mensajes de ese contrato */
  array?: boolean
  /** la respuesta (puede depender de la query o, en un `POST`, del pedido; sin pedido, uno típico) */
  body: (query: URLSearchParams, request?: unknown) => unknown
  /** `true` si es un flujo SSE: `body` es el único mensaje que se manda */
  sse?: boolean
}

/**
 * `liquidity_view.v1` de US100 desde el ejemplo de `src/mocks/` (sin su nota, que no es del
 * contrato). Los demás instrumentos van sin velas guardadas: la UI muestra "sin datos todavía".
 */
function liquidityView(query: URLSearchParams): unknown {
  const instrument = query.get('instrument') ?? 'US100'
  const timeframe = query.get('timeframe') ?? '15m'
  const source = { id: 'ig', label: 'IG CFD' }
  if (instrument === 'US100' && timeframe === '15m') {
    const view: Record<string, unknown> = {
      ...liquidityUs100,
      timeframe,
      source,
      generated_at: NOW,
    }
    delete view.note
    return view
  }
  return {
    instrument_id: instrument,
    timeframe,
    source,
    generated_at: NOW,
    as_of: null,
    candles: [],
    levels: [],
    fvgs: [],
    imbalance: [],
    stats: [],
  }
}

/** `gateway_ibkr_bars.v1`: velas de MNQ y MES en 15m; otro pedido, sin velas. */
function ibkrBars(query: URLSearchParams): unknown {
  const instrument = query.get('instrument') ?? 'MNQ'
  const timeframe = query.get('timeframe') ?? '15m'
  const fixture = { MNQ: ibkrBarsMnq, MES: ibkrBarsMes }[instrument]
  if (fixture && timeframe === '15m') return fixture
  return {
    instrument_id: instrument,
    timeframe,
    source: 'ibkr',
    as_of: NOW,
    generated_at: NOW,
    series: null,
    bars: [],
  }
}

/**
 * `ibkr_live_bar.v1`: un solo mensaje con la última vela ya cerrada de la historia (no cambia el
 * gráfico, así las capturas no dependen de la hora).
 */
function ibkrLive(query: URLSearchParams): unknown {
  const history = ibkrBars(query) as typeof ibkrBarsMnq
  const last = history.bars.at(-1)
  return {
    instrument_id: history.instrument_id,
    timeframe: history.timeframe,
    source: 'ibkr',
    status: last ? 'streaming' : 'waiting',
    reason: null,
    data_type: last ? 'delayed' : null,
    delay_seconds: last ? 600 : null,
    contract: history.series?.contract ?? null,
    forming: false,
    bar: last
      ? {
          start: last.start,
          end: new Date(Date.parse(last.start) + 15 * 60_000).toISOString().replace('.000Z', 'Z'),
          open: last.open,
          high: last.high,
          low: last.low,
          close: last.close,
          partial: false,
          ticks: 120,
        }
      : null,
    generated_at: NOW,
  }
}

export const ROUTES: MockRoute[] = [
  {
    path: '/api/v1/instruments',
    schema: 'gateway_instruments.v1.schema.json',
    body: () => instruments,
  },
  { path: '/api/v1/liquidity', schema: 'liquidity_view.v1.schema.json', body: liquidityView },
  {
    path: '/api/v1/paper/account',
    schema: 'paper_account.v1.schema.json',
    body: () => paperAccount,
  },
  {
    path: '/api/v1/paper/positions',
    schema: 'position.v1.schema.json',
    array: true,
    body: () => [],
  },
  { path: '/api/v1/ops/status', schema: 'ops_status.v1.schema.json', body: () => opsStatus },
  {
    path: '/api/v1/ops/events',
    schema: 'ops_event.v1.schema.json',
    array: true,
    body: () => opsEvents,
  },
  {
    path: '/api/v1/ibkr/watchlist',
    schema: 'gateway_ibkr_watchlist.v1.schema.json',
    body: () => ibkrWatchlist,
  },
  { path: '/api/v1/ibkr/bars', schema: 'gateway_ibkr_bars.v1.schema.json', body: ibkrBars },
  {
    path: '/api/v1/ibkr/live',
    schema: 'ibkr_live_bar.v1.schema.json',
    sse: true,
    body: ibkrLive,
  },
  {
    path: '/api/v1/paper_comparisons',
    schema: 'paper_comparison.v1.schema.json',
    array: true,
    body: () => [],
  },
  // sin informe todavía: el gateway contesta `null`
  { path: '/api/v1/learning_report', schema: 'learning_report.v1.schema.json', body: () => null },
  {
    path: '/api/v1/learning_report/votes',
    schema: 'change_vote_status.v1.schema.json',
    array: true,
    body: () => [],
  },
  {
    path: '/api/v1/ml/experiments',
    schema: 'ml_experiment_entry.v1.schema.json',
    array: true,
    body: () => mlExperiments,
  },
  {
    path: '/api/v1/backtest_runs',
    schema: 'backtest_run.v1.schema.json',
    array: true,
    body: () => backtestRuns,
  },
  { path: '/api/v1/fund', schema: 'fund_summary.v1.schema.json', body: () => fund },
  {
    path: '/api/v1/fund/movements',
    schema: 'fund_movement.v1.schema.json',
    array: true,
    body: () => fundMovements,
  },
  { path: '/api/v1/calendar', schema: 'gateway_calendar.v1.schema.json', body: () => calendar },
  {
    path: '/api/v1/sentiment',
    schema: 'sentiment.v1.schema.json',
    array: true,
    body: () => sentiment,
  },
  {
    path: '/api/v1/analogs',
    schema: 'analog_report.v1.schema.json',
    array: true,
    body: () => analogs,
  },
  {
    path: '/api/v1/emotions',
    schema: 'emotional_state.v1.schema.json',
    array: true,
    body: () => emotions,
  },
  { path: '/api/v1/news', schema: 'gateway_news.v1.schema.json', body: () => news },
  { path: '/api/v1/journal', schema: 'gateway_journal.v1.schema.json', body: () => journal },
  // el laboratorio (#778): el catálogo que arma el motor con config/lab.toml y un informe real
  // (US500, EMA 20/50 en 1 h, 2020–2022, exploratorio)
  {
    path: '/api/v1/lab/catalog',
    schema: 'strategy_catalog.v1.schema.json',
    body: () => labCatalog,
  },
  {
    path: `/api/v1/lab/reports/${labReport.report_id}`,
    schema: 'lab_report.v1.schema.json',
    body: () => labReport,
  },
  // la Academia (#769): el catálogo de config/academy.toml (niveles 0–2, sin preguntas todavía) y
  // el progreso de alguien que recién empieza (nada aprobado, nada abierto a mano)
  {
    path: '/api/v1/academy/catalog',
    schema: 'academy_catalog.v1.schema.json',
    body: () => academyCatalog,
  },
  {
    path: '/api/v1/academy/progress',
    schema: 'academy_progress.v1.schema.json',
    body: () => academyProgress,
  },
  {
    path: '/api/v1/academy/events',
    method: 'POST',
    schema: 'academy_event.v1.schema.json',
    body: academyEvent,
  },
]

/** `academy_event.v1`: el evento como lo guardaría el motor (lección completada o desbloqueo a mano). */
function academyEvent(_query: URLSearchParams, request?: unknown): unknown {
  const req = (request ?? { kind: 'lesson_completed', lesson_id: '0.3' }) as Record<string, string>
  const override = req.kind === 'tool_override'
  return {
    seq: 1,
    member_id: academyProgress.member_id,
    kind: req.kind,
    lesson_id: override ? null : req.lesson_id,
    tool_id: override ? req.tool_id : null,
    action: override ? req.action : null,
    quiz: null,
    catalog_version: academyCatalog.catalog_version,
    recorded_at: NOW,
  }
}

/**
 * Intercepta todo `/api/**` de la página. Devuelve la lista de pedidos que no tienen respuesta
 * simulada (se contestan con 501).
 */
export async function mockGateway(page: Page): Promise<string[]> {
  const unmocked: string[] = []
  await page.route('**/api/**', async (route) => {
    const req = route.request()
    const url = new URL(req.url())
    const match = ROUTES.find(
      (r) => r.path === url.pathname && (r.method ?? 'GET') === req.method(),
    )
    if (!match) {
      unmocked.push(`${req.method()} ${url.pathname}${url.search}`)
      return route.fulfill({ status: 501, contentType: 'text/plain', body: 'sin fixture' })
    }
    const body = match.body(
      url.searchParams,
      req.method() === 'POST' ? (req.postDataJSON() as unknown) : undefined,
    )
    if (match.sse) {
      // un mensaje y se corta; `retry` largo para que el navegador no vuelva a pedir en la prueba
      return route.fulfill({
        contentType: 'text/event-stream',
        body: `retry: 600000\n\ndata: ${JSON.stringify(body)}\n\n`,
      })
    }
    return route.fulfill({ json: body })
  })
  return unmocked
}
