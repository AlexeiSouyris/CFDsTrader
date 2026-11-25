/**
 * Abrir una vista de la app para las pruebas e2e (#661): gateway simulado, hora congelada, intro
 * salteada (el ingreso lo saltea `VITE_SKIP_SIGNIN=1` en `playwright.config.ts`) y el tema pedido.
 */
import { readFileSync } from 'node:fs'

import type { Page } from '@playwright/test'

import { mockGateway, NOW } from './gateway'

export type Theme = 'dark' | 'light'

/** El ancho de celular que tiene que entrar sin desplazamiento horizontal. */
export const MOBILE = { width: 375, height: 812 }

export type Opened = {
  /** pedidos a `/api` sin respuesta simulada */
  unmocked: string[]
  /** errores de consola y excepciones de la página */
  errors: string[]
}

/**
 * La UI lee el glosario y el registro de resultados de `docs/` al compilar (`?raw`). Para que las
 * capturas no cambien con cada fila nueva del registro (un PR de solo docs no corre el CI de la UI),
 * el servidor de desarrollo los entrega desde copias recortadas y fijas de `e2e/fixtures/`.
 */
const FROZEN_DOCS: [RegExp, string][] = [
  [/\/registro-de-resultados\.csv\?(.*&)?raw\b/, 'registro-de-resultados.csv'],
  [/\/GLOSARIO\.md\?(.*&)?raw\b/, 'GLOSARIO.md'],
]

async function freezeDocs(page: Page) {
  for (const [pattern, file] of FROZEN_DOCS) {
    const text = readFileSync(new URL(`./fixtures/${file}`, import.meta.url), 'utf8')
    await page.route(pattern, (route) =>
      route.fulfill({
        contentType: 'text/javascript',
        body: `export default ${JSON.stringify(text)}\n`,
      }),
    )
  }
}

export async function openView(page: Page, id: string, theme: Theme = 'dark'): Promise<Opened> {
  const errors: string[] = []
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`console: ${m.text()}`)
  })
  page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`))
  const unmocked = await mockGateway(page)
  await freezeDocs(page)
  await page.clock.setFixedTime(new Date(NOW))
  await page.addInitScript((t) => {
    localStorage.setItem('cfds.intro.seen', '1')
    localStorage.setItem('cfds.theme', t)
  }, theme)
  await page.goto(`/#${id}`)
  await page.waitForLoadState('networkidle')
  await page.evaluate(() => document.fonts.ready)
  await settleLayout(page)
  return { unmocked, errors }
}

/**
 * Espera a que el alto de la página deje de cambiar (paneles que se miden después de montarse,
 * como las tablas con desplazamiento horizontal): 3 lecturas iguales seguidas, cada 150 ms.
 */
async function settleLayout(page: Page) {
  let last = -1
  let same = 0
  for (let i = 0; i < 40 && same < 3; i++) {
    const h = await page.evaluate(() => document.documentElement.scrollHeight)
    same = h === last ? same + 1 : 0
    last = h
    await page.waitForTimeout(150)
  }
}
