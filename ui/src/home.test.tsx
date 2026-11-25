/**
 * Inicio (#581, etapa 2): vista de entrada con vidrio, cuenta del motor y puertas a cada sección.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { decimal } from '@/lib/format'
import { PAPER_ACCOUNT } from '@/mocks'
import { DOORS } from '@/views/home-doors'

const heading = () => screen.getByRole('heading', { level: 1 })
const doors = () => screen.getByRole('navigation', { name: 'Secciones del inicio' })
const strip = () => screen.getByRole('region', { name: 'Cuenta de simulación' })

/** Gateway falso: responde solo las rutas que se le pasan; lo demás falla (y se ve el ejemplo). */
function gateway(routes: Record<string, unknown> = {}) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const path = new URL(String(input), 'http://localhost').pathname
      return path in routes
        ? new Response(JSON.stringify(routes[path]), { status: 200 })
        : new Response('sin gateway', { status: 503 })
    }),
  )
}

beforeEach(() => {
  localStorage.clear()
  history.replaceState(null, '', '/')
  gateway()
})
afterEach(() => {
  vi.unstubAllGlobals()
  localStorage.clear()
})

describe('Inicio como vista de entrada', () => {
  it('sin hash abre el Inicio, con vidrio y fondo animado', () => {
    const { container } = render(<App initialStage="app" />)
    expect(heading()).toHaveTextContent('Inicio')
    expect(container.firstElementChild).toHaveAttribute('data-glass', 'on')
    expect(container.querySelector('[data-wallpaper]')).not.toBeNull()
    expect(container.querySelector('[data-section="hub"]')).not.toBeNull()
  })

  it('las demás vistas no tienen vidrio ni fondo animado; los hashes viejos siguen', () => {
    history.replaceState(null, '', '/#terminal')
    const { container } = render(<App initialStage="app" />)
    expect(heading()).toHaveTextContent('Terminal')
    expect(container.firstElementChild).not.toHaveAttribute('data-glass')
    expect(container.querySelector('[data-wallpaper]')).toBeNull()

    act(() => {
      history.pushState(null, '', '/#panel')
      window.dispatchEvent(new HashChangeEvent('hashchange'))
    })
    expect(heading()).toHaveTextContent('Panel')
    expect(container.querySelector('[data-wallpaper]')).toBeNull()
  })

  it('el ingreso conserva el vidrio y el fondo', () => {
    const { container } = render(<App initialStage="signin" />)
    expect(container.firstElementChild).toHaveAttribute('data-glass', 'on')
    expect(container.querySelector('[data-wallpaper]')).not.toBeNull()
  })
})

describe('puertas a las secciones', () => {
  it('una por sección, cada una con su acento, su enlace y su atajo', () => {
    render(<App initialStage="app" />)
    const links = within(doors()).getAllByRole('link')
    expect(links.map((a) => a.querySelector('strong')?.textContent)).toEqual([
      'Terminal',
      'Gráficos',
      'Investigación',
      'Noticias',
      'Fondo',
      'Diario',
      'Competencia',
    ])
    expect(links.map((a) => a.getAttribute('href'))).toEqual(DOORS.map((d) => `#${d.view}`))
    expect(links.map((a) => a.getAttribute('data-section'))).toEqual([
      'terminal',
      'charts',
      'research',
      'news',
      'fund',
      'journal',
      'trophies',
    ])
    expect(links.map((a) => a.getAttribute('aria-keyshortcuts'))).toEqual(
      DOORS.map((d) => d.shortcut),
    )
  })

  it('un clic abre la sección y entra en el historial', () => {
    render(<App initialStage="app" />)
    const before = history.length
    fireEvent.click(within(doors()).getByRole('link', { name: /Diario/ }))
    expect(location.hash).toBe('#operaciones')
    expect(heading()).toHaveTextContent('Operaciones')
    expect(history.length).toBe(before + 1)
  })

  it('con el teclado: Tab llega a cada puerta y 1 a 7 abren la sección', () => {
    render(<App initialStage="app" />)
    const first = within(doors()).getAllByRole('link')[0]
    first.focus()
    expect(first).toHaveFocus()
    fireEvent.keyDown(window, { key: '3' })
    expect(location.hash).toBe('#performance')
    expect(heading()).toHaveTextContent('Performance')
    // fuera del Inicio, el atajo ya no existe
    fireEvent.keyDown(window, { key: '1' })
    expect(location.hash).toBe('#performance')
  })

  it('el atajo no salta mientras se escribe ni con modificadores', () => {
    render(<App initialStage="app" />)
    const input = document.createElement('input')
    screen.getByRole('main').append(input)
    fireEvent.keyDown(input, { key: '1' })
    fireEvent.keyDown(window, { key: '1', ctrlKey: true })
    expect(heading()).toHaveTextContent('Inicio')
  })

  it('ningún texto promete ganancias', () => {
    render(<App initialStage="app" />)
    expect(document.body).not.toHaveTextContent(/ganancia asegurada|garantizad|sin riesgo/i)
  })
})

describe('cuenta de simulación', () => {
  it('sin motor, la cuenta de ejemplo lleva el sello y el aviso', () => {
    render(<App initialStage="app" />)
    expect(within(strip()).getByText('EJEMPLO')).toBeInTheDocument()
    expect(screen.getByText(/Sin conexión con el motor: la cuenta es de ejemplo/)).toBeVisible()
  })

  it('con motor, muestra la cuenta tal como llega, sin sello', async () => {
    gateway({
      '/api/v1/paper/account': { ...PAPER_ACCOUNT, equity: '12345.67', open_positions: 3 },
    })
    render(<App initialStage="app" />)
    await waitFor(() => expect(strip()).toHaveTextContent(decimal('12345.67', 2)))
    expect(within(strip()).queryByText('EJEMPLO')).toBeNull()
    expect(within(doors()).getByRole('link', { name: /Terminal/ })).toHaveTextContent(
      '3 de 5 posiciones abiertas',
    )
  })
})
