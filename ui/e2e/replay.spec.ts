/**
 * Replay de práctica (#772) en un navegador real, con el gateway simulado: empezar, avanzar velas
 * (solo llegan las que manda el motor), axe sin fallas serias ni críticas y 375 px sin
 * desplazamiento horizontal en la sesión.
 */
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'

import { MOBILE, openView } from './support'

async function serious(page: import('@playwright/test').Page) {
  const { violations } = await new AxeBuilder({ page }).analyze()
  return violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => v.id)
}

for (const mobile of [false, true]) {
  test(`práctica: empezar y avanzar${mobile ? ' (375 px)' : ''}`, async ({ page }) => {
    if (mobile) await page.setViewportSize(MOBILE)
    const { unmocked, errors } = await openView(page, 'replay')
    await page.getByRole('button', { name: 'Empezar práctica' }).click()
    const session = page.getByRole('region', { name: 'Sesión de replay' })
    await expect(session).toContainText('Vela200 de 288')
    await expect(page.getByRole('img', { name: /200 recibidas del motor/ })).toBeVisible()
    await page.getByRole('button', { name: 'Avanzar 5' }).click()
    await expect(session).toContainText('Vela205 de 288')
    await page.getByRole('button', { name: 'Avanzar 1', exact: true }).press('ArrowRight')
    await expect(session).toContainText('Vela206 de 288')
    await page.getByRole('button', { name: 'Avanzar 100' }).click()
    await expect(page.getByRole('status')).toContainText('Llegaste a la última vela')
    await expect(page.getByRole('button', { name: 'Avanzar 1', exact: true })).toBeDisabled()
    expect(unmocked).toEqual([])
    expect(errors).toEqual([])
    expect(await serious(page)).toEqual([])
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    )
    expect(overflow).toBeLessThanOrEqual(0)
  })
}

test('marcas: van al motor y se dibujan como vuelven (#773)', async ({ page }) => {
  const { unmocked, errors } = await openView(page, 'replay')
  await page.getByRole('button', { name: 'Empezar práctica' }).click()
  const chart = page.getByRole('img', { name: /Velas del replay/ })
  const box = (await chart.boundingBox())!
  // tocar el gráfico propone el precio del nivel
  await chart.click({ position: { x: box.width / 3, y: box.height / 2 } })
  const marks = page.getByRole('form', { name: 'Marcar en el gráfico' })
  await expect(marks.getByRole('textbox', { name: 'Precio' })).not.toHaveValue('')
  await marks.getByRole('button', { name: 'Marcar' }).click()
  const log = page.getByRole('list', { name: 'Guardado por el motor' })
  await expect(log).toContainText('M1 · Nivel')
  expect(unmocked).toEqual([])
  expect(errors).toEqual([])
  expect(await serious(page)).toEqual([])
})
