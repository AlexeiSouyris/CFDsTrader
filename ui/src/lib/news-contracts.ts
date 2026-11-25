/**
 * Tipos de la vista Noticias (#600). Siguen **exactamente** los contratos de `contracts/`:
 * `gateway_news.v1` (que junta `news_item.v1` y `news_interpretation.v1`) y `gateway_calendar.v1`
 * (que junta `economic_event.v1` y `official_release.v1`). Decimales como texto y horas UTC. Lo
 * escribe Python y lo sirve el gateway; la UI solo lo muestra.
 */

type Utc = string
type Decimal = string

/** `news_item.v1`: solo el título y el enlace; el texto completo no se guarda. */
export type NewsItem = {
  source: string
  feed: string
  item_id: string
  title: string
  url: string
  published_at: Utc
  retrieved_at: Utc
}

/** `news_interpretation.v1`: automática; sentimiento y relevancia son estimaciones. */
export type NewsInterpretation = {
  interpretation_id: string
  source: string
  item_id: string
  published_at: Utc
  interpreted_at: Utc
  interpreter: {
    name: string
    version: string
    model: string | null
    prompt_sha256: string | null
  }
  summary_es: string
  impacts: { instrument_id: string; sentiment: Decimal; relevance: Decimal }[]
}

/** `gateway_news.v1`. */
export type GatewayNews = {
  as_of: Utc
  generated_at: Utc
  feed: string | null
  items: { item: NewsItem; interpretations: NewsInterpretation[] }[]
  next_before: Utc | null
}

/** `economic_event.v1`. */
export type EconomicEvent = {
  source: string
  event: string
  country: string | null
  instrument_id: string | null
  impact: 'low' | 'medium' | 'high'
  scheduled_at: Utc
  published_at: Utc
}

/** `official_release.v1`. */
export type OfficialRelease = {
  source: 'fed_fomc' | 'bls'
  series: 'fomc_statement' | 'cpi' | 'employment_situation'
  reference_period: string
  status: 'scheduled' | 'rescheduled' | 'cancelled' | 'unscheduled'
  scheduled_at: Utc | null
  published_at: Utc
  time_rule: string
  with_press_conference: boolean | null
  source_url: string
}

/** `gateway_calendar.v1`. */
export type GatewayCalendar = {
  from: Utc
  to: Utc
  as_of: Utc
  generated_at: Utc
  economic_events: EconomicEvent[]
  official_releases: OfficialRelease[]
}
