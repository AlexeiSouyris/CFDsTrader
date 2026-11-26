/**
 * Los simuladores del laboratorio en el gateway (#805). Sin efectos: el motor simula y responde;
 * la UI pide y muestra (CLAUDE.md §1).
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `POST /api/v1/lab/variance` | `variance_sim_request.v1` → `variance_sim_report.v1` |
 *
 * Los POST llevan la cabecera anti-CSRF `x-cfds-token`, que agrega el proxy de desarrollo de Vite
 * (`gateway-token.ts`): el token nunca está en el cliente.
 */
import { request } from '@/lib/paper-api'
import type { VarianceReport, VarianceRequest } from '@/lib/sim-contracts'

/** Corre el simulador de varianza en el motor. */
export const runVariance = (req: VarianceRequest, signal?: AbortSignal) =>
  request<VarianceReport>('/api/v1/lab/variance', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
    signal,
  })
