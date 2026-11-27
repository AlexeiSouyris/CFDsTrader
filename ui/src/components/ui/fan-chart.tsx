/**
 * Gráfico de abanico de las bandas de capital que manda el motor (`variance_sim_report.v1`):
 * p5–p95 (claro), p25–p75 (más oscuro) y la mediana. La UI solo ubica en pantalla los números que
 * llegan; no calcula nada de trading (CLAUDE.md §1). Escala logarítmica: ganar ×2 y perder la
 * mitad quedan a la misma distancia del 1.
 */
import { decimal } from '@/lib/format'
import type { Band } from '@/lib/sim-contracts'
import { useElementWidth } from '@/lib/use-element-width'

const LEFT = 40
const BOTTOM = 18
const TICKS = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500, 1000]
/** El capital puede llegar a 0: en escala logarítmica se dibuja en el piso. */
const FLOOR = 0.01

export function FanChart({
  bands,
  ruinLevel,
  label,
}: {
  bands: Band[]
  /** Capital que cuenta como ruina (1 = el inicial), para la línea de referencia. */
  ruinLevel?: number
  label: string
}) {
  // Dibujado 1:1 con el ancho real: el texto no se achica en un celular.
  const [ref, W] = useElementWidth<HTMLElement>(600, 260)
  const H = Math.round(Math.min(260, Math.max(180, W * 0.42)))
  if (bands.length < 2) return null
  const v = (s: string) => Math.log10(Math.max(Number(s), FLOOR))
  const lows = bands.map((b) => v(b.p5))
  const highs = bands.map((b) => v(b.p95))
  const lo = Math.min(...lows, ruinLevel ? v(String(ruinLevel)) : 0, 0)
  const hi = Math.max(...highs, 0)
  const span = hi - lo || 1
  const last = bands[bands.length - 1].trade || 1
  const x = (t: number) => LEFT + (t / last) * (W - LEFT - 4)
  const y = (lv: number) => 6 + (1 - (lv - lo) / span) * (H - BOTTOM - 10)
  const pt = (t: number, s: string) => `${x(t).toFixed(1)},${y(v(s)).toFixed(1)}`
  const area = (top: keyof Band, bottom: keyof Band) =>
    [
      ...bands.map((b) => pt(b.trade, String(b[top]))),
      ...[...bands].reverse().map((b) => pt(b.trade, String(b[bottom]))),
    ].join(' ')
  const ticks = TICKS.filter((t) => Math.log10(t) >= lo - 1e-9 && Math.log10(t) <= hi + 1e-9)
  return (
    <figure ref={ref} className="grid gap-1">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={label} className="h-auto w-full">
        {ticks.map((t) => (
          <g key={t}>
            <line
              x1={LEFT}
              x2={W}
              y1={y(Math.log10(t))}
              y2={y(Math.log10(t))}
              stroke="var(--border)"
              strokeDasharray={t === 1 ? undefined : '2 4'}
            />
            <text
              x={LEFT - 4}
              y={y(Math.log10(t)) + 3}
              textAnchor="end"
              fontSize={10}
              fill="var(--muted-foreground)"
            >
              ×{decimal(String(t))}
            </text>
          </g>
        ))}
        {ruinLevel !== undefined && ruinLevel > 0 && (
          <line
            x1={LEFT}
            x2={W}
            y1={y(Math.log10(ruinLevel))}
            y2={y(Math.log10(ruinLevel))}
            stroke="var(--chart-down)"
            strokeDasharray="6 3"
            data-ruin-line
          />
        )}
        <polygon points={area('p95', 'p5')} fill="var(--info)" fillOpacity={0.16} />
        <polygon points={area('p75', 'p25')} fill="var(--info)" fillOpacity={0.32} />
        <polyline
          points={bands.map((b) => pt(b.trade, b.p50)).join(' ')}
          fill="none"
          stroke="var(--primary)"
          strokeWidth={2}
          vectorEffect="non-scaling-stroke"
        />
        {[0, Math.round(last / 2), last].map((t) => (
          <text
            key={t}
            x={x(t)}
            y={H - 4}
            textAnchor={t === 0 ? 'start' : t === last ? 'end' : 'middle'}
            fontSize={10}
            fill="var(--muted-foreground)"
          >
            {t}
          </text>
        ))}
      </svg>
      <figcaption className="text-[11px] text-pretty text-muted-foreground">
        Capital (×1 = el inicial) después de cada operación, en escala logarítmica. Banda clara: 90
        % de los caminos (p5–p95); oscura: la mitad del medio (p25–p75); línea: la mediana.
        {ruinLevel !== undefined && ' Raya roja: el umbral de ruina.'}
      </figcaption>
    </figure>
  )
}
