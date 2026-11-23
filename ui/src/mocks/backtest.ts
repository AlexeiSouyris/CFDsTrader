/**
 * DATOS DE EJEMPLO de las corridas de backtest (#286), con la forma de
 * `contracts/backtest_run.v1.schema.json`. No son resultados reales.
 * `engine/crates/core/tests/ui_mocks.rs` valida cada corrida contra el contrato.
 *
 * TODO(#286): el gateway todavía no expone `backtest_runs` (PostgreSQL). Cuando exista el
 * endpoint, la pestaña Performance lo lee como la liquidez y esto queda solo para los tests.
 */
import type { BacktestRun } from '@/lib/backtest-contracts'

import sample from './backtest-runs.json'

export type { BacktestRun } from '@/lib/backtest-contracts'

export const BACKTEST_RUNS = sample.runs as BacktestRun[]
