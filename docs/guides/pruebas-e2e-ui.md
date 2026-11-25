# Pruebas de la UI en un navegador real (Playwright)

Issue #661 (ítem 7 del plan de `docs/analysis/2026-11-autoanalisis.md`). Los tests de `vitest`
corren en jsdom y no ven roturas visuales. Estas pruebas abren la app en Chromium.

## Qué hay

Todo vive en `ui/e2e/` y se corre con `npm run test:e2e` desde `ui/`.

| Archivo | Qué prueba |
|---|---|
| `contracts.spec.ts` | Cada respuesta del gateway simulado cumple su esquema de `contracts/` (con Ajv). |
| `smoke.spec.ts` | Cada vista de `ui/src/views/nav.ts`, a 1400 px y a 375 px: sin errores de consola, sin pedidos a `/api` sin fixture, sin desplazamiento horizontal a 375 px y axe sin violaciones `serious` ni `critical`. |
| `visual.spec.ts` | Una captura de referencia por vista en tema oscuro, tema claro y celular de 375 px. |

Cómo se arman los datos:

- **Gateway simulado** (`e2e/gateway.ts`): Playwright intercepta `/api/**` y contesta desde
  `e2e/fixtures/`. No hay red ni gateway real. Una ruta nueva de la UI necesita su fixture en la
  tabla `ROUTES` con el contrato que cumple. Si falta, la prueba de humo falla y dice qué pedido
  quedó sin respuesta.
- **Datos estables** (`e2e/support.ts`): la hora está congelada en `NOW` (25-11-2026 16:00 UTC), el
  movimiento está reducido, las animaciones apagadas en la captura y la intro y el ingreso salteados
  (`VITE_SKIP_SIGNIN=1`).
- **Docs fijas**: el glosario y el registro de resultados se leen de `docs/` al compilar. En las
  pruebas se reemplazan por copias recortadas de `e2e/fixtures/`, así una fila nueva del registro no
  cambia las capturas.

## Correrlas en tu máquina

```bash
cd ui
npm ci
npx playwright install chromium   # una sola vez
npm run test:e2e                  # contratos y humo; las capturas se saltean fuera de Linux
```

Las capturas de referencia son **de Linux**: en macOS el texto se dibuja distinto y no coincidiría
con el CI. Por eso `visual.spec.ts` se saltea fuera de Linux. **No commitees capturas de macOS.**

## Actualizar las capturas

Cuando un cambio de la UI cambia cómo se ve una vista a propósito, hay que actualizar las capturas
de `ui/e2e/__screenshots__/`. Hay dos caminos.

**1. Desde el CI (no necesita Docker).**

1. Subí el cambio. El job `ui (e2e Playwright)` falla en las vistas que cambiaron.
2. En la corrida, descargá el artefacto `e2e-capturas-linux`. Trae `e2e/__screenshots__/` con todas
   las capturas regeneradas en Linux, más el informe con las diferencias
   (`playwright-report/index.html`).
3. Mirá las diferencias. Si son las buscadas, copiá las capturas a `ui/e2e/__screenshots__/` y
   commitealas.

**2. Con Docker, en la misma imagen que el CI.**

```bash
cd ui
docker run --rm -v "$PWD/..:/repo" -w /repo/ui mcr.microsoft.com/playwright:v1.65.1-noble \
  bash -c "npm ci && npm run test:e2e -- --update-snapshots"
```

Ojo: deja en `ui/node_modules` los paquetes de Linux. Después corré `npm ci` de nuevo en tu máquina.

`npm run test:e2e -- --update-snapshots` regenera las capturas que cambiaron. Corrido fuera de Linux,
no hace nada con las capturas (se saltean).

## En el CI

El job `ui-e2e` de `.github/workflows/ci.yml` corre con los cambios de la UI, dentro de la imagen
oficial `mcr.microsoft.com/playwright:v1.65.1-noble`, con Chromium y sus dependencias ya instalados.
Tiene que coincidir con la versión de `@playwright/test` en `ui/package.json`. Un paso del job lo
verifica: al subir la versión, cambiá también la imagen. El job forma parte de «CI ok».

El umbral de diferencia es del 1 % de los píxeles (`maxDiffPixelRatio` en
`ui/playwright.config.ts`). Las vistas que se ajustan al alto de la ventana (Terminal, Gráficos y
Multigráfico) se capturan sin desplazarse, y las demás con la página entera.
