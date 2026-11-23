/**
 * DATOS DE EJEMPLO de las corridas de backtest (#286), con la forma de
 * `contracts/backtest_run.v1.schema.json`. No son resultados reales.
 * `engine/crates/core/tests/ui_mocks.rs` valida cada corrida contra el contrato.
 *
 * TODO(#286): el gateway todavía no expone `backtest_runs` (PostgreSQL). Cuando exista el
 * endpoint, la pestaña Performance lo lee como la liquidez y esto queda solo para los tests.
 */
import type { BacktestRun, RunDetail } from '@/lib/backtest-contracts'

import detailSample from './backtest-run-detail.json'
import sample from './backtest-runs.json'

export type { BacktestRun, RunDetail } from '@/lib/backtest-contracts'

export const BACKTEST_RUNS = sample.runs as BacktestRun[]

/**
 * DATOS DE EJEMPLO del detalle de una corrida (#299, `backtest_run_detail.v1`): el de la corrida
 * de índices en 1 h. `ui_mocks.rs` lo valida contra el contrato y contra esa corrida.
 */
export const BACKTEST_RUN_DETAIL = detailSample.detail as unknown as RunDetail
