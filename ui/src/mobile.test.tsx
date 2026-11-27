/// <reference types="node" />
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import indexHtml from '../index.html?raw'
import manifestRaw from '../public/manifest.webmanifest?raw'
import swSource from '../public/sw.js?raw'
import App from '@/App'
import { CURRENT_MEMBER } from '@/mocks'
import { TerminalView } from '@/views/terminal'

// Vitest vacía los `.css` importados (también con `?raw`): el texto se lee del disco.
const indexCss = readFileSync(join(process.cwd(), 'src/index.css'), 'utf8')

/* ---------- service worker (#307): el `public/sw.js` real, con un navegador de mentira ---------- */

type FakeResponse = {
  ok: boolean
  type: string
  headers: Headers
  body: string
  clone: () => FakeResponse
}
type FakeRequest = { method: string; url: string; mode: string }
type Handler = (event: {
  request?: FakeRequest
  respondWith: (p: Promise<FakeResponse | undefined>) => void
  waitUntil: (p: Promise<unknown>) => void
}) => void

const ORIGIN = 'http://localhost'

function response(body: string, { ok = true, cacheControl = '' } = {}): FakeResponse {
  const r: FakeResponse = {
    ok,
    type: 'basic',
    headers: new Headers(cacheControl ? { 'Cache-Control': cacheControl } : {}),
    body,
    clone: () => r,
  }
  return r
}

/** Carga el service worker con `self`, `caches` y `fetch` simulados. */
function loadWorker(network: (req: FakeRequest | string) => Promise<FakeResponse>) {
  const store = new Map<string, FakeResponse>()
  const key = (r: FakeRequest | string) =>
    typeof r === 'string' ? r : new URL(r.url, ORIGIN).pathname
  const cache = {
    addAll: async (urls: string[]) => {
      for (const u of urls) store.set(u, await network(u))
    },
    put: async (r: FakeRequest | string, res: FakeResponse) => void store.set(key(r), res),
  }
  const deleted: string[] = []
  const caches = {
    open: async () => cache,
    match: async (r: FakeRequest | string) => store.get(key(r)),
    keys: async () => ['cfds-shell-v0', 'cfds-shell-v1'],
    delete: async (k: string) => void deleted.push(k),
  }
  const handlers: Record<string, Handler> = {}
  const self = {
    location: { origin: ORIGIN },
    addEventListener: (type: string, h: Handler) => (handlers[type] = h),
    skipWaiting: vi.fn(),
    clients: { claim: vi.fn() },
  }
  const fetchSpy = vi.fn(network)
  new Function('self', 'caches', 'fetch', swSource)(self, caches, fetchSpy)

  const lifecycle = async (type: 'install' | 'activate') => {
    let done: Promise<unknown> = Promise.resolve()
    handlers[type]({ respondWith: () => {}, waitUntil: (p) => (done = p) })
    await done
  }
  /** Devuelve `undefined` si el SW no responde (el navegador hace el pedido normal). */
  const request = async (path: string, init: Partial<FakeRequest> = {}) => {
    let answer: Promise<FakeResponse | undefined> | undefined
    handlers.fetch({
      request: { method: 'GET', mode: 'cors', url: new URL(path, ORIGIN).href, ...init },
      respondWith: (p) => (answer = p),
      waitUntil: () => {},
    })
    return answer ? { response: await answer } : undefined
  }
  return { store, deleted, fetchSpy, lifecycle, request }
}

const flush = () => new Promise((r) => setTimeout(r, 0))

describe('service worker (#307)', () => {
  it('al instalarse guarda solo el armazón, nunca /api', async () => {
    const sw = loadWorker(async (r) => response(`net:${typeof r === 'string' ? r : r.url}`))
    await sw.lifecycle('install')
    const keys = [...sw.store.keys()]
    expect(keys).toContain('/index.html')
    expect(keys).toContain('/manifest.webmanifest')
    expect(keys.some((k) => k.startsWith('/api'))).toBe(false)
  })

  it('no toca /api, ni POST, ni otros orígenes: van a la red sin caché', async () => {
    const sw = loadWorker(async () => response('net'))
    await sw.lifecycle('install')
    for (const path of ['/api', '/api/paper/account', '/api/market/candles?x=1']) {
      expect(await sw.request(path)).toBeUndefined()
    }
    expect(await sw.request('/assets/app.js', { method: 'POST' })).toBeUndefined()
    expect(await sw.request('https://otro.example/assets/app.js')).toBeUndefined()
    await flush()
    expect([...sw.store.keys()].some((k) => k.startsWith('/api'))).toBe(false)
  })

  it('sin red abre la app desde la caché; con red usa la página nueva', async () => {
    let online = true
    const sw = loadWorker(async (r) => {
      if (!online) throw new TypeError('offline')
      return response(`v2:${typeof r === 'string' ? r : new URL(r.url).pathname}`)
    })
    await sw.lifecycle('install')
    const fresh = await sw.request('/', { mode: 'navigate' })
    expect(fresh?.response?.body).toBe('v2:/')

    online = false
    const offline = await sw.request('/#terminal', { mode: 'navigate' })
    expect(offline?.response?.body).toBe('v2:/')
  })

  it('no guarda respuestas con error ni con no-store', async () => {
    const sw = loadWorker(async (r) => {
      const path = typeof r === 'string' ? r : new URL(r.url).pathname
      if (path === '/assets/roto.js') return response('err', { ok: false })
      if (path === '/assets/privado.js') return response('x', { cacheControl: 'no-store' })
      return response('ok')
    })
    await sw.request('/assets/roto.js')
    await sw.request('/assets/privado.js')
    await sw.request('/assets/bueno.js')
    await flush()
    expect(sw.store.has('/assets/roto.js')).toBe(false)
    expect(sw.store.has('/assets/privado.js')).toBe(false)
    expect(sw.store.has('/assets/bueno.js')).toBe(true)
  })

  it('al activarse borra las cachés viejas', async () => {
    const sw = loadWorker(async () => response('ok'))
    await sw.lifecycle('activate')
    expect(sw.deleted).toEqual(['cfds-shell-v0'])
  })
})

/* ---------- manifest, index.html y CSS ---------- */

describe('instalable en el celular (#307)', () => {
  it('el manifest abre la app sola, con íconos y avisa que es simulación', () => {
    const m = JSON.parse(manifestRaw) as {
      display: string
      start_url: string
      description: string
      icons: { sizes: string; purpose?: string }[]
    }
    expect(m.display).toBe('standalone')
    expect(m.start_url.startsWith('/')).toBe(true)
    expect(m.description).toMatch(/simulación/)
    const sizes = m.icons.map((i) => i.sizes)
    expect(sizes).toContain('192x192')
    expect(sizes).toContain('512x512')
    expect(m.icons.some((i) => i.purpose === 'maskable')).toBe(true)
  })

  it('index.html enlaza el manifest y prepara el iPhone', () => {
    expect(indexHtml).toContain('rel="manifest" href="/manifest.webmanifest"')
    expect(indexHtml).toContain('name="theme-color"')
    expect(indexHtml).toContain('rel="apple-touch-icon"')
    expect(indexHtml).toContain('viewport-fit=cover')
  })

  it('la página no se desplaza en horizontal y los toques miden al menos 44 px', () => {
    expect(indexCss).toMatch(/overflow-x:\s*clip/)
    expect(indexCss).toMatch(/@media \(pointer: coarse\), \(max-width: 767px\)/)
    expect(indexCss).toMatch(/min-height:\s*44px/)
    expect(indexCss).toContain('[data-inline-hit]::after')
  })
})

/* ---------- gestos, gráficos y ticket ---------- */

const heading = () => screen.getByRole('heading', { level: 1 })

function fingerSwipe(el: Element, fromX: number, toX: number, y = 300) {
  fireEvent.touchStart(el, { touches: [{ clientX: fromX, clientY: y }] })
  fireEvent.touchEnd(el, { changedTouches: [{ clientX: toX, clientY: y + 5 }] })
}

function coarse(matches: boolean) {
  vi.spyOn(window, 'matchMedia').mockImplementation(
    (query: string) =>
      ({
        matches: matches && query.includes('coarse'),
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
      }) as unknown as MediaQueryList,
  )
}

describe('celular: gestos, gráficos y ticket (#307)', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    localStorage.clear()
  })

  it('deslizar con el dedo cambia de pestaña; desde el borde o sobre un gráfico, no', () => {
    render(<App initialStage="app" />)
    const main = screen.getByRole('main')
    expect(heading()).toHaveTextContent('Inicio')
    fingerSwipe(main, 400, 200)
    expect(heading()).toHaveTextContent('Terminal')
    fingerSwipe(main, 200, 400)
    expect(heading()).toHaveTextContent('Inicio')

    // pegado al borde izquierdo: es el "atrás" del iPhone
    fingerSwipe(main, 10, 300)
    expect(heading()).toHaveTextContent('Inicio')
    // corto o vertical: no cuenta
    fingerSwipe(main, 400, 360)
    expect(heading()).toHaveTextContent('Inicio')

    const chart = document.createElement('div')
    chart.className = 'touch-none'
    main.appendChild(chart)
    fingerSwipe(chart, 400, 100)
    expect(heading()).toHaveTextContent('Inicio')
  })

  it('con 4 gráficos, en el celular se ve uno y un selector elige cuál', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    expect(document.querySelector('[data-chart-switcher]')).toBeNull()
    fireEvent.click(
      within(screen.getByRole('group', { name: 'Gráficos en pantalla' })).getByRole('button', {
        name: '4',
      }),
    )
    const hidden = () =>
      [...document.querySelectorAll('[data-chart-slot]')].map((el) =>
        el.classList.contains('max-lg:hidden'),
      )
    expect(hidden()).toEqual([false, true, true, true])
    const picker = screen.getByRole('group', { name: 'Gráfico a la vista' })
    fireEvent.click(within(picker).getByRole('button', { name: '3' }))
    expect(hidden()).toEqual([true, true, false, true])
    expect(within(picker).getByRole('button', { name: '3' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
  })

  async function validated() {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = screen.getByRole('form', { name: 'Ticket de orden' })
    expect(form.querySelector('[data-paper-banner]')).toHaveTextContent('SIMULACIÓN')
    fireEvent.change(within(form).getByLabelText(/Stop/), { target: { value: '28150,2' } })
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    await within(form).findByRole('region', { name: 'Validación de riesgo del motor' })
    return form
  }

  it('en pantalla táctil, enviar pide confirmar; cancelar no manda nada', async () => {
    coarse(true)
    const form = await validated()
    fireEvent.click(within(form).getByRole('button', { name: 'Comprar en simulación' }))
    const confirm = within(form).getByRole('group', { name: 'Confirmar el envío' })
    expect(confirm).toHaveTextContent('SIMULACIÓN')
    expect(within(form).queryByRole('status')).not.toBeInTheDocument()

    fireEvent.click(within(confirm).getByRole('button', { name: 'Cancelar' }))
    expect(within(form).queryByRole('group', { name: 'Confirmar el envío' })).toBeNull()
    expect(within(form).queryByRole('status')).not.toBeInTheDocument()

    fireEvent.click(within(form).getByRole('button', { name: 'Comprar en simulación' }))
    fireEvent.click(
      within(within(form).getByRole('group', { name: 'Confirmar el envío' })).getByRole('button', {
        name: 'Confirmar',
      }),
    )
    expect(within(form).getByRole('status')).toHaveTextContent('no se envió nada')
  })

  it('con mouse, enviar va directo como siempre', async () => {
    coarse(false)
    const form = await validated()
    fireEvent.click(within(form).getByRole('button', { name: 'Comprar en simulación' }))
    expect(within(form).queryByRole('group', { name: 'Confirmar el envío' })).toBeNull()
    expect(within(form).getByRole('status')).toHaveTextContent('no se envió nada')
  })
})
