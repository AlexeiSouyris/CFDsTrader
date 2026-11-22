/**
 * Íconos animados de vela japonesa y monedas.
 *
 * Basados en "chart-candlestick" y "coins" de AnimateIcons / Lucide (https://animateicons.in),
 * de Avijit Dey (@avijit07x). Licencia MIT: https://github.com/Avijit07x/animateicons
 * Adaptados (#164): los dos originales repetían la misma lógica de hover; acá vive una sola
 * vez en `useHoverAnimation`, y respetan `prefers-reduced-motion`.
 */
import { motion, useAnimation, useReducedMotion, type Variants } from 'motion/react'
import { useEffect, type HTMLAttributes } from 'react'

import { cn } from '@/lib/utils'

type IconProps = Omit<
  HTMLAttributes<HTMLSpanElement>,
  'onAnimationStart' | 'onAnimationEnd' | 'onDrag' | 'onDragStart' | 'onDragEnd'
> & {
  size?: number
  /** multiplicador de la duración: más alto, más lento */
  duration?: number
  /** anima una vez al aparecer, además de al pasar el puntero */
  playOnMount?: boolean
}

function useHoverAnimation(playOnMount: boolean) {
  const controls = useAnimation()
  const reduced = useReducedMotion()
  const play = () => {
    if (!reduced) void controls.start('animate')
  }
  // Animar al montar desde el estado normal: arrancar en 'animate' dejaba el ícono vacío.
  useEffect(() => {
    if (playOnMount && !reduced) void controls.start('animate')
  }, [controls, playOnMount, reduced])
  return {
    controls,
    initial: 'normal',
    onMouseEnter: play,
    onMouseLeave: () => void controls.start('normal'),
  }
}

function Frame({
  size,
  className,
  children,
  hover,
  ...props
}: IconProps & { hover: ReturnType<typeof useHoverAnimation>; children: React.ReactNode }) {
  return (
    <span
      className={cn('inline-flex items-center justify-center', className)}
      onMouseEnter={hover.onMouseEnter}
      onMouseLeave={hover.onMouseLeave}
      {...props}
    >
      <motion.svg
        xmlns="http://www.w3.org/2000/svg"
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        animate={hover.controls}
        initial={hover.initial}
      >
        {children}
      </motion.svg>
    </span>
  )
}

export function CandlestickIcon({
  size = 24,
  duration = 1,
  playOnMount = false,
  ...props
}: IconProps) {
  const hover = useHoverAnimation(playOnMount)
  const axis: Variants = {
    normal: { pathLength: 1 },
    animate: { pathLength: [0, 1], transition: { duration: 0.4 * duration, ease: 'easeOut' } },
  }
  const body: Variants = {
    normal: { scaleY: 1 },
    animate: (i: number) => ({
      scaleY: [0, 1],
      transition: {
        delay: (0.3 + i * 0.16) * duration,
        duration: 0.45 * duration,
        ease: [0.34, 1.4, 0.64, 1],
      },
    }),
  }
  const wick: Variants = {
    normal: { pathLength: 1, opacity: 1 },
    animate: (i: number) => ({
      pathLength: [0, 1],
      opacity: [0, 1],
      transition: { delay: (0.42 + i * 0.16) * duration, duration: 0.35 * duration },
    }),
  }
  return (
    <Frame size={size} hover={hover} {...props}>
      <motion.path d="M9 5v4" custom={0} variants={wick} />
      <motion.rect
        width="4"
        height="6"
        x="7"
        y="9"
        rx="1"
        custom={0}
        variants={body}
        style={{ transformBox: 'view-box', originX: '9px', originY: '12px' }}
      />
      <motion.path d="M9 15v2" custom={0} variants={wick} />
      <motion.path d="M17 3v2" custom={1} variants={wick} />
      <motion.rect
        width="4"
        height="8"
        x="15"
        y="5"
        rx="1"
        custom={1}
        variants={body}
        style={{ transformBox: 'view-box', originX: '17px', originY: '9px' }}
      />
      <motion.path d="M17 13v3" custom={1} variants={wick} />
      <motion.path d="M3 3v16a2 2 0 0 0 2 2h16" variants={axis} />
    </Frame>
  )
}

export function CoinsIcon({ size = 24, duration = 1, playOnMount = false, ...props }: IconProps) {
  const hover = useHoverAnimation(playOnMount)
  const body: Variants = {
    normal: { pathLength: 1, opacity: 1 },
    animate: (i: number) => ({
      pathLength: [0, 1],
      opacity: [0, 1],
      transition: { duration: 0.5 * duration, delay: i * 0.08 * duration, ease: [0.16, 1, 0.3, 1] },
    }),
  }
  const coin: Variants = {
    normal: { scale: 1, opacity: 1 },
    animate: (i: number) => ({
      scale: [0, 1.12, 1],
      opacity: [0, 1, 1],
      transition: {
        duration: 0.45 * duration,
        delay: (0.5 + i * 0.06) * duration,
        times: [0, 0.6, 1],
      },
    }),
  }
  return (
    <Frame size={size} hover={hover} {...props}>
      <motion.circle cx="8" cy="8" r="6" custom={0} variants={body} />
      <motion.path d="M7 6h1v4" custom={1} variants={body} />
      <motion.path
        d="M18.09 10.37A6 6 0 1 1 10.34 18"
        custom={0}
        variants={coin}
        style={{ transformBox: 'view-box', originX: '14px', originY: '14px' }}
      />
      <motion.path
        d="m16.71 13.88.7.71-2.82 2.82"
        custom={1}
        variants={coin}
        style={{ transformBox: 'view-box', originX: '15px', originY: '15px' }}
      />
    </Frame>
  )
}
