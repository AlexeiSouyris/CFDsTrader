import { describe, expect, it } from 'vitest'
import { LEVEL_LABEL, LEVEL_SHORT } from '@/lib/liquidity-labels'

describe('etiquetas de niveles de número redondo (#509)', () => {
  it('round_high y round_low tienen nombre largo y corto', () => {
    for (const k of ['round_high', 'round_low'] as const) {
      expect(LEVEL_LABEL[k]).toMatch(/Número redondo/)
      expect(LEVEL_SHORT[k].length).toBeGreaterThan(0)
    }
  })
})
