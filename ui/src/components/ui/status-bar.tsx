/**
 * Barra de estado de la terminal (#708): relojes de las plazas principales y frescura de las
 * fuentes de datos, como la última línea de una terminal profesional.
 *
 * - Relojes: la hora local de cada plaza con su zona IANA (`Intl.DateTimeFormat`). No dicen si el
 *   mercado está abierto: los horarios de sesión son configuración del motor y la UI no los calcula.
 * - Fuentes: lo que manda el gateway, tal cual. El gateway (`ops_status.v1`: estado y `as_of`),
 *   IBKR (`gateway_ibkr_watchlist.v1`: el atraso que mide el gateway) y las noticias
 *   (`gateway_news.v1`: la hora de publicación de la más nueva). La UI solo dice hace cuánto.
 */
import { useEffect, useState } from 'react'

import { fetchIbkrWatchlist } from '@/lib/ibkr-api'
import { REFRESH_MS, useRemote } from '@/lib/liquidity-api'
import { ageText } from '@/lib/format'
import { CLOCKS, clockText } from '@/lib/market-clocks'
import { useNews } from '@/lib/news-api'
import { useOpsStatus, type Gate } from '@/lib/ops-api'
import { cn } from '@/lib/utils'

/** La hora actual, que avanza cada `every` ms. */
function useNow(every = 1000) {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), every)
    return () => window.clearInterval(t)
  }, [every])
  return now
}

const GATE_TEXT: Record<Gate, string> = {
  starting: 'arrancando',
  ready: 'listo',
  stopping: 'deteniéndose',
  unavailable: 'no disponible',
}

type Tone = 'ok' | 'warn' | 'off' | 'wait'
const DOT: Record<Tone, string> = {
  ok: 'bg-up',
  warn: 'bg-chart-signal',
  off: 'bg-down',
  wait: 'bg-muted-foreground',
}

/** Una fuente: punto (decorativo) + nombre + estado en palabras (no solo color). */
function Source({ name, tone, text }: { name: string; tone: Tone; text: string }) {
  return (
    <li className="flex items-center gap-1.5 whitespace-nowrap">
      <i aria-hidden className={cn('size-1.5 rounded-full', DOT[tone])} />
      <span className="font-semibold text-muted-foreground">{name}</span>
      <span>{text}</span>
    </li>
  )
}

export function StatusBar({ className }: { className?: string }) {
  const now = useNow()
  const at = new Date(now)
  const ops = useOpsStatus()
  const ibkr = useRemote('status-ibkr', fetchIbkrWatchlist, REFRESH_MS)
  const news = useNews()

  const gateway: { tone: Tone; text: string } =
    ops.status === 'loading'
      ? { tone: 'wait', text: 'conectando…' }
      : ops.status === 'error'
        ? { tone: 'off', text: 'sin conexión' }
        : {
            tone: ops.data?.gate === 'ready' ? 'ok' : 'warn',
            text: `${GATE_TEXT[ops.data?.gate] ?? 'sin estado'} · ${ageText(ops.data?.as_of, now) ?? '—'}`,
          }

  // el menor atraso que informa el gateway entre las cotizaciones que llegaron (una respuesta
  // que no es la del contrato cuenta como sin cotizaciones: la barra nunca rompe la app)
  const delays =
    ibkr.status === 'ok' && Array.isArray(ibkr.data?.instruments)
      ? ibkr.data.instruments.flatMap((i) =>
          typeof i?.quote?.delay_seconds === 'number' ? [i.quote.delay_seconds] : [],
        )
      : []
  const ibkrState: { tone: Tone; text: string } =
    ibkr.status === 'loading'
      ? { tone: 'wait', text: '…' }
      : ibkr.status === 'error'
        ? { tone: 'off', text: 'sin datos' }
        : delays.length === 0
          ? { tone: 'warn', text: 'sin cotizaciones' }
          : { tone: 'ok', text: `~${Math.round(Math.min(...delays) / 60)} min de atraso` }

  const newest =
    news.status === 'ok' && Array.isArray(news.data?.items)
      ? news.data.items.reduce<string | null>((a, it) => {
          const at = it?.item?.published_at
          return typeof at === 'string' && (a === null || at > a) ? at : a
        }, null)
      : null
  const newsState: { tone: Tone; text: string } =
    news.status === 'loading'
      ? { tone: 'wait', text: '…' }
      : news.status === 'error'
        ? { tone: 'off', text: 'sin datos' }
        : newest === null
          ? { tone: 'warn', text: 'sin titulares' }
          : { tone: 'ok', text: `última ${ageText(newest, now) ?? '—'}` }

  return (
    <footer
      aria-label="Estado"
      className={cn(
        'flex flex-wrap items-center gap-x-5 gap-y-1 border-t border-rule bg-card px-3 py-1.5 font-mono text-[11px] leading-tight',
        className,
      )}
    >
      <ul aria-label="Relojes" className="flex flex-wrap items-center gap-x-4 gap-y-1">
        {CLOCKS.map((c, i) => (
          <li key={c.zone} className="flex items-center gap-1.5 whitespace-nowrap">
            <span className="font-semibold text-info">{c.label}</span>
            <time dateTime={at.toISOString()} aria-label={`${c.name}: ${clockText(at, c.zone)}`}>
              {clockText(at, c.zone, i === 0)}
            </time>
          </li>
        ))}
      </ul>
      <ul aria-label="Fuentes de datos" className="flex flex-wrap items-center gap-x-4 gap-y-1">
        <Source name="GATEWAY" {...gateway} />
        <Source name="IBKR" {...ibkrState} />
        <Source name="NOTICIAS" {...newsState} />
      </ul>
      <p className="ml-auto hidden items-center gap-3 whitespace-nowrap lg:flex">
        <span className="text-muted-foreground">
          <kbd>F1</kbd>–<kbd>F10</kbd> secciones · <kbd>/</kbd> comando · <kbd>?</kbd> atajos
        </span>
        <strong className="font-bold tracking-[0.12em] text-primary">SIMULACIÓN</strong>
      </p>
    </footer>
  )
}
