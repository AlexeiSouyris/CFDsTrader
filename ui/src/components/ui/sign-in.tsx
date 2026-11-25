/**
 * Ingreso. Inspirado en "Modern Stunning Sign In" (21st.dev), reescrito (#164):
 * - pide el código de 6 dígitos del segundo factor (spec §1.1: 2FA con TOTP);
 * - sin "Continuar con Google" ni imágenes de CDNs externos;
 * - es una maqueta: valida el formato y no envía nada a ningún lado.
 */
import { useId, useState, type FormEvent } from 'react'

import { CandlestickIcon } from '@/components/ui/animated-icons'

const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

export function SignIn({ onSignIn }: { onSignIn: () => void }) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [code, setCode] = useState('')
  const [error, setError] = useState('')
  const errorId = useId()
  const codeHint = useId()

  const submit = (e: FormEvent) => {
    e.preventDefault()
    if (!email || !password || !code)
      return setError('Completá el email, la contraseña y el código.')
    if (!EMAIL.test(email)) return setError('El email no tiene un formato válido.')
    if (!/^\d{6}$/.test(code)) return setError('El código del segundo factor tiene 6 dígitos.')
    setError('')
    onSignIn()
  }

  const field =
    'w-full rounded-xl border border-border bg-[color-mix(in_srgb,var(--card)_45%,transparent)] px-4 py-3 text-[14px] font-normal placeholder:text-muted-foreground focus:outline-2 focus:outline-ring'

  // etiquetas visibles (#579): el texto de ejemplo desaparece al escribir y no alcanza como nombre
  const labelCls = 'grid gap-1.5 text-[12px] font-semibold'

  return (
    <div className="grid min-h-svh place-items-center px-4">
      <form
        onSubmit={submit}
        noValidate
        aria-describedby={error ? errorId : undefined}
        className="glass w-full max-w-sm rounded-3xl p-8"
      >
        <span className="mx-auto mb-5 grid size-12 place-items-center rounded-full bg-primary text-primary-foreground">
          <CandlestickIcon size={24} playOnMount duration={1.6} />
        </span>
        <h1 className="text-center text-2xl font-bold tracking-tight">CFDsTrader</h1>
        <p className="mt-1 mb-6 text-center text-[12px] text-muted-foreground">
          Fondo del grupo · modo simulación
        </p>
        <div className="grid gap-3">
          <label className={labelCls}>
            Email
            <input
              className={field}
              type="email"
              autoComplete="username"
              placeholder="nombre@ejemplo.com"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </label>
          <label className={labelCls}>
            Contraseña
            <input
              className={field}
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
          <label className={labelCls}>
            Código del segundo factor
            <input
              className={`${field} tracking-[0.4em] placeholder:tracking-normal`}
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={6}
              placeholder="6 dígitos"
              aria-describedby={codeHint}
              value={code}
              onChange={(e) => setCode(e.target.value.replace(/\D/g, ''))}
            />
          </label>
          <span id={codeHint} className="-mt-1.5 text-[11px] text-muted-foreground">
            El de tu app de autenticación (2FA).
          </span>
          {error && (
            <p id={errorId} role="alert" className="text-[12px] text-down">
              {error}
            </p>
          )}
        </div>
        <button
          type="submit"
          className="mt-6 w-full rounded-full bg-primary py-3 text-[14px] font-bold text-primary-foreground hover:brightness-110"
        >
          Entrar
        </button>
        <p className="mt-4 text-center text-[11px] text-muted-foreground">
          Maqueta: no se conecta a ningún servidor. Las cuentas las crea el responsable del fondo.
        </p>
      </form>
    </div>
  )
}
