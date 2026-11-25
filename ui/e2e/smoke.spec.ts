/**
 * Humo e2e (#661): cada vista de `src/views/nav.ts` carga con el gateway simulado, sin errores de
 * consola, sin desplazamiento horizontal a 375 px y sin violaciones serias o críticas de axe.
 */
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'

import { VIEWS } from '../src/views/nav'
import { MOBILE, openView } from './support'

async function seriousViolations(page: import('@playwright/test').Page) {
  const { violations } = await new AxeBuilder({ page }).analyze()
  return violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => `${v.id} (${v.impact}): ${v.nodes.map((n) => n.target.join(' ')).join(' | ')}`)
}

for (const view of VIEWS) {
  test.describe(`#${view.id}`, () => {
    test('escritorio: carga sin errores y pasa axe', async ({ page }) => {
      const { unmocked, errors } = await openView(page, view.id)
      await expect(page).toHaveTitle(new RegExp(view.label))
      expect(unmocked, 'pedidos al gateway sin fixture').toEqual([])
      expect(errors).toEqual([])
      expect(await seriousViolations(page)).toEqual([])
    })

    test('celular 375 px: sin desplazamiento horizontal y pasa axe', async ({ page }) => {
      await page.setViewportSize(MOBILE)
      const { unmocked, errors } = await openView(page, view.id)
      expect(unmocked, 'pedidos al gateway sin fixture').toEqual([])
      expect(errors).toEqual([])
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      )
      expect(overflow, 'píxeles de desplazamiento horizontal').toBeLessThanOrEqual(0)
      expect(await seriousViolations(page)).toEqual([])
    })
  })
}
