import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

afterEach(() => {
  cleanup()
  history.replaceState(null, '', '/')
})

// jsdom no trae estas APIs del navegador: se simulan lo mínimo para que los componentes monten.
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }),
})
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver
// sin WebGL en jsdom: el fondo usa su degradé de CSS
HTMLCanvasElement.prototype.getContext = vi.fn(() => null) as never
window.scrollTo = vi.fn() as never
