/**
 * Cuándo el proxy de desarrollo de Vite agrega la cabecera anti-CSRF `x-cfds-token` (#292).
 *
 * **Lo usa solo `vite.config.ts` (lado servidor), nunca el cliente:** el token sale de la
 * variable `CFDS_GATEWAY_TOKEN` al arrancar Vite y no entra en el bundle ni en el repo.
 *
 * Solo se agrega a las escrituras (`POST /api/v1/paper/*`, desde #769 `POST /api/v1/academy/*` y
 * desde #778 `POST /api/v1/lab/*` y `DELETE /api/v1/lab/jobs/{id}`) que vienen de la misma UI
 * (cabecera `Origin` igual al host del servidor de Vite). Otra web abierta en el navegador no recibe
 * el token.
 */
export const TOKEN_HEADER = 'x-cfds-token'

/** Prefijos de las rutas del gateway que escriben con `POST` y el token (`paper::write_guard`). */
const POST_WRITE_PREFIXES = ['/api/v1/paper/', '/api/v1/academy/', '/api/v1/lab/']

/** `true` si es una escritura que lleva el token: los POST de paper, de la Academia y del
 * laboratorio, y cancelar un trabajo del laboratorio (`DELETE /api/v1/lab/jobs/{id}`, #778). */
function isWrite(method: string | undefined, url: string | undefined) {
  if (!url) return false
  if (method === 'POST') return POST_WRITE_PREFIXES.some((p) => url.startsWith(p))
  return method === 'DELETE' && url.startsWith('/api/v1/lab/jobs/')
}

export function gatewayTokenFor(
  req: { method?: string; url?: string; origin?: string; host?: string },
  token: string | undefined,
): string | null {
  if (!token) return null
  if (!isWrite(req.method, req.url)) return null
  if (!req.origin || !req.host) return null
  try {
    return new URL(req.origin).host === req.host ? token : null
  } catch {
    return null
  }
}
