/**
 * Inicio (#581, etapa 2): vista de entrada con vidrio, cuenta del motor, puertas a cada sección,
 * selector de estrategia que solo filtra y resultados recientes tal como llegan.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '@/App'
import { decimal } from '@/lib/format'
import { parseCsv, parseRegistry, REGISTRY } from '@/lib/results-registry'
import { BACKTEST_RUNS, PAPER_ACCOUNT } from '@/mocks'
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
  it('sin hash abre el Inicio, sin vidrio ni fondo animado (#708)', () => {
    const { container } = render(<App initialStage="app" />)
    expect(heading()).toHaveTextContent('Inicio')
    expect(container.querySelector('[data-glass]')).toBeNull()
    expect(container.querySelector('[data-wallpaper]')).toBeNull()
    expect(container.querySelector('[data-section="hub"]')).not.toBeNull()
  })

  it('las demás vistas tampoco; los hashes viejos siguen', () => {
    history.replaceState(null, '', '/#terminal')
    const { container } = render(<App initialStage="app" />)
    expect(heading()).toHaveTextContent('Terminal')
    expect(container.querySelector('[data-glass]')).toBeNull()

    act(() => {
      history.pushState(null, '', '/#panel')
      window.dispatchEvent(new HashChangeEvent('hashchange'))
    })
    expect(heading()).toHaveTextContent('Panel')
    expect(container.querySelector('[data-wallpaper]')).toBeNull()
  })

  it('el ingreso tampoco tiene vidrio ni fondo animado', () => {
    const { container } = render(<App initialStage="signin" />)
    expect(container.querySelector('[data-glass]')).toBeNull()
    expect(container.querySelector('[data-wallpaper]')).toBeNull()
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
      'Trofeos',
      'Academia',
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
      'hub',
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
    expect(heading()).toHaveTextContent('Diario')
    expect(history.length).toBe(before + 1)
  })

  it('con el teclado: Tab llega a cada puerta y 1 a 7 abren la sección', () => {
    render(<App initialStage="app" />)
    const first = within(doors()).getAllByRole('link')[0]
    first.focus()
    expect(first).toHaveFocus()
    fireEvent.keyDown(window, { key: '3' })
    expect(location.hash).toBe('#investigacion')
    expect(heading()).toHaveTextContent('Investigación')
    // fuera del Inicio, el atajo ya no existe
    fireEvent.keyDown(window, { key: '1' })
    expect(location.hash).toBe('#investigacion')
  })

  it('el atajo no salta mientras se usa un control ni con modificadores', () => {
    render(<App initialStage="app" />)
    const radio = screen.getByRole('radio', { name: 'Todas' })
    fireEvent.keyDown(radio, { key: '1' })
    fireEvent.keyDown(window, { key: '1', ctrlKey: true })
    expect(heading()).toHaveTextContent('Inicio')
  })
})

describe('espacio de trabajo (#708)', () => {
  it('paneles de mercados, noticias y calendario, resultados y secciones, con la vista completa', () => {
    render(<App initialStage="app" />)
    expect(screen.getByRole('region', { name: 'Mercados' })).toBeInTheDocument()
    const news = screen.getByRole('region', { name: 'Noticias y calendario' })
    expect(within(news).getByRole('link', { name: /Noticias/ })).toHaveAttribute(
      'aria-keyshortcuts',
      'F6',
    )
    expect(screen.getByRole('region', { name: 'Resultados recientes' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: 'Secciones' })).toContainElement(doors())
  })

  it('sin gateway, noticias y calendario lo dicen: sin ejemplo inventado', async () => {
    render(<App initialStage="app" />)
    const news = screen.getByRole('region', { name: 'Noticias y calendario' })
    expect(
      await within(news).findByText(/Sin conexión con el gateway: no hay noticias/),
    ).toBeInTheDocument()
  })

  it('elegir un mercado abre la Terminal', async () => {
    render(<App initialStage="app" />)
    const markets = screen.getByRole('region', { name: 'Mercados' })
    fireEvent.click((await within(markets).findAllByRole('button', { name: /US100/ }))[0])
    expect(location.hash).toBe('#terminal')
    expect(heading()).toHaveTextContent('Terminal')
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

describe('resultados recientes y selector de estrategia', () => {
  const registry = () => screen.getByRole('list', { name: 'Últimas pruebas del registro' })

  it('las últimas filas del registro, la más nueva arriba, con el veredicto tal cual', () => {
    render(<App initialStage="app" />)
    const items = within(registry()).getAllByRole('listitem')
    const latest = REGISTRY.slice(-5).reverse()
    expect(items).toHaveLength(latest.length)
    items.forEach((li, i) => {
      expect(li).toHaveTextContent(latest[i].id)
      expect(li).toHaveTextContent(latest[i].veredicto)
    })
    const results = screen.getByRole('region', { name: 'Resultados recientes' })
    expect(within(results).getByText(/Resultados del pasado, no promesas/)).toBeVisible()
  })

  it('el selector solo filtra lo que se ve y se recuerda', () => {
    const { unmount } = render(<App initialStage="app" />)
    // píldoras de vidrio con radios de verdad (#645), agrupadas bajo "Estrategia en pantalla"
    const group = screen.getByRole('group', { name: 'Estrategia en pantalla' })
    fireEvent.click(within(group).getByRole('radio', { name: 'ML' }))
    const ml = REGISTRY.filter((r) => r.programa === 'ML')
    const items = within(registry()).getAllByRole('listitem')
    expect(items).toHaveLength(Math.min(5, ml.length))
    for (const li of items)
      expect(ml.some((r) => li.textContent?.includes(`${r.id} · ${r.fecha}`))).toBe(true)
    expect(document.querySelector('[data-metric="registro"]')).toHaveTextContent(String(ml.length))
    // las corridas del motor no traen la estrategia: no cambian
    expect(
      within(screen.getByRole('list', { name: 'Últimas corridas del motor' })).getAllByRole(
        'listitem',
      ),
    ).toHaveLength(BACKTEST_RUNS.length)
    unmount()
    render(<App initialStage="app" />)
    expect(screen.getByRole('radio', { name: 'ML' })).toBeChecked()
    expect(screen.getByRole('radio', { name: 'Todas' })).not.toBeChecked()
  })

  it('las corridas: de ejemplo con sello sin motor; del motor, en el orden en que llegan', async () => {
    const { unmount } = render(<App initialStage="app" />)
    const runs = () => screen.getByRole('list', { name: 'Últimas corridas del motor' })
    expect(runs().parentElement).toHaveTextContent('EJEMPLO')
    unmount()

    const arrived = [BACKTEST_RUNS[2], BACKTEST_RUNS[0]]
    gateway({ '/api/v1/backtest_runs': arrived })
    render(<App initialStage="app" />)
    await waitFor(() => expect(within(runs()).getAllByRole('listitem')).toHaveLength(2))
    expect(runs().parentElement).not.toHaveTextContent('EJEMPLO')
    const items = within(runs()).getAllByRole('listitem')
    expect(items[0]).toHaveTextContent('0,00 %')
    expect(items[1]).toHaveTextContent('+4,12 %')
  })

  it('ningún texto promete ganancias', () => {
    render(<App initialStage="app" />)
    expect(document.body).not.toHaveTextContent(/ganancia asegurada|garantizad|sin riesgo/i)
  })
})

describe('lectura del registro', () => {
  it('respeta comillas, comillas dobles y comas dentro de un campo', () => {
    expect(parseCsv('a,b\n"x, y","dijo ""no"""\n\n')).toEqual([
      ['a', 'b'],
      ['x, y', 'dijo "no"'],
    ])
    const [row] = parseRegistry('id,veredicto,programa\nR1,"no pasa, por poco",ML\r\n')
    expect(row).toMatchObject({ id: 'R1', veredicto: 'no pasa, por poco', programa: 'ML' })
  })

  it('el registro real se lee entero', () => {
    expect(REGISTRY.length).toBeGreaterThan(10)
    for (const r of REGISTRY) expect(r.id).toMatch(/^R\d{3}$/)
  })
})
