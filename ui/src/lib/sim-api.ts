/**
 * Los simuladores del laboratorio en el gateway (#805). Sin efectos: el motor simula y responde;
 * la UI pide y muestra (CLAUDE.md §1).
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `POST /api/v1/lab/variance` | `variance_sim_request.v1` → `variance_sim_report.v1` |
 * | `GET /api/v1/lab/propsim/templates` | lista de `prop_rules.v1` (hoy, la genérica en blanco) |
 * | `POST /api/v1/lab/propsim` | `prop_sim_request.v1` → `prop_sim_result.v1` |
 *
 * Los POST llevan la cabecera anti-CSRF `x-cfds-token`, que agrega el proxy de desarrollo de Vite
 * (`gateway-token.ts`): el token nunca está en el cliente.
 */
import { useRemote } from '@/lib/liquidity-api'
import { GatewayError, request } from '@/lib/paper-api'
import type {
  PropRules,
  PropSimRequest,
  PropSimResult,
  RuleError,
  VarianceReport,
  VarianceRequest,
} from '@/lib/sim-contracts'

/** Corre el simulador de varianza en el motor. */
export const runVariance = (req: VarianceRequest, signal?: AbortSignal) =>
  request<VarianceReport>('/api/v1/lab/variance', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
    signal,
  })

export const fetchPropTemplates = (signal?: AbortSignal) =>
  request<PropRules[]>('/api/v1/lab/propsim/templates', { signal })

/** Las plantillas de reglas del motor (`config/propsim.toml`). No cambian mientras corre. */
export const usePropTemplates = () => useRemote('propsim-templates', fetchPropTemplates)

/** El motor no simula: el motivo y, si son las reglas, cada error con su ruta. */
export class PropSimRejected extends Error {
  readonly rules: RuleError[]
  constructor(message: string, rules: RuleError[]) {
    super(message)
    this.rules = rules
  }
}

/**
 * Corre el simulador de cuentas fondeadas. Un 422 trae `{error, rules?}`: se lanza
 * `PropSimRejected` con los errores de cada campo; cualquier otro error, `GatewayError`.
 */
export async function runPropSim(req: PropSimRequest, signal?: AbortSignal) {
  const res = await fetch('/api/v1/lab/propsim', {
    method: 'POST',
    headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
    signal,
  })
  if (res.status === 422) {
    const body = (await res.json().catch(() => ({}))) as { error?: string; rules?: RuleError[] }
    throw new PropSimRejected(body.error ?? 'el motor no lo pudo simular', body.rules ?? [])
  }
  if (!res.ok) {
    const detail = await res.text().catch(() => '')
    throw new GatewayError(res.status, detail.trim().slice(0, 200))
  }
  return (await res.json()) as PropSimResult
}
