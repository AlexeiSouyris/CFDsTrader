/**
 * Replay de práctica (#772) en un navegador real, con el gateway simulado: empezar, avanzar velas
 * (solo llegan las que manda el motor), axe sin fallas serias ni críticas y 375 px sin
 * desplazamiento horizontal en la sesión. El examen de una lección (#826), desde su página.
 */
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'

import academyCatalog from './fixtures/academy-catalog.json' with { type: 'json' }
import academyLessonDoc from './fixtures/academy-lesson-doc.json' with { type: 'json' }
import academyProgress from './fixtures/academy-progress.json' with { type: 'json' }
import { NOW } from './gateway'
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
    await page.getByRole('button', { name: 'Empezar' }).click()
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

test('marcas y ticket de estudio: van al motor y se dibujan como vuelven (#773, #791)', async ({
  page,
}) => {
  const { unmocked, errors } = await openView(page, 'replay')
  await page.getByRole('button', { name: 'Empezar' }).click()
  const chart = page.getByRole('img', { name: /Velas del replay/ })
  const box = (await chart.boundingBox())!
  // tocar el gráfico propone el precio del nivel
  await chart.click({ position: { x: box.width / 3, y: box.height / 2 } })
  const marks = page.getByRole('form', { name: 'Marcar en el gráfico' })
  await expect(marks.getByRole('textbox', { name: 'Precio' })).not.toHaveValue('')
  await marks.getByRole('button', { name: 'Marcar' }).click()
  const log = page.getByRole('list', { name: 'Guardado por el motor' })
  await expect(log).toContainText('M1 · Nivel')
  const ticket = page.getByRole('form', { name: 'Ticket de estudio' })
  await ticket.getByRole('textbox', { name: 'Stop' }).fill('19800')
  await ticket.getByRole('textbox', { name: 'Riesgo %' }).fill('1')
  await ticket.getByRole('textbox', { name: 'Plan escrito antes de entrar' }).fill('Plan de prueba')
  await ticket.getByRole('button', { name: 'Mandar al motor' }).click()
  await expect(log).toContainText('E1 · largo · stop 19.800,0')
  expect(unmocked).toEqual([])
  expect(errors).toEqual([])
  expect(await serious(page)).toEqual([])
})

test('examen: al terminar, proceso → niveles → R → fecha real, y la fecha solo al final (#790)', async ({
  page,
}) => {
  const { unmocked, errors } = await openView(page, 'replay')
  await page.getByRole('button', { name: 'Niveles' }).click()
  await page.getByRole('button', { name: 'Empezar' }).click()
  const marks = page.getByRole('form', { name: 'Marcar en el gráfico' })
  await marks.getByRole('textbox', { name: 'Precio' }).fill('19880')
  await marks.getByRole('button', { name: 'Marcar' }).click()
  const ticket = page.getByRole('form', { name: 'Ticket de estudio' })
  await ticket.getByRole('textbox', { name: 'Riesgo %' }).fill('1')
  await ticket.getByRole('button', { name: 'Mandar al motor' }).click()
  await expect(page.getByRole('list', { name: 'Guardado por el motor' })).toContainText('E1')
  // antes de terminar no hay ninguna fecha real en la página
  await expect(page.locator('main')).not.toContainText('2026')
  await page.getByRole('button', { name: 'Terminar y calificar' }).click()
  const result = page.getByRole('article', { name: 'Proceso no aprobado' })
  const titles = await result.getByRole('heading', { level: 3 }).allTextContents()
  expect(titles).toEqual([
    '1. Proceso',
    '2. Niveles',
    '3. Resultado en R (no aprueba ni reprueba)',
    '4. La fecha real',
  ])
  await expect(result).toContainText('Tiene stopno cumplida')
  await expect(result).toContainText('neto — (sin stop no hay R)')
  await expect(result).toContainText('del 18 nov 2026, 00:00 UTC')
  await expect(page.getByRole('form', { name: 'Ticket de estudio' })).toHaveCount(0)
  expect(unmocked).toEqual([])
  expect(errors).toEqual([])
  expect(await serious(page)).toEqual([])
})

test('examen de una lección: se empieza en su página y el resultado dice lo del progreso del motor (#826)', async ({
  page,
}) => {
  const { unmocked, errors } = await openView(page, 'replay')
  // El catálogo de las pruebas es el de los niveles 0–2, sin exámenes de replay: la 1.2 pasa a
  // ser uno de pivotes (de prueba). Después de terminar, el progreso la da por aprobada.
  const catalog = structuredClone(academyCatalog)
  const lesson = catalog.levels.flatMap((l) => l.lessons).find((l) => l.id === '1.2')!
  Object.assign(lesson, {
    exam: { kind: 'replay', ready: true, questions: [], replay_kind: 'pivots' },
  })
  let finished = false
  await page.route('**/api/v1/academy/catalog', (r) => r.fulfill({ json: catalog }))
  await page.route('**/api/v1/academy/lessons/1.2/doc', (r) =>
    r.fulfill({ json: { ...academyLessonDoc, lesson_id: '1.2' } }),
  )
  await page.route('**/api/v1/academy/replay/*/finish', async (r) => {
    finished = true
    await r.fallback()
  })
  await page.route('**/api/v1/academy/progress', (r) => {
    const progress = structuredClone(academyProgress)
    const p = progress.lessons.find((l) => l.lesson_id === '1.2')!
    if (finished) {
      Object.assign(p, { passed: true, passed_at: NOW, attempts: 1, best_score_pct: '100' })
      progress.as_of = new Date(Date.parse(NOW) + 1000).toISOString()
    }
    return r.fulfill({ json: progress })
  })
  await page.evaluate(() => {
    location.hash = '#academia'
  })
  const map = page.getByRole('navigation', { name: 'Mapa de niveles' })
  await map.getByRole('button', { name: /Largo y corto/ }).click()
  const article = page.getByRole('article', { name: /Largo y corto/ })
  await expect(article).toContainText('vas a marcar máximos y mínimos locales')
  expect(await serious(page)).toEqual([])
  await article.getByRole('button', { name: 'Empezar el examen' }).click()
  const session = page.getByRole('region', { name: 'Sesión de replay' })
  await expect(session).toContainText('Examen de la lección 1.2 · Largo y corto')
  await page.getByRole('button', { name: 'Terminar y calificar' }).click()
  const verdict = page.getByRole('region', { name: 'Progreso de la lección 1.2' })
  await expect(verdict).toContainText('Lección 1.2 aprobada, según el progreso del motor')
  expect(await serious(page)).toEqual([])
  await verdict.getByRole('link', { name: 'Volver a la lección' }).click()
  await expect(page.getByRole('article', { name: /Largo y corto/ })).toContainText('Aprobada')
  expect(unmocked).toEqual([])
  expect(errors).toEqual([])
})
