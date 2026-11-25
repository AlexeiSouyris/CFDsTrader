/**
 * Lectura de noticias y calendario desde el gateway (#600), solo `GET`:
 *
 * | Ruta | Contrato |
 * |---|---|
 * | `GET /api/v1/news` | `gateway_news.v1`: las últimas noticias, con sus interpretaciones |
 * | `GET /api/v1/calendar` | `gateway_calendar.v1`: desde ayer hasta dentro de dos semanas |
 */
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'
import type { GatewayCalendar, GatewayNews } from '@/lib/news-contracts'

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`el gateway respondió ${res.status}`)
  return (await res.json()) as T
}

export const fetchNews = (signal?: AbortSignal) => getJson<GatewayNews>('/api/v1/news', signal)

export const fetchCalendar = (signal?: AbortSignal) =>
  getJson<GatewayCalendar>('/api/v1/calendar', signal)

export const useNews = () => useRemote('news', fetchNews, REFRESH_MS)

export const useCalendar = () => useRemote('calendar', fetchCalendar, REFRESH_MS)
