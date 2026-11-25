/**
 * Mapa de mercado: un cuadro por empresa, del tamaño de su capitalización y del color de su
 * cambio del día (verde sube, rojo baja; más intenso, más movimiento).
 * Inspirado en "Market Heatmap" (21st.dev), reescrito (#164) con `motion` (la única librería
 * de animación de la app) y el formato en español. El reparto del espacio (treemap
 * "squarified") es geometría de dibujo, no lógica de trading.
 */
import { motion, useReducedMotion } from 'motion/react'
import { useMemo, useState } from 'react'

import { decimal, signed } from '@/lib/format'
import { squarify } from '@/lib/squarify'
import { useElementWidth } from '@/lib/use-element-width'
import type { HeatmapCompany } from '@/mocks'

const fill = (chg: number) =>
  `color-mix(in srgb, ${chg >= 0 ? 'var(--chart-up)' : 'var(--chart-down)'} ${Math.round(Math.min(Math.abs(chg) / 3, 1) * 55 + 10)}%, var(--card))`

export function MarketHeatmap({
  data,
  title = 'NASDAQ 100 · hoy',
}: {
  data: HeatmapCompany[]
  title?: string
}) {
  const reduced = useReducedMotion()
  const [ref, width] = useElementWidth<HTMLDivElement>(460, 240)
  const height = Math.round(width * 0.6)
  const [hot, setHot] = useState<string | null>(null)
  const tiles = useMemo(
    () =>
      squarify(
        [...data].sort((a, b) => Number(b.cap) - Number(a.cap)),
        width,
        height,
      ),
    [data, width, height],
  )
  const tile = tiles.find((t) => t.co.sym === hot)

  return (
    <div className="w-full">
      <div className="mb-3 flex flex-wrap items-baseline justify-between gap-x-2">
        <span className="text-[14px] font-bold">{title}</span>
        <span className="text-[10px] text-muted-foreground">tamaño = capitalización</span>
      </div>
      <div
        ref={ref}
        className="relative"
        tabIndex={0}
        role="group"
        aria-label={`${title}. Flechas para recorrer las empresas.`}
        onBlur={() => setHot(null)}
        onKeyDown={(e) => {
          const at = tiles.findIndex((t) => t.co.sym === hot)
          const step =
            e.key === 'ArrowRight' || e.key === 'ArrowDown'
              ? 1
              : e.key === 'ArrowLeft' || e.key === 'ArrowUp'
                ? -1
                : 0
          if (!step) return
          e.preventDefault()
          setHot(tiles[(at + step + tiles.length) % tiles.length].co.sym)
        }}
      >
        <svg
          width={width}
          height={height}
          viewBox={`0 0 ${width} ${height}`}
          className="block rounded-xl"
          aria-hidden="true"
          onPointerLeave={() => setHot(null)}
        >
          {tiles.map((t, i) => {
            const chg = Number(t.co.chg)
            const big = t.w > 56 && t.h > 34
            return (
              <motion.g
                key={t.co.sym}
                onPointerEnter={() => setHot(t.co.sym)}
                initial={{ opacity: reduced ? 1 : 0 }}
                animate={{ opacity: hot && hot !== t.co.sym ? 0.45 : 1 }}
                transition={{ duration: reduced ? 0 : 0.35, delay: reduced ? 0 : i * 0.02 }}
              >
                <rect
                  x={t.x + 1}
                  y={t.y + 1}
                  width={Math.max(0, t.w - 2)}
                  height={Math.max(0, t.h - 2)}
                  rx={4}
                  fill={fill(chg)}
                  stroke={hot === t.co.sym ? 'var(--foreground)' : 'transparent'}
                />
                {t.w > 30 && (
                  <text
                    x={t.x + 7}
                    y={t.y + 17}
                    fontSize={big ? 12 : 9}
                    fontWeight={800}
                    fill="var(--foreground)"
                  >
                    {t.co.sym}
                  </text>
                )}
                {big && (
                  <text
                    x={t.x + 7}
                    y={t.y + 31}
                    fontSize={10}
                    fontWeight={600}
                    fill="var(--foreground)"
                    opacity={0.85}
                  >
                    {signed(t.co.chg)} %
                  </text>
                )}
              </motion.g>
            )
          })}
        </svg>
        {tile && (
          <div
            role="status"
            className="panel-raised pointer-events-none absolute z-10 w-40 rounded-xl px-3 py-2 text-[11px] tabular-nums"
            style={{
              left: Math.min(width - 164, Math.max(0, tile.x + tile.w / 2 - 80)),
              top: tile.y > 70 ? tile.y - 66 : Math.min(height - 62, tile.y + tile.h + 6),
            }}
          >
            <div className="flex justify-between font-bold">
              <span>{tile.co.sym}</span>
              <span className={Number(tile.co.chg) >= 0 ? 'text-up' : 'text-down'}>
                {signed(tile.co.chg)} %
              </span>
            </div>
            <div className="text-muted-foreground">{tile.co.name}</div>
            <div className="mt-1 flex justify-between text-muted-foreground">
              <span>US$ {decimal(tile.co.price)}</span>
              <span>{decimal(tile.co.cap)} B</span>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
