/**
 * Intro "El mercado espera" (#581, etapa 9; documento de diseño §4). Idea propia: el mercado solo
 * avanza cuando te movés. Un gráfico de velas ficticio y fijo, en grafito con un acento cian,
 * queda detenido hasta que la persona mueve el mouse, hace scroll, desliza el dedo o usa las
 * flechas; si en 4 s no hay movimiento, avanza solo, despacio.
 *
 * - Un solo `<canvas>` 2D con requestAnimationFrame; las frases son un `<p>` (con `aria-live`).
 * - Escuchas pasivas: el movimiento se acumula en un número y lo consume el cuadro.
 * - Dura 6–9 s; tope duro de 12 s. Se saltea con Esc, Enter, Espacio o "Saltar".
 * - El tiempo de la intro solo corre con la pestaña visible (cuenta los cuadros, no el reloj).
 * - Con `prefers-reduced-motion`: imagen quieta con la frase final y un fundido de 1,2 s.
 * - Sin destellos: solo cambia de a poco la opacidad del cursor de tiempo y el fundido final.
 *
 * Las velas son una ilustración: no son precios, no salen del motor y la intro no calcula nada.
 * No hay cifras de dinero ni promesas.
 */
import { useEffect, useRef, useState } from 'react'

/** Velas visibles al empezar y al terminar el recorrido. */
const FIRST = 30
const LAST = 66
/** Espera sin movimiento antes de avanzar sola, y velocidad sola (velas por segundo). */
const IDLE_MS = 4000
const AUTO_RATE = 12
/** Píxeles de movimiento por vela, velocidad máxima y lo máximo que se acumula. */
const PX_PER_CANDLE = 45
const MAX_RATE = 14
const MAX_PENDING = PX_PER_CANDLE * 2
/** Cierre (marca y fundido), tope total y fundido con movimiento reducido. */
export const CLOSE_MS = 900
export const INTRO_CAP_MS = 12000
export const INTRO_REDUCED_MS = 1200

const PHRASES = [
  'el tiempo avanza cuando vos avanzás',
  'cada vela, solo cuando cierra',
  'simulación, no promesas',
] as const
const WAITING = 'el mercado espera'

const BG = '#0a0a0b'
const GREY = '#8a8f96'
const DIM = '#4b5057'
const CYAN = '#7fd6e8'

type Candle = { o: number; h: number; l: number; c: number }

/** La serie fija y ficticia (siempre la misma): una suma de ondas, sin azar ni datos reales. */
function buildSeries(): Candle[] {
  const out: Candle[] = []
  let prev = 50
  for (let i = 0; i <= LAST; i++) {
    const c = 50 + 9 * Math.sin(i * 0.19) + 4 * Math.sin(i * 0.61 + 1) + 2 * Math.sin(i * 1.7)
    const wick = 0.8 + 1.4 * Math.abs(Math.sin(i * 2.3))
    out.push({ o: prev, c, h: Math.max(prev, c) + wick, l: Math.min(prev, c) - wick })
    prev = c
  }
  return out
}

const clamp01 = (x: number) => Math.min(1, Math.max(0, x))

function draw(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  series: Candle[],
  p: number,
  cursor: number,
  zoom: number,
) {
  ctx.setTransform(1, 0, 0, 1, 0, 0)
  ctx.fillStyle = BG
  ctx.fillRect(0, 0, w, h)
  ctx.setTransform(zoom, 0, 0, zoom, (w * (1 - zoom)) / 2, (h * (1 - zoom)) / 2)
  const lo = 30
  const hi = 70
  const top = h * 0.24
  const bottom = h * 0.7
  const y = (v: number) => bottom - ((v - lo) / (hi - lo)) * (bottom - top)
  const step = Math.max(8, Math.min(26, w / 46))
  // la cámara sigue a la vela que se forma, a dos tercios del ancho
  const x0 = w * 0.66 - p * step
  const k = Math.floor(p)
  const f = p - k
  for (let i = Math.max(0, Math.floor(-x0 / step) - 1); i <= Math.min(k, LAST); i++) {
    const x = x0 + i * step
    const s = series[i]
    const forming = i === k && i < LAST
    const c = forming ? s.o + (s.c - s.o) * f : s.c
    const up = c >= s.o
    const color = forming ? CYAN : up ? GREY : DIM
    ctx.strokeStyle = color
    ctx.fillStyle = color
    ctx.lineWidth = 1
    ctx.beginPath()
    ctx.moveTo(Math.round(x) + 0.5, y(forming ? Math.max(s.o, c) + 0.6 : s.h))
    ctx.lineTo(Math.round(x) + 0.5, y(forming ? Math.min(s.o, c) - 0.6 : s.l))
    ctx.stroke()
    const bw = step * 0.56
    const by = y(Math.max(s.o, c))
    const bh = Math.max(1, Math.abs(y(s.o) - y(c)))
    if (up && !forming) ctx.strokeRect(x - bw / 2 + 0.5, by + 0.5, bw - 1, bh - 1)
    else ctx.fillRect(x - bw / 2, by, bw, bh)
  }
  // cursor de tiempo: una línea punteada en la vela que se forma
  if (cursor > 0) {
    const x = Math.round(x0 + (k + 0.6) * step) + 0.5
    ctx.globalAlpha = cursor
    ctx.strokeStyle = CYAN
    ctx.setLineDash([3, 5])
    ctx.beginPath()
    ctx.moveTo(x, top - 20)
    ctx.lineTo(x, bottom + 20)
    ctx.stroke()
    ctx.setLineDash([])
    ctx.globalAlpha = 1
  }
}

const prefersReducedMotion = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true

export default function Intro({ onDone }: { onDone: () => void }) {
  const rootRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const brandRef = useRef<HTMLDivElement>(null)
  const phraseRef = useRef<HTMLParagraphElement>(null)
  const onDoneRef = useRef(onDone)
  const [reduced] = useState(prefersReducedMotion)
  const [phrase, setPhrase] = useState(reduced ? 2 : -1)
  useEffect(() => {
    onDoneRef.current = onDone
  }, [onDone])
  useEffect(() => {
    phraseRef.current?.animate?.([{ opacity: 0 }, { opacity: 1 }], { duration: 400 })
  }, [phrase])

  useEffect(() => {
    let finished = false
    let raf = 0
    const finish = () => {
      if (finished) return
      finished = true
      cancelAnimationFrame(raf)
      onDoneRef.current()
    }
    const canvas = canvasRef.current
    const ctx = canvas?.getContext('2d') ?? null
    const series = buildSeries()
    let w = 1
    let h = 1
    const resize = () => {
      if (!canvas) return
      const dpr = Math.min(2, window.devicePixelRatio || 1)
      w = canvas.width = Math.max(1, Math.round(window.innerWidth * dpr))
      h = canvas.height = Math.max(1, Math.round(window.innerHeight * dpr))
    }
    resize()
    window.addEventListener('resize', resize)
    // la página de abajo no se desplaza mientras corre la intro
    const root = document.documentElement
    const overflow = root.style.overflow
    root.style.overflow = 'hidden'

    // estado del recorrido (fuera de React: el cuadro lo lee y lo escribe)
    let p = reduced ? LAST : FIRST
    let pending = 0
    let elapsed = 0
    let idleSince = 0
    let closingAt = -1
    let shown = reduced ? 2 : -1
    let last = 0
    const total = reduced ? INTRO_REDUCED_MS : INTRO_CAP_MS
    const move = (px: number) => {
      pending = Math.min(MAX_PENDING, pending + px)
      idleSince = elapsed
    }

    const frame = (now: number) => {
      const dt = last ? Math.min(50, now - last) : 16
      last = now
      elapsed += dt
      if (!reduced && closingAt < 0) {
        const take = Math.min(pending, (MAX_RATE * PX_PER_CANDLE * dt) / 1000)
        pending -= take
        p += take / PX_PER_CANDLE
        if (elapsed - idleSince >= IDLE_MS) p += (AUTO_RATE * dt) / 1000
        if (p >= LAST || elapsed >= INTRO_CAP_MS - CLOSE_MS) {
          p = LAST
          closingAt = elapsed
        }
      }
      const q = closingAt < 0 ? 0 : clamp01((elapsed - closingAt) / CLOSE_MS)
      const next = reduced
        ? 2
        : p <= FIRST
          ? -1
          : Math.min(2, Math.floor(((p - FIRST) / (LAST - FIRST)) * 3))
      if (next !== shown) setPhrase((shown = next))
      // el cursor "respira" despacio cuando está quieto (menos de una vez por segundo)
      const still = pending <= 0 && elapsed - idleSince < IDLE_MS
      const cursor = closingAt >= 0 ? 0 : still ? 0.45 + 0.3 * Math.sin(elapsed / 300) : 0.8
      if (ctx) draw(ctx, w, h, series, p, reduced ? 0 : cursor, 1 - 0.08 * q)
      if (brandRef.current) brandRef.current.style.opacity = String(clamp01(q * 3))
      const veil = reduced
        ? 1 - clamp01((elapsed - total / 2) / (total / 2))
        : 1 - clamp01((q - 0.33) / 0.67)
      if (rootRef.current) rootRef.current.style.opacity = String(veil)
      if ((reduced && elapsed >= total) || q >= 1) return finish()
      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)

    // respaldo si el navegador no corre los cuadros; se pausa con la pestaña oculta
    let safety = 0
    const arm = () => {
      window.clearTimeout(safety)
      if (!document.hidden) safety = window.setTimeout(finish, total - elapsed + 500)
    }
    arm()
    const onVisible = () => {
      last = 0
      arm()
    }
    let px = -1
    let py = -1
    const onPointer = (e: PointerEvent) => {
      if (px >= 0) move(Math.abs(e.clientX - px) + Math.abs(e.clientY - py))
      px = e.clientX
      py = e.clientY
    }
    const onWheel = (e: WheelEvent) => move(Math.abs(e.deltaX) + Math.abs(e.deltaY))
    let ty = -1
    let tx = -1
    const onTouch = (e: TouchEvent) => {
      const t = e.touches[0]
      if (!t) return
      if (tx >= 0) move(Math.abs(t.clientX - tx) + Math.abs(t.clientY - ty))
      tx = t.clientX
      ty = t.clientY
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault()
        finish()
      } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
        e.preventDefault()
        move(PX_PER_CANDLE * 1.5)
      } else if (e.key === 'Tab') e.preventDefault() // el foco se queda en "Saltar"
    }
    const passive = { passive: true }
    window.addEventListener('pointermove', onPointer, passive)
    window.addEventListener('wheel', onWheel, passive)
    window.addEventListener('touchmove', onTouch, passive)
    window.addEventListener('keydown', onKey)
    document.addEventListener('visibilitychange', onVisible)
    return () => {
      finished = true
      cancelAnimationFrame(raf)
      window.clearTimeout(safety)
      window.removeEventListener('resize', resize)
      root.style.overflow = overflow
      window.removeEventListener('pointermove', onPointer)
      window.removeEventListener('wheel', onWheel)
      window.removeEventListener('touchmove', onTouch)
      window.removeEventListener('keydown', onKey)
      document.removeEventListener('visibilitychange', onVisible)
    }
  }, [reduced])

  return (
    <div
      ref={rootRef}
      role="dialog"
      aria-modal="true"
      aria-label="Introducción"
      aria-describedby="intro-resumen"
      data-testid="intro"
      className="fixed inset-0 z-[100] overflow-hidden text-[#e8e8e8] select-none will-change-[opacity]"
      style={{ background: BG, touchAction: 'none' }}
    >
      <canvas ref={canvasRef} aria-hidden="true" className="absolute inset-0 block size-full" />
      <p id="intro-resumen" className="sr-only">
        Un gráfico de velas ficticio que avanza cuando te movés. Podés saltar la introducción.
      </p>
      <button
        type="button"
        autoFocus
        onClick={() => onDoneRef.current()}
        className="absolute top-[max(1rem,env(safe-area-inset-top))] right-4 min-h-11 rounded-full px-4 text-[12px] font-semibold outline-offset-2 focus-visible:outline-2 focus-visible:outline-[#7fd6e8]"
        style={{ color: GREY }}
      >
        Saltar (Esc)
      </button>
      <p
        ref={phraseRef}
        aria-live="polite"
        className="absolute inset-x-4 bottom-[18%] text-center text-[clamp(14px,2.2vw,20px)] tracking-[0.02em]"
        style={{ fontFamily: 'var(--font-mono)', color: phrase < 0 ? GREY : '#e8e8e8' }}
      >
        {phrase < 0 ? WAITING : PHRASES[phrase]}
      </p>
      <div
        ref={brandRef}
        aria-hidden="true"
        className="absolute inset-x-0 top-[9%] text-center opacity-0"
      >
        <strong className="block text-[13px] font-extrabold tracking-[0.3em] uppercase">
          CFDsTrader
        </strong>
        <small className="mt-1 block text-[11px]" style={{ color: CYAN }}>
          Simulación (paper)
        </small>
      </div>
    </div>
  )
}
