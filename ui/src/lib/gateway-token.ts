/**
 * Cuándo el proxy de desarrollo de Vite agrega la cabecera anti-CSRF `x-cfds-token` (#292).
 *
 * **Lo usa solo `vite.config.ts` (lado servidor), nunca el cliente:** el token sale de la
 * variable `CFDS_GATEWAY_TOKEN` al arrancar Vite y no entra en el bundle ni en el repo.
 *
 * Solo se agrega a los `POST /api/v1/paper/*` que vienen de la misma UI (cabecera `Origin` igual
 * al host del servidor de Vite). Otra web abierta en el navegador no recibe el token.
 */
export const TOKEN_HEADER = 'x-cfds-token'

export function gatewayTokenFor(
  req: { method?: string; url?: string; origin?: string; host?: string },
  token: string | undefined,
): string | null {
  if (!token) return null
  if (req.method !== 'POST' || !req.url?.startsWith('/api/v1/paper/')) return null
  if (!req.origin || !req.host) return null
  try {
    return new URL(req.origin).host === req.host ? token : null
  } catch {
    return null
  }
}
