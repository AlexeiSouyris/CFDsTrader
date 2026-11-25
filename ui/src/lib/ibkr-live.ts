/**
 * Vela en formación de IBKR (#558), **solo visual**: el motor la arma con los precios por
 * streaming y el gateway la manda por SSE (`GET /api/v1/ibkr/live?instrument=<id>&timeframe=<tf>`,
 * `contracts/ibkr_live_bar.v1.schema.json`), como mucho un mensaje por segundo. La UI no arma
 * velas: dibuja la que llega, la marca "en formación" con su tipo de dato y atraso, y cuando llega
 * con `forming = false` la deja fija hasta que la reemplaza la histórica (`ibkr_bars`).
 */
import { useEffect, useState } from 'react'

import type { ChartBar } from '@/lib/liquidity-bars'
import type { Decimal, Utc } from '@/lib/liquidity-contracts'

/** Timeframes de la vela en formación (enum de `ibkr_live_bar.v1` `timeframe`). */
export type IbkrLiveTimeframe = '1m' | '5m' | '15m' | '1h'

export type IbkrLiveStatus =
  'disabled' | 'connecting' | 'disconnected' | 'stopped' | 'rejected' | 'waiting' | 'streaming'

export type IbkrLiveDataType = 'live' | 'delayed' | 'frozen' | 'delayed_frozen'

/** `bar` de `ibkr_live_bar.v1`. */
export type IbkrLiveBar = {
  start: Utc
  end: Utc
  open: Decimal
  high: Decimal
  low: Decimal
  close: Decimal
  /** empezó a armarse a mitad de su período: apertura, máximo y mínimo pueden ser parciales */
  partial: boolean
  ticks: number
}

/** `ibkr_live_bar.v1`. */
export type IbkrLiveMessage = {
  instrument_id: string
  timeframe: IbkrLiveTimeframe
  source: string
  status: IbkrLiveStatus
  reason: string | null
  data_type: IbkrLiveDataType | null
  delay_seconds: number | null
  contract: string | null
  forming: boolean
  bar: IbkrLiveBar | null
  generated_at: Utc
}

/** Lo que se lleva de la conexión: el último mensaje y las velas que llegaron cerradas. */
export type IbkrLiveState = {
  message: IbkrLiveMessage | null
  /** velas que llegaron con `forming = false`, por inicio, hasta que la histórica las reemplace */
  closed: ChartBar[]
}

const EMPTY: IbkrLiveState = { message: null, closed: [] }

/** Velas cerradas por streaming que se guardan, como mucho (las más nuevas). */
const MAX_CLOSED = 240

const toChart = (b: IbkrLiveBar): ChartBar => ({
  start: b.start,
  end: b.end,
  open: b.open,
  high: b.high,
  low: b.low,
  close: b.close,
})

/** Un mensaje nuevo → el estado siguiente. Función pura. */
export function nextLiveState(prev: IbkrLiveState, msg: IbkrLiveMessage): IbkrLiveState {
  let closed = prev.closed
  if (msg.bar && !msg.forming) {
    const bar = toChart(msg.bar)
    closed = [...prev.closed.filter((b) => b.start !== bar.start), bar]
      .sort((x, y) => Date.parse(x.start) - Date.parse(y.start))
      .slice(-MAX_CLOSED)
  }
  // Otro contrato: las cerradas del anterior no se mezclan.
  if (prev.message?.contract && msg.contract && prev.message.contract !== msg.contract) {
    closed = msg.bar && !msg.forming ? [toChart(msg.bar)] : []
  }
  return { message: msg, closed }
}

/**
 * Las velas para dibujar: las históricas, las que llegaron cerradas por streaming y todavía no
 * están en la historia, y al final la vela en formación. Entre cerradas, las históricas mandan
 * (una vela con el mismo inicio queda la de `ibkr_bars`). La vela en formación va después de la
 * última; si la última es del mismo período (#575: la historia la trae, por ejemplo a medias por
 * el atraso de los datos), la reemplaza, porque el período sigue abierto. Una en formación más
 * vieja que la última no se dibuja. Solo si el contrato es el mismo que el de la historia.
 * Ubica velas que manda el motor por su inicio; no calcula ninguna. Función pura.
 */
export function withLiveBars(
  history: ChartBar[],
  contract: string | null | undefined,
  live: IbkrLiveState,
): { bars: ChartBar[]; forming: IbkrLiveMessage | null } {
  const msg = live.message
  if (!msg || !contract || msg.contract !== contract || history.length === 0) {
    return { bars: history, forming: null }
  }
  const lastStart = Date.parse(history[history.length - 1].start)
  const extra = live.closed.filter((b) => Date.parse(b.start) > lastStart)
  const bars = extra.length ? [...history, ...extra] : history
  if (!msg.forming || !msg.bar) return { bars, forming: null }
  const tail = Date.parse(bars[bars.length - 1].start)
  const start = Date.parse(msg.bar.start)
  if (start > tail) return { bars: [...bars, toChart(msg.bar)], forming: msg }
  if (start === tail) return { bars: [...bars.slice(0, -1), toChart(msg.bar)], forming: msg }
  return { bars, forming: null }
}

/**
 * Etiqueta del tipo de dato, tal como lo informa IBKR: "en vivo" o "IBKR · delayed ~N min".
 * Minutos redondeados del atraso que mide el motor (presentación).
 */
export function liveLabel(msg: Pick<IbkrLiveMessage, 'data_type' | 'delay_seconds'>): string {
  if (msg.data_type === 'live') return 'en vivo'
  const kind = msg.data_type ? msg.data_type.replace('_', ' ') : 'sin tipo'
  if (msg.delay_seconds === null) return `IBKR · ${kind}`
  return `IBKR · ${kind} ~${Math.max(0, Math.round(msg.delay_seconds / 60))} min`
}

/** Espera antes de volver a abrir una conexión que el navegador dio por cerrada (se duplica). */
export const RETRY_MIN_MS = 5_000
export const RETRY_MAX_MS = 60_000

/**
 * La vela en formación de un instrumento y timeframe de IBKR por SSE; `null` no abre nada. Si
 * la conexión se corta, el navegador reconecta solo; si la da por cerrada (por ejemplo, el gateway
 * respondió un error), se vuelve a abrir con espera creciente. Sin `EventSource` (tests, entornos
 * viejos) no hay vela en formación.
 */
export function useIbkrLive(sel: { instrument: string; timeframe: string } | null): IbkrLiveState {
  const key = sel ? `${sel.instrument}|${sel.timeframe}` : null
  const [state, setState] = useState<{ key: string | null; live: IbkrLiveState }>({
    key: null,
    live: EMPTY,
  })
  useEffect(() => {
    if (!key || typeof EventSource === 'undefined') return
    const [instrument, timeframe] = key.split('|')
    const url =
      `/api/v1/ibkr/live?instrument=${encodeURIComponent(instrument)}` +
      `&timeframe=${encodeURIComponent(timeframe)}`
    let source: EventSource | null = null
    let timer: ReturnType<typeof setTimeout> | undefined
    let wait = RETRY_MIN_MS
    let alive = true
    const open = () => {
      if (!alive) return
      const es = new EventSource(url)
      source = es
      es.onmessage = (ev: MessageEvent<string>) => {
        let msg: IbkrLiveMessage
        try {
          msg = JSON.parse(ev.data) as IbkrLiveMessage
        } catch {
          return
        }
        wait = RETRY_MIN_MS
        setState((s) => ({
          key,
          live: nextLiveState(s.key === key ? s.live : EMPTY, msg),
        }))
      }
      es.onerror = () => {
        if (es.readyState !== EventSource.CLOSED) return // el navegador reintenta solo
        es.close()
        timer = setTimeout(open, wait)
        wait = Math.min(wait * 2, RETRY_MAX_MS)
      }
    }
    open()
    return () => {
      alive = false
      clearTimeout(timer)
      source?.close()
    }
  }, [key])
  return state.key === key ? state.live : EMPTY
}
