/**
 * El laboratorio de estrategias en el gateway (#776, #778). La UI **solo arma la spec**
 * (`strategy_spec.v1`): el motor la valida, la corre (en el `lab_worker`, nunca en el gateway ni
 * en la UI) y arma el informe. La UI pide y muestra.
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `GET /api/v1/lab/catalog` | `strategy_catalog.v1` |
 * | `POST /api/v1/lab/validate` | `strategy_spec.v1` → `spec_validation.v1` |
 * | `POST /api/v1/lab/jobs` | `lab_job_request.v1` → `lab_job_event.v1` |
 * | `GET` / `DELETE /api/v1/lab/jobs/{job_id}` | `lab_job_event.v1` (el último; cancelar) |
 * | `GET /api/v1/lab/jobs/{job_id}/events` | SSE de `lab_job_event.v1` |
 * | `GET /api/v1/lab/reports/{report_id}` | `lab_report.v1` |
 *
 * Las escrituras llevan la cabecera anti-CSRF `x-cfds-token`, que **no pone el navegador**: la
 * agrega el proxy de desarrollo de Vite (`gateway-token.ts`). El token nunca está en el cliente.
 */
import { useEffect, useState } from 'react'

import type {
  LabJobEvent,
  LabJobRequest,
  LabReport,
  SpecValidation,
  StrategyCatalog,
  StrategySpec,
} from '@/lib/lab-contracts'
import { useRemote } from '@/lib/liquidity-api'
import { GatewayError } from '@/lib/paper-api'

async function request<T>(url: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(url, {
    ...init,
    headers: { Accept: 'application/json', ...init.headers },
  })
  if (!res.ok) {
    const detail = await res.text().catch(() => '')
    throw new GatewayError(res.status, detail.trim().slice(0, 400))
  }
  return (await res.json()) as T
}

const post = <T>(url: string, body: unknown) =>
  request<T>(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })

const job = (id: string) => `/api/v1/lab/jobs/${encodeURIComponent(id)}`

export const fetchLabCatalog = (signal?: AbortSignal) =>
  request<StrategyCatalog>('/api/v1/lab/catalog', { signal })

/** El catálogo del motor (qué bloques y parámetros existen). No cambia mientras corre el gateway. */
export const useLabCatalog = () => useRemote('lab-catalog', fetchLabCatalog)

/** El veredicto del motor sobre una spec: válida o la lista completa de errores. */
export const validateSpec = (spec: StrategySpec) =>
  post<SpecValidation>('/api/v1/lab/validate', spec)

/** Encola la corrida. El mismo pedido devuelve el mismo trabajo (idempotente). */
export const submitJob = (req: LabJobRequest) => post<LabJobEvent>('/api/v1/lab/jobs', req)

export const fetchJob = (id: string, signal?: AbortSignal) =>
  request<LabJobEvent>(job(id), { signal })

/** Cancela el trabajo. Si ya terminó, devuelve su final. */
export const cancelJob = (id: string) => request<LabJobEvent>(job(id), { method: 'DELETE' })

export const fetchReport = (id: string, signal?: AbortSignal) =>
  request<LabReport>(`/api/v1/lab/reports/${encodeURIComponent(id)}`, { signal })

/** Los informes de un trabajo terminado, en el orden de las variantes. Un informe no cambia. */
export const useLabReports = (ids: string[] | null) =>
  useRemote(ids && ids.length > 0 ? `lab-reports|${ids.join(',')}` : null, (signal) =>
    Promise.all(ids!.map((id) => fetchReport(id, signal))),
  )

const ENDED: LabJobEvent['state'][] = ['done', 'failed', 'cancelled']
export const isEnded = (e: LabJobEvent | null) => e !== null && ENDED.includes(e.state)

/** Sin `EventSource`, cada cuánto se pide el estado del trabajo. */
export const JOB_POLL_MS = 2_000

/**
 * El último evento del trabajo `id` (empieza en `first`): por SSE mientras corre (el gateway
 * reconecta con `Last-Event-ID`) y se cierra al llegar el final. Sin `EventSource`, pide el
 * estado cada `JOB_POLL_MS`. Nunca vuelve atrás: un evento con `seq` menor se ignora.
 */
export function useLabJob(id: string | null, first: LabJobEvent | null) {
  const [event, setEvent] = useState<LabJobEvent | null>(first)
  const [error, setError] = useState<string | null>(null)
  /** Se queda con el más nuevo del mismo trabajo; otro trabajo reemplaza. */
  const take = (e: LabJobEvent | null) =>
    setEvent((prev) => (prev && e && prev.job_id === e.job_id && prev.seq >= e.seq ? prev : e))
  // Lo que devuelve el gateway al encolar o cancelar también cuenta.
  const [seen, setSeen] = useState(first)
  if (seen !== first) {
    setSeen(first)
    take(first)
  }
  const [seenId, setSeenId] = useState(id)
  if (seenId !== id) {
    setSeenId(id)
    setError(null)
  }
  useEffect(() => {
    if (!id || isEnded(first)) return
    if (typeof EventSource !== 'undefined') {
      const source = new EventSource(`${job(id)}/events`)
      source.onmessage = (m: MessageEvent<string>) => {
        try {
          const e = JSON.parse(m.data) as LabJobEvent
          take(e)
          if (isEnded(e)) source.close()
        } catch {
          setError('el gateway mandó un evento que no se pudo leer')
        }
      }
      source.onerror = () => {
        if (source.readyState === EventSource.CLOSED) setError('se cortó el progreso del gateway')
      }
      return () => source.close()
    }
    let stop = false
    const ctrl = new AbortController()
    const tick = async () => {
      while (!stop) {
        try {
          const e = await fetchJob(id, ctrl.signal)
          take(e)
          if (isEnded(e)) return
        } catch (e) {
          if (!stop) setError(e instanceof Error ? e.message : String(e))
        }
        await new Promise((r) => setTimeout(r, JOB_POLL_MS))
      }
    }
    void tick()
    return () => {
      stop = true
      ctrl.abort()
    }
    // Solo al cambiar de trabajo: lo demás llega por el SSE.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id])
  return { event, error }
}
