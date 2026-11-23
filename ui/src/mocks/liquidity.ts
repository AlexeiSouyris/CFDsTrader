/**
 * DATOS DE EJEMPLO de la vista de liquidez (#231, spec §5.4.1). No son precios reales.
 *
 * Tiene la forma de `liquidity_view.v1` (la respuesta del gateway) más una nota. Lo usan los
 * tests y nada más: la vista lee al motor. `engine/crates/core/tests/ui_mocks.rs` valida cada
 * mensaje contra su contrato.
 */
import type { LiquidityViewData } from '@/lib/liquidity-contracts'

import sample from './liquidity-us100-15m.json'

export type * from '@/lib/liquidity-contracts'

export type LiquiditySnapshot = LiquidityViewData & { note: string }

export const LIQUIDITY_US100 = sample as LiquiditySnapshot
