/**
 * Intro "El mercado espera" (#581, etapa 9): diálogo accesible, se saltea con el teclado o el
 * botón, se recuerda en este navegador y en los tests no aparece salvo que se pida.
 */
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import Intro, { INTRO_CAP_MS, INTRO_REDUCED_MS } from '@/components/ui/intro'
import { INTRO_KEY, introOnStart, introSeen, markIntroSeen } from '@/lib/intro-pref'

const reducedMotion = (on: boolean) =>
  vi.spyOn(window, 'matchMedia').mockImplementation(
    (query: string) =>
      ({
        matches: on && query.includes('reduced-motion'),
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }) as unknown as MediaQueryList,
  )

beforeEach(() => localStorage.clear())
afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
})

describe('intro "El mercado espera"', () => {
  it('es un diálogo con el foco en "Saltar" y una región que lee las frases', () => {
    render(<Intro onDone={() => {}} />)
    const dialog = screen.getByRole('dialog', { name: 'Introducción' })
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    expect(screen.getByRole('button', { name: 'Saltar (Esc)' })).toHaveFocus()
    const live = dialog.querySelector('[aria-live="polite"]')
    expect(live).toHaveTextContent('el mercado espera')
  })

  it.each(['Escape', 'Enter', ' '])('se saltea con %j', (key) => {
    const onDone = vi.fn()
    render(<Intro onDone={onDone} />)
    fireEvent.keyDown(window, { key })
    expect(onDone).toHaveBeenCalledTimes(1)
  })

  it('se saltea con el botón', () => {
    const onDone = vi.fn()
    render(<Intro onDone={onDone} />)
    fireEvent.click(screen.getByRole('button', { name: 'Saltar (Esc)' }))
    expect(onDone).toHaveBeenCalled()
  })

  it('termina sola antes del tope aunque no se corran los cuadros', () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const onDone = vi.fn()
    render(<Intro onDone={onDone} />)
    act(() => vi.advanceTimersByTime(INTRO_CAP_MS + 600))
    expect(onDone).toHaveBeenCalledTimes(1)
  })

  it('con movimiento reducido muestra la frase final y se funde en 1,2 s', () => {
    reducedMotion(true)
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const onDone = vi.fn()
    render(<Intro onDone={onDone} />)
    expect(screen.getByText('simulación, no promesas')).toBeInTheDocument()
    act(() => vi.advanceTimersByTime(INTRO_REDUCED_MS + 600))
    expect(onDone).toHaveBeenCalledTimes(1)
  })

  it('no muestra cifras de dinero ni promesas', () => {
    render(<Intro onDone={() => {}} />)
    const text = document.body.textContent ?? ''
    expect(text).not.toMatch(/[$€£]|\d+\s*%|ganancia|garantiza/i)
  })
})

describe('cuándo aparece', () => {
  it('en los tests no aparece', () => {
    expect(introOnStart()).toBe(false)
    render(<App />)
    expect(screen.queryByTestId('intro')).not.toBeInTheDocument()
  })

  it('se recuerda en este navegador', () => {
    expect(introSeen()).toBe(false)
    markIntroSeen()
    expect(localStorage.getItem(INTRO_KEY)).toBe('1')
    expect(introSeen()).toBe(true)
  })

  it('si el navegador no deja leer el almacenamiento, cuenta como vista', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('bloqueado')
    })
    expect(introSeen()).toBe(true)
  })

  it('con VITE_INTRO_PREVIEW aparece antes del ingreso y al saltarla sigue el ingreso', async () => {
    vi.stubEnv('VITE_INTRO_PREVIEW', '1')
    vi.resetModules()
    const { default: PreviewApp } = await import('@/App')
    render(<PreviewApp />)
    const skip = await screen.findByRole('button', { name: 'Saltar (Esc)' })
    expect(screen.queryByLabelText('Email')).not.toBeInTheDocument()
    fireEvent.click(skip)
    expect(await screen.findByLabelText('Email')).toBeInTheDocument()
    expect(localStorage.getItem(INTRO_KEY)).toBe('1')
  })

  it('se puede volver a ver desde la paleta (⌘K)', async () => {
    render(<App initialStage="app" />)
    fireEvent.keyDown(window, { key: 'k', metaKey: true })
    fireEvent.click(await screen.findByRole('option', { name: /Ver la intro otra vez/ }))
    expect(await screen.findByRole('dialog', { name: 'Introducción' })).toBeInTheDocument()
  })
})
