/**
 * Trofeo de la competencia interna (simulación): una tarjeta holográfica que se inclina con el
 * puntero y se da vuelta con un clic.
 *
 * Une dos componentes de referencia que hacían lo mismo (#164): "Holographic Foil Card" y
 * "Holo Card". Queda una sola tarjeta con dos acabados: plata (trofeo del día) y oro (trofeo de
 * la semana). Como en "Holo Card", el brillo depende de la inclinación, no de un temporizador:
 * si la tarjeta está quieta, el brillo también.
 */
import {
  motion,
  useMotionTemplate,
  useMotionValue,
  useReducedMotion,
  useSpring,
  useTransform,
} from 'motion/react'
import { Trophy as TrophyIcon } from 'lucide-react'
import { useRef, useState, type PointerEvent } from 'react'

import { cn } from '@/lib/utils'
import type { Trophy } from '@/mocks'

const FINISH = {
  día: {
    name: 'Trofeo del día',
    rim: 'from-zinc-100 via-zinc-400 to-zinc-700',
    foil: ['rgba(200,220,255,0.40)', 'rgba(255,255,255,0.32)', 'rgba(170,190,230,0.30)'],
  },
  semana: {
    name: 'Trofeo de la semana',
    rim: 'from-amber-100 via-amber-400 to-amber-800',
    foil: ['rgba(255,214,120,0.45)', 'rgba(255,246,210,0.35)', 'rgba(220,150,60,0.35)'],
  },
} as const

export function TrophyCard({ trophy, className }: { trophy: Trophy; className?: string }) {
  const reduced = useReducedMotion()
  const ref = useRef<HTMLButtonElement>(null)
  const [flipped, setFlipped] = useState(false)
  const mx = useMotionValue(0)
  const my = useMotionValue(0)
  const spring = { stiffness: 170, damping: 18, mass: 0.6 }
  const sx = useSpring(mx, spring)
  const sy = useSpring(my, spring)
  const rotateY = useTransform(sx, [-0.5, 0.5], [-16, 16])
  const rotateX = useTransform(sy, [-0.5, 0.5], [14, -14])
  const angle = useTransform(sx, (v) => 120 + v * 90)
  const px = useTransform(sx, (v) => `${(v + 0.5) * 100}%`)
  const py = useTransform(sy, (v) => `${(v + 0.5) * 100}%`)
  const f = FINISH[trophy.period]
  const foil = useMotionTemplate`linear-gradient(${angle}deg, transparent 18%, ${f.foil[0]} 42%, ${f.foil[1]} 55%, ${f.foil[2]} 68%, transparent 86%), radial-gradient(circle at ${px} ${py}, rgba(255,255,255,0.5), transparent 40%)`

  const onMove = (e: PointerEvent) => {
    if (reduced || !ref.current) return
    const r = ref.current.getBoundingClientRect()
    mx.set((e.clientX - r.left) / r.width - 0.5)
    my.set((e.clientY - r.top) / r.height - 0.5)
  }
  const reset = () => {
    mx.set(0)
    my.set(0)
  }

  return (
    <div className={cn('[perspective:1100px]', className)}>
      <motion.button
        ref={ref}
        type="button"
        aria-pressed={flipped}
        aria-label={`${f.name}: ${trophy.winner.name}, ${trophy.result}. ${flipped ? 'Ver el frente' : 'Ver el detalle'}`}
        onPointerMove={onMove}
        onPointerLeave={reset}
        onClick={() => setFlipped((v) => !v)}
        style={{ rotateX, rotateY, transformStyle: 'preserve-3d' }}
        className="relative aspect-[5/7] w-[min(260px,70vw)] cursor-pointer rounded-[22px] text-left"
      >
        <motion.div
          animate={{ rotateY: flipped ? 180 : 0 }}
          transition={{ duration: reduced ? 0 : 0.6, ease: [0.16, 1, 0.3, 1] }}
          style={{ transformStyle: 'preserve-3d' }}
          className="absolute inset-0"
        >
          {/* frente */}
          <div
            className={cn(
              'absolute inset-0 rounded-[22px] bg-gradient-to-br p-[2px] shadow-[0_30px_70px_-20px_rgba(0,0,0,0.65)] [backface-visibility:hidden]',
              f.rim,
            )}
          >
            <div className="relative h-full overflow-hidden rounded-[20px] bg-[#111216] text-white">
              <motion.div
                className="absolute inset-0 opacity-80 mix-blend-color-dodge"
                style={{ backgroundImage: foil }}
              />
              <div className="absolute inset-0 opacity-25 mix-blend-overlay [background:repeating-conic-gradient(from_0deg,transparent_0deg_8deg,rgba(255,255,255,0.18)_8deg_9deg)]" />
              <div className="relative z-10 flex h-full flex-col justify-between p-5">
                <div className="flex justify-between text-[10px] font-bold tracking-[0.22em] text-white/70 uppercase">
                  <span>{f.name}</span>
                  <span>{trophy.serial}</span>
                </div>
                <div>
                  <div className="mb-4 grid aspect-[4/3] place-items-center rounded-xl border border-white/15 bg-white/5">
                    <TrophyIcon
                      className="size-16 text-white/85"
                      strokeWidth={1.2}
                      aria-hidden="true"
                    />
                  </div>
                  <p className="text-[26px] leading-none font-extrabold tracking-tight">
                    {trophy.winner.name}
                  </p>
                  <p className="mt-2 text-[11px] font-bold tracking-[0.2em] text-white/70 uppercase">
                    {trophy.result} · simulación
                  </p>
                </div>
                <div className="flex justify-between text-[10px] text-white/55">
                  <span>{trophy.date}</span>
                  <span>Tocá para dar vuelta</span>
                </div>
              </div>
            </div>
          </div>
          {/* dorso */}
          <div className="absolute inset-0 grid place-items-center rounded-[22px] bg-[radial-gradient(120%_90%_at_50%_20%,#2a2d33_0%,#121316_60%,#0a0b0d_100%)] p-6 text-center text-white [backface-visibility:hidden] [transform:rotateY(180deg)]">
            <div>
              <p className="text-[11px] font-bold tracking-[0.2em] text-white/60 uppercase">
                Cómo se gana
              </p>
              <p className="mt-3 text-[13px] leading-relaxed text-white/85">
                Regla de ejemplo, la define el grupo: mejor resultado en R de las propuestas en
                simulación del período, con todos los costos incluidos.
              </p>
              <p className="mt-3 text-[11px] text-white/60">
                Es un juego del grupo: no es dinero real ni una promesa de ganancia.
              </p>
            </div>
          </div>
        </motion.div>
      </motion.button>
    </div>
  )
}
