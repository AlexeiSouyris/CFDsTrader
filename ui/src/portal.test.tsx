import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { PORTAL_MS, PortalTransition, REDUCED_MS } from '@/components/ui/portal-transition'

const overlay = () => screen.queryByTestId('portal-transition')

function signIn() {
  fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'a@b.co' } })
  fireEvent.change(screen.getByLabelText('Contraseña'), { target: { value: 'x' } })
  fireEvent.change(screen.getByLabelText('Código del segundo factor'), {
    target: { value: '123456' },
  })
  fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
}

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
})

describe('transición de entrada (#414)', () => {
  it('aparece solo al aceptarse el ingreso, sobre la app ya montada, y se va sola', () => {
    vi.useFakeTimers()
    render(<App initialStage="signin" />)
    expect(overlay()).not.toBeInTheDocument()
    signIn()
    expect(overlay()).toBeInTheDocument()
    // la app ya está montada debajo: no espera a que termine
    expect(screen.getByRole('navigation', { name: 'Vistas' })).toBeInTheDocument()
    act(() => vi.advanceTimersByTime(PORTAL_MS + 400))
    expect(overlay()).not.toBeInTheDocument()
  })

  it('no aparece al cambiar de vista', () => {
    vi.useFakeTimers()
    render(<App initialStage="signin" />)
    signIn()
    act(() => vi.advanceTimersByTime(PORTAL_MS + 400))
    act(() => {
      location.hash = '#graficos'
      window.dispatchEvent(new HashChangeEvent('hashchange'))
    })
    expect(overlay()).not.toBeInTheDocument()
  })

  it('se saltea con un clic o con una tecla', () => {
    render(<App initialStage="signin" />)
    signIn()
    fireEvent.pointerDown(overlay()!)
    expect(overlay()).not.toBeInTheDocument()

    const onDone = vi.fn()
    render(<PortalTransition onDone={onDone} />)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onDone).toHaveBeenCalledOnce()
  })

  it('con prefers-reduced-motion es un fundido corto, sin dibujar el túnel', () => {
    vi.useFakeTimers()
    vi.spyOn(window, 'matchMedia').mockImplementation(
      (q: string) =>
        ({
          matches: q.includes('reduce'),
          media: q,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
        }) as unknown as MediaQueryList,
    )
    const getContext = vi.spyOn(HTMLCanvasElement.prototype, 'getContext')
    getContext.mockClear()
    const onDone = vi.fn()
    render(<PortalTransition onDone={onDone} />)
    expect(getContext).not.toHaveBeenCalled()
    act(() => vi.advanceTimersByTime(REDUCED_MS + 300))
    expect(onDone).toHaveBeenCalledOnce()
  })

  it('al recargar con la app abierta no aparece', () => {
    render(<App initialStage="app" />)
    expect(overlay()).not.toBeInTheDocument()
  })

  it('no aparece cuando el ingreso se saltea con VITE_SKIP_SIGNIN', async () => {
    vi.stubEnv('VITE_SKIP_SIGNIN', '1')
    vi.resetModules()
    const { default: SkippedApp } = await import('@/App')
    render(<SkippedApp />)
    expect(screen.getByRole('navigation', { name: 'Vistas' })).toBeInTheDocument()
    expect(overlay()).not.toBeInTheDocument()
  })

  it('con VITE_PORTAL_PREVIEW sí aparece aunque se saltee el ingreso (para probarla)', async () => {
    vi.stubEnv('VITE_SKIP_SIGNIN', '1')
    vi.stubEnv('VITE_PORTAL_PREVIEW', '1')
    vi.resetModules()
    const { default: SkippedApp } = await import('@/App')
    render(<SkippedApp />)
    expect(overlay()).toBeInTheDocument()
  })
})
