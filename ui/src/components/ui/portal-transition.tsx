/**
 * Transición de entrada después del login (#414): la cámara avanza por un túnel de marcos
 * rectangulares blancos sobre negro, los atraviesa acelerando, hay un destello blanco breve al
 * cruzar el último y todo se funde con la app, que ya está montada debajo (no espera datos).
 *
 * Efecto propio, escrito desde cero (sin código, nombres ni assets de terceros). La idea de
 * proyectar marcos a distintas profundidades con una perspectiva simple (tamaño ∝ 1 / distancia)
 * es la de los túneles de cuadrados en canvas y la del eje Z de CSS 3D; ver el PR de #414.
 *
 * - Canvas 2D con requestAnimationFrame: por cuadro solo se redibuja el canvas y se cambia la
 *   `opacity` de dos capas; nada de layout por cuadro.
 * - Dura ~1,5 s. Un clic, un toque o cualquier tecla la saltean.
 * - Con `prefers-reduced-motion`, un fundido simple de negro a la app.
 * - Si el navegador no dibuja (pestaña oculta, sin canvas), un temporizador la termina igual.
 */
import { useEffect, useRef } from 'react'

/** Duración del recorrido por el túnel, hasta cruzar el último marco. */
export const TUNNEL_MS = 1150
/** Duración total (recorrido + destello + fundido a la app). */
export const PORTAL_MS = 1500
/** Duración del fundido simple con `prefers-reduced-motion`. */
export const REDUCED_MS = 400

const FRAMES = 13
/** Margen del temporizador de respaldo, por si el navegador no corre los cuadros. */
const SAFETY_MS = 250

type Frame = { z: number; ox: number; oy: number; rot: number; scale: number }

/**
 * Marcos del túnel: separados de forma irregular en profundidad, con un leve desfase lateral y
 * una rotación mínima. Determinístico (sin aleatorio): siempre el mismo túnel.
 */
function buildFrames(): Frame[] {
  const frames: Frame[] = []
  let z = 2.2
  for (let i = 0; i < FRAMES; i++) {
    frames.push({
      z,
      ox: Math.sin(i * 1.7 + 0.3) * 0.05,
      oy: Math.cos(i * 2.3 + 0.9) * 0.04,
      rot: Math.sin(i * 0.9) * 0.012,
      scale: 0.86 + 0.06 * Math.sin(i * 1.3),
    })
    z += 1.35 + 0.45 * Math.abs(Math.sin(i * 2.1))
  }
  return frames
}

const clamp01 = (x: number) => Math.min(1, Math.max(0, x))

function drawTunnel(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  dpr: number,
  frames: Frame[],
  t: number,
) {
  const p = clamp01(t / TUNNEL_MS)
  // la cámara arranca casi quieta y acelera hasta cruzar el último marco
  const camZ = frames[frames.length - 1].z * Math.pow(p, 2.3)
  const cx = w / 2
  const cy = h / 2
  const k = h / 2 // a distancia 1, un marco de alto 2 ocupa toda la pantalla
  const aspect = w / h

  ctx.setTransform(1, 0, 0, 1, 0, 0)
  ctx.fillStyle = '#000'
  ctx.fillRect(0, 0, w, h)
  ctx.strokeStyle = '#fff'

  // del más lejano al más cercano
  for (let i = frames.length - 1; i >= 0; i--) {
    const f = frames[i]
    const dz = f.z - camZ
    if (dz <= 0.04) continue
    const s = k / dz
    const hw = aspect * f.scale * s
    const hh = f.scale * s
    const alpha = clamp01(1.15 - dz / 20) * clamp01(dz / 0.35)
    if (alpha <= 0.01) continue
    ctx.globalAlpha = alpha
    ctx.lineWidth = Math.min(2.4, Math.max(1, 2.2 / dz)) * dpr
    ctx.setTransform(
      Math.cos(f.rot),
      Math.sin(f.rot),
      -Math.sin(f.rot),
      Math.cos(f.rot),
      cx + f.ox * s * aspect,
      cy + f.oy * s,
    )
    ctx.strokeRect(-hw, -hh, hw * 2, hh * 2)
  }
  ctx.globalAlpha = 1
}

/** Opacidad del destello blanco: sube justo antes de cruzar el último marco y se apaga. */
function flashAt(t: number): number {
  const rise = TUNNEL_MS - 110
  if (t < rise) return 0
  if (t < TUNNEL_MS) return 0.9 * ((t - rise) / 110)
  return 0.9 * (1 - clamp01((t - TUNNEL_MS) / (PORTAL_MS - TUNNEL_MS)))
}

/** Opacidad de toda la capa: plena durante el túnel y se funde con la app después. */
function veilAt(t: number, reduced: boolean): number {
  if (reduced) return 1 - clamp01(t / REDUCED_MS)
  if (t < TUNNEL_MS) return 1
  const q = clamp01((t - TUNNEL_MS) / (PORTAL_MS - TUNNEL_MS))
  return 1 - q * q * (3 - 2 * q)
}

const prefersReducedMotion = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true

export function PortalTransition({ onDone }: { onDone: () => void }) {
  const rootRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const flashRef = useRef<HTMLDivElement>(null)
  const onDoneRef = useRef(onDone)
  useEffect(() => {
    onDoneRef.current = onDone
  }, [onDone])

  useEffect(() => {
    const reduced = prefersReducedMotion()
    const total = reduced ? REDUCED_MS : PORTAL_MS
    let finished = false
    let raf = 0
    const finish = () => {
      if (finished) return
      finished = true
      cancelAnimationFrame(raf)
      onDoneRef.current()
    }

    const canvas = canvasRef.current
    const ctx = !reduced && canvas ? canvas.getContext('2d') : null
    const frames = buildFrames()
    let w = 0
    let h = 0
    let dpr = 1
    const resize = () => {
      if (!canvas) return
      dpr = Math.min(2, window.devicePixelRatio || 1)
      w = Math.max(1, Math.round(window.innerWidth * dpr))
      h = Math.max(1, Math.round(window.innerHeight * dpr))
      canvas.width = w
      canvas.height = h
    }
    if (ctx) {
      resize()
      window.addEventListener('resize', resize)
    }

    const start = performance.now()
    const step = (now: number) => {
      const t = now - start
      if (ctx) drawTunnel(ctx, w, h, dpr, frames, t)
      if (rootRef.current) rootRef.current.style.opacity = String(veilAt(t, reduced))
      if (flashRef.current) flashRef.current.style.opacity = reduced ? '0' : String(flashAt(t))
      if (t >= total) return finish()
      raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    const safety = window.setTimeout(finish, total + SAFETY_MS)

    // cualquier tecla la saltea (la escucha es de la ventana: el foco puede estar en cualquier lado)
    const onKey = () => finish()
    window.addEventListener('keydown', onKey)

    return () => {
      finished = true
      cancelAnimationFrame(raf)
      window.clearTimeout(safety)
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('resize', resize)
    }
  }, [])

  return (
    <div
      ref={rootRef}
      data-testid="portal-transition"
      aria-hidden="true"
      onPointerDown={() => onDoneRef.current()}
      className="fixed inset-0 z-[100] cursor-pointer bg-black will-change-[opacity]"
    >
      <canvas ref={canvasRef} className="absolute inset-0 block h-full w-full" />
      <div
        ref={flashRef}
        className="pointer-events-none absolute inset-0 bg-white opacity-0 will-change-[opacity]"
      />
    </div>
  )
}
