/**
 * Capturas de referencia por vista (#661): tema oscuro y claro a 1400 px, y celular de 375 px.
 * Datos estables: gateway simulado, hora congelada, sin animaciones (movimiento reducido) e intro
 * salteada.
 *
 * Las capturas de referencia son de Linux (la imagen de Playwright del CI): en macOS o Windows el
 * texto se dibuja distinto, así que acá se saltean. Para actualizarlas, ver
 * `docs/guides/pruebas-e2e-ui.md`.
 */
import { expect, test } from '@playwright/test'

import { VIEWS } from '../src/views/nav'
import { MOBILE, openView, type Theme } from './support'

test.skip(
  process.platform !== 'linux' && !process.env.E2E_VISUAL_ANY_OS,
  'las capturas de referencia se generan y comparan en Linux',
)

const CASES: { name: string; theme: Theme; mobile?: boolean }[] = [
  { name: 'oscuro', theme: 'dark' },
  { name: 'claro', theme: 'light' },
  { name: 'celular', theme: 'dark', mobile: true },
]

/**
 * Vistas que se ajustan al alto de la ventana (gráficos a pantalla completa): con captura de página
 * entera el alto cambia mientras se saca la foto, así que se captura solo la ventana.
 */
const FIT_VIEWPORT = new Set(['terminal', 'graficos', 'multigrafico'])

for (const view of VIEWS) {
  for (const c of CASES) {
    test(`#${view.id} ${c.name}`, async ({ page }) => {
      if (c.mobile) await page.setViewportSize(MOBILE)
      await openView(page, view.id, c.theme)
      await expect(page).toHaveScreenshot(`${view.id}-${c.name}.png`, {
        fullPage: !FIT_VIEWPORT.has(view.id),
      })
    })
  }
}
