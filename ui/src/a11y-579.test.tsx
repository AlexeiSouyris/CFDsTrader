/// <reference types="node" />
/**
 * Accesibilidad y celular (QA 25-11, #579): saltar al contenido, logo, tema guardado y del
 * sistema, buscador como diálogo modal, barra inferior con "Más", nombres iguales a los títulos,
 * deslizadores con nombre, tablas desplazables enfocables, campos de 16 px en táctil, verdes y
 * rojos con contraste AA, PWA sin bloqueo de orientación y etiquetas visibles en el ingreso.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import manifestRaw from '../public/manifest.webmanifest?raw'
import App from '@/App'
import { ScrollRegion } from '@/components/ui/scroll-region'
import { VIEWS } from '@/views/nav'

const indexCss = readFileSync(join(process.cwd(), 'src/index.css'), 'utf8')

beforeEach(() => {
  localStorage.clear()
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => new Response('sin gateway', { status: 503 })),
  )
})
afterEach(() => {
  vi.unstubAllGlobals()
  localStorage.clear()
})

describe('armazón', () => {
  it('el primer enlace salta al contenido principal', () => {
    render(<App initialStage="app" />)
    const skip = screen.getByRole('link', { name: 'Saltar al contenido' })
    expect(skip).toHaveAttribute('href', '#contenido')
    const main = screen.getByRole('main')
    expect(main).toHaveAttribute('id', 'contenido')
    expect(main).toHaveAttribute('tabindex', '-1')
    // es lo primero que se enfoca con Tab
    const focusables = document.querySelectorAll('a[href], button, input')
    expect(focusables[0]).toBe(skip)
  })

  it('el logo lleva a la vista de inicio que dice su enlace', () => {
    history.replaceState(null, '', '/#terminal')
    render(<App initialStage="app" />)
    const logo = screen.getByRole('link', { name: 'CFDsTrader: ir a Inicio' })
    expect(logo).toHaveAttribute('href', '#inicio')
    fireEvent.click(logo)
    expect(location.hash).toBe('#inicio')
    expect(screen.getByRole('heading', { level: 1, name: 'Inicio' })).toBeInTheDocument()
  })

  it('la barra inferior usa los mismos nombres que los títulos y tiene "Más vistas"', () => {
    for (const v of VIEWS) expect(v.short).toBe(v.label)
    render(<App initialStage="app" />)
    const nav = screen.getByRole('navigation', { name: 'Vistas' })
    expect(within(nav).getByText('Operaciones')).toBeInTheDocument()
    expect(within(nav).getByText('Competencia')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Más vistas' }))
    const dialog = screen.getByRole('dialog', { name: 'Buscar una vista o un instrumento' })
    // con el campo vacío lista todas las vistas
    expect(within(dialog).getAllByRole('option').length).toBeGreaterThanOrEqual(VIEWS.length)
  })

  it('los deslizadores del vidrio tienen nombre y el popover se cierra con un clic afuera', () => {
    render(<App initialStage="app" />)
    fireEvent.click(screen.getByRole('button', { name: 'Ajustar el vidrio' }))
    expect(screen.getByRole('slider', { name: 'Desenfoque' })).toHaveAttribute('aria-valuetext')
    expect(screen.getByRole('slider', { name: 'Opacidad' })).toBeInTheDocument()
    fireEvent.pointerDown(screen.getByRole('main'))
    expect(screen.queryByRole('slider', { name: 'Desenfoque' })).not.toBeInTheDocument()
  })
})

describe('buscador', () => {
  it('es un diálogo modal con trampa de foco y devuelve el foco al cerrar', () => {
    render(<App initialStage="app" />)
    const opener = screen.getByRole('button', { name: /Buscar una vista o un instrumento/ })
    opener.focus()
    fireEvent.click(opener)
    const dialog = screen.getByRole('dialog', { name: 'Buscar una vista o un instrumento' })
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    const field = screen.getByRole('combobox', { name: 'Buscar' })
    const close = within(dialog).getByRole('button', { name: 'Cerrar el buscador' })
    close.focus()
    fireEvent.keyDown(close, { key: 'Tab' })
    expect(document.activeElement).toBe(field)
    fireEvent.keyDown(field, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(close)
    fireEvent.click(close)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(document.activeElement).toBe(opener)
  })
})

describe('tema', () => {
  const mediaWith = (light: boolean) => {
    const listeners: ((e: { matches: boolean }) => void)[] = []
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query.includes('light') ? light : false,
      media: query,
      addEventListener: (_: string, fn: (e: { matches: boolean }) => void) => listeners.push(fn),
      removeEventListener: vi.fn(),
    }))
    return (matches: boolean) => listeners.forEach((fn) => fn({ matches }))
  }

  it('sin elección guardada sigue al sistema, también si cambia', () => {
    const change = mediaWith(true)
    render(<App initialStage="app" />)
    expect(document.documentElement.dataset.theme).toBe('light')
    act(() => change(false))
    expect(document.documentElement.dataset.theme).toBe('dark')
  })

  it('la elección se guarda y gana al sistema', () => {
    const change = mediaWith(false)
    const { unmount } = render(<App initialStage="app" />)
    fireEvent.click(screen.getByRole('button', { name: 'Tema claro' }))
    expect(localStorage.getItem('cfds.theme')).toBe('light')
    unmount()
    render(<App initialStage="app" />)
    expect(document.documentElement.dataset.theme).toBe('light')
    act(() => change(false))
    expect(document.documentElement.dataset.theme).toBe('light')
  })
})

describe('tablas desplazables', () => {
  it('son regiones con nombre y entran al Tab solo si se desplazan', () => {
    const sizes = vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(800)
    const client = vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(340)
    render(
      <ScrollRegion label="Diario">
        <table />
      </ScrollRegion>,
    )
    const region = screen.getByRole('region', { name: 'Diario (tabla)' })
    expect(region).toHaveAttribute('tabindex', '0')
    expect(region.style.maskImage).toContain('transparent')
    expect(screen.getByText('Deslizá para ver más →')).toBeInTheDocument()
    sizes.mockRestore()
    client.mockRestore()
  })

  it('sin desborde no agrega una parada de Tab', () => {
    render(
      <ScrollRegion label="Chica">
        <table />
      </ScrollRegion>,
    )
    expect(screen.getByRole('region', { name: 'Chica (tabla)' })).not.toHaveAttribute('tabindex')
  })
})

describe('estilos y PWA', () => {
  it('en pantallas táctiles los campos tienen 16 px (Safari no hace zoom)', () => {
    expect(indexCss).toMatch(
      /@media \(pointer: coarse\) \{\s*input:not\(\[type='range'\], \[type='checkbox'\], \[type='radio'\]\),\s*select,\s*textarea \{\s*font-size: 16px;/,
    )
  })

  it('verde y rojo del tema claro con contraste AA sobre la tarjeta y el fondo', () => {
    const light = indexCss.slice(indexCss.indexOf("[data-theme='light'] {"))
    const pick = (name: string) => light.match(new RegExp(`--${name}: (#[0-9a-f]{6})`))![1]
    const lum = (hex: string) => {
      const [r, g, b] = [1, 3, 5].map((i) => {
        const v = parseInt(hex.slice(i, i + 2), 16) / 255
        return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
      })
      return 0.2126 * r + 0.7152 * g + 0.0722 * b
    }
    const ratio = (a: string, b: string) => {
      const [x, y] = [lum(a), lum(b)].sort((m, n) => n - m)
      return (x + 0.05) / (y + 0.05)
    }
    for (const color of [pick('chart-up'), pick('chart-down')])
      for (const bg of [pick('card'), pick('background')])
        expect(ratio(color, bg)).toBeGreaterThanOrEqual(4.5)
  })

  it('la PWA no bloquea la orientación', () => {
    const m = JSON.parse(manifestRaw) as { orientation?: string }
    expect(m.orientation ?? 'any').toBe('any')
  })
})

describe('ingreso', () => {
  it('cada campo tiene una etiqueta visible', () => {
    render(<App initialStage="signin" />)
    for (const name of ['Email', 'Contraseña', 'Código del segundo factor']) {
      const field = screen.getByLabelText(name)
      expect(field.closest('label')).toHaveTextContent(name)
      expect(field).not.toHaveAttribute('aria-label')
    }
  })
})
