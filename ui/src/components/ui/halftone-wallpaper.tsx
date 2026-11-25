/**
 * Fondo de la app: un flujo líquido en semitono (puntos), gris grafito con brillos de plata.
 *
 * Inspirado en "Halftone Flow" (21st.dev), reescrito (#164):
 * - sin iframe ni scripts de CDN: el shader vive acá y se dibuja en un <canvas>;
 * - más lento (la velocidad es un prop) y en grafito/plata metalizada en vez de rojo;
 * - con `prefers-reduced-motion` dibuja un solo cuadro y queda quieto;
 * - se pausa cuando la pestaña no se ve; sin WebGL, queda un degradé de CSS.
 */
import { useEffect, useRef, useState } from 'react'

import { cn } from '@/lib/utils'

const VERT = `
attribute vec2 p;
void main() { gl_Position = vec4(p, 0.0, 1.0); }
`

const FRAG = `
precision highp float;
uniform vec2 u_res;
uniform float u_time;
uniform float u_light;

mat2 rot(float a) { float s = sin(a), c = cos(a); return mat2(c, -s, s, c); }

void main() {
  vec2 uv = gl_FragCoord.xy / u_res;
  vec2 p = uv * 2.0 - 1.0;
  p.x *= u_res.x / u_res.y;

  vec2 f = p;
  float t = u_time;
  for (float i = 1.0; i < 4.0; i++) {
    f *= rot(t * 0.1);
    f.x += sin(f.y * 2.0 * i + t) * 0.5;
    f.y += cos(f.x * 1.5 * i - t * 0.8) * 0.5;
  }
  float k = sin(f.x * 2.0 + f.y * 3.0) * 0.5 + 0.5;

  // grafito -> acero -> plata, con un brillo especular fino (metalizado)
  vec3 graphite = vec3(0.13, 0.14, 0.155);
  vec3 steel = vec3(0.42, 0.44, 0.48);
  vec3 silver = vec3(0.86, 0.88, 0.92);
  vec3 col = mix(graphite, steel, smoothstep(0.25, 0.7, k));
  col = mix(col, silver, smoothstep(0.78, 1.0, k));
  col += vec3(0.9, 0.92, 0.96) * pow(k, 18.0) * 0.35;

  // semitono: un punto por celda, del tamaño de la intensidad
  vec2 cell = fract(gl_FragCoord.xy / 6.0) - 0.5;
  float dot_mask = smoothstep(k * 0.46, k * 0.46 - 0.12, length(cell));
  vec3 base = mix(vec3(0.045, 0.048, 0.054), vec3(0.86, 0.88, 0.91), u_light);
  vec3 outc = mix(base, col, dot_mask) + col * 0.10;
  gl_FragColor = vec4(outc, 1.0);
}
`

export function HalftoneWallpaper({
  speed = 0.12,
  light = false,
  className,
}: {
  /** velocidad del flujo; el original iba a 0.4 */
  speed?: number
  /** fondo claro (tema claro) en lugar de grafito */
  light?: boolean
  className?: string
}) {
  const ref = useRef<HTMLCanvasElement>(null)
  const [failed, setFailed] = useState(false)
  const lightRef = useRef(light)
  useEffect(() => {
    lightRef.current = light
  }, [light])

  useEffect(() => {
    const canvas = ref.current
    const gl = canvas?.getContext('webgl', { antialias: false, alpha: false })
    if (!canvas || !gl) {
      setFailed(true)
      return
    }
    const compile = (type: number, src: string) => {
      const s = gl.createShader(type)!
      gl.shaderSource(s, src)
      gl.compileShader(s)
      return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null
    }
    const vs = compile(gl.VERTEX_SHADER, VERT)
    const fs = compile(gl.FRAGMENT_SHADER, FRAG)
    const prog = gl.createProgram()
    if (!vs || !fs || !prog) {
      setFailed(true)
      return
    }
    gl.attachShader(prog, vs)
    gl.attachShader(prog, fs)
    gl.linkProgram(prog)
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      setFailed(true)
      return
    }
    gl.useProgram(prog)
    const buf = gl.createBuffer()
    gl.bindBuffer(gl.ARRAY_BUFFER, buf)
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW)
    const loc = gl.getAttribLocation(prog, 'p')
    gl.enableVertexAttribArray(loc)
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0)
    const uRes = gl.getUniformLocation(prog, 'u_res')
    const uTime = gl.getUniformLocation(prog, 'u_time')
    const uLight = gl.getUniformLocation(prog, 'u_light')

    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const resize = () => {
      // el semitono se ve igual a 1x: no vale la pena pagar el doble de píxeles
      canvas.width = Math.max(1, Math.round(canvas.clientWidth))
      canvas.height = Math.max(1, Math.round(canvas.clientHeight))
      gl.viewport(0, 0, canvas.width, canvas.height)
    }
    const ro = new ResizeObserver(resize)
    ro.observe(canvas)
    resize()

    let raf = 0
    const start = performance.now()
    const draw = () => {
      gl.uniform2f(uRes, canvas.width, canvas.height)
      gl.uniform1f(uTime, reduced ? 4 : ((performance.now() - start) / 1000) * speed)
      gl.uniform1f(uLight, lightRef.current ? 1 : 0)
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4)
      if (!reduced && !document.hidden) raf = requestAnimationFrame(draw)
    }
    const onVisible = () => {
      cancelAnimationFrame(raf)
      if (!document.hidden) raf = requestAnimationFrame(draw)
    }
    document.addEventListener('visibilitychange', onVisible)
    raf = requestAnimationFrame(draw)
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      document.removeEventListener('visibilitychange', onVisible)
      gl.deleteProgram(prog)
      gl.deleteBuffer(buf)
    }
  }, [speed])

  return (
    <div
      aria-hidden="true"
      data-wallpaper
      className={cn('pointer-events-none fixed inset-0 z-0', className)}
    >
      {failed ? (
        <div className="h-full w-full bg-[radial-gradient(120%_90%_at_30%_10%,#3a3e44_0%,#16181b_55%,#0c0d0f_100%)]" />
      ) : (
        <canvas ref={ref} className="h-full w-full" />
      )}
      {/* velo para que el vidrio y el texto siempre se lean sobre el fondo */}
      <div className="absolute inset-0 bg-[linear-gradient(180deg,rgb(8_9_11/0.5),rgb(8_9_11/0.72))]" />
    </div>
  )
}
