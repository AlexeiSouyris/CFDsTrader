/// <reference types="node" />
/**
 * Tokens compartidos y por sección (#581, etapa 1): fuentes autoalojadas, escala de 4 px, colores
 * de gráficos iguales en toda la app, variables por `data-section`, contraste AA en claro y oscuro
 * y vidrio opt-in.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import App from '@/App'

// Vitest vacía los `.css` importados (también con `?raw`): el texto se lee del disco.
const css = readFileSync(join(process.cwd(), 'src/index.css'), 'utf8')
const pkg = JSON.parse(readFileSync(join(process.cwd(), 'package.json'), 'utf8')) as {
  dependencies: Record<string, string>
}

/**
 * Las declaraciones `--x: valor` del bloque cuya lista de selectores termina en `selector`.
 * `last`: el último bloque así (la terminal aparece primero en la lista del tema oscuro).
 */
function block(selector: string, last = false): Record<string, string> {
  const at = last ? css.lastIndexOf(`${selector} {`) : css.indexOf(`${selector} {`)
  if (at < 0) throw new Error(`no está el bloque ${selector}`)
  const body = css.slice(at, css.indexOf('}', at))
  return Object.fromEntries([...body.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2]]))
}

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

const dark = block("[data-section='terminal']")
const terminal = block("[data-section='terminal']", true)
const light = block("[data-theme='light']")

/** Tokens resueltos de cada sección en cada tema (base del tema + lo que redefine la sección). */
const SECTIONS: { name: string; dark: Record<string, string>; light: Record<string, string> }[] = [
  { name: 'sin sección', dark, light },
  { name: 'hub', dark, light },
  {
    name: 'terminal',
    // siempre oscura: en el tema claro usa también los tokens oscuros
    dark: { ...dark, ...terminal },
    light: { ...dark, ...terminal },
  },
  {
    name: 'charts',
    dark: { ...dark, ...block("[data-section='charts']") },
    light: { ...light, ...block("[data-theme='light'][data-section='charts']") },
  },
  {
    name: 'research / journal',
    dark: { ...dark, ...block("[data-section='journal']") },
    light: { ...light, ...block("[data-theme='light'][data-section='journal']") },
  },
  {
    name: 'news',
    dark: { ...dark, ...block("[data-section='news']") },
    light: { ...light, ...block("[data-theme='light'][data-section='news']") },
  },
  {
    name: 'trophies',
    dark: { ...dark, ...block("[data-section='trophies']") },
    light: { ...light, ...block("[data-theme='light'][data-section='trophies']") },
  },
]

describe('fuentes', () => {
  it('JetBrains Mono y Source Serif 4 autoalojadas, con versión exacta', () => {
    for (const name of [
      '@fontsource-variable/manrope',
      '@fontsource-variable/jetbrains-mono',
      '@fontsource-variable/source-serif-4',
    ]) {
      expect(pkg.dependencies[name]).toMatch(/^\d+\.\d+\.\d+$/)
      expect(css).toContain(`@import '${name}';`)
    }
    expect(css).toMatch(/--font-mono:\s*'JetBrains Mono Variable'/)
    expect(css).toMatch(/--font-serif: 'Source Serif 4 Variable'/)
    // nada de fuentes de un CDN
    expect(css).not.toMatch(/fonts\.googleapis|@import url\(/)
  })
})

describe('escala y secciones', () => {
  it('espaciado con base de 4 px', () => {
    const root = block(':root')
    const steps = { 1: 0.25, 2: 0.5, 3: 0.75, 4: 1, 6: 1.5, 8: 2, 12: 3 }
    for (const [n, rem] of Object.entries(steps)) expect(root[`--space-${n}`]).toBe(`${rem}rem`)
  })

  it('cada sección define densidad, forma y acento', () => {
    const own = ['charts', 'news', 'trophies'].map((s) => block(`[data-section='${s}']`))
    for (const b of [terminal, ...own]) {
      expect(Object.keys(b)).toEqual(expect.arrayContaining(['--density', '--radius', '--accent']))
    }
    const editorial = block("[data-section='journal']")
    expect(editorial['--radius']).toBe('8px')
    expect(terminal['--radius']).toBe('4px')
    expect(block("[data-section='fund']")['--radius']).toBe('20px')
  })

  it('ninguna sección redefine los colores de los gráficos', () => {
    const sectionRules = [...css.matchAll(/\[data-section='[a-z]+'\][^{]*\{([^}]*)\}/g)]
    expect(sectionRules.length).toBeGreaterThan(5)
    for (const [, body] of sectionRules) {
      // el bloque base del tema oscuro también aplica a la terminal: ahí están los `--chart-*`
      if (body.includes('--background')) continue
      expect(body).not.toMatch(/--chart-/)
    }
  })

  it('la terminal es siempre oscura, también para las variantes `dark:`', () => {
    expect(css).toMatch(/:root,\s*\[data-theme='dark'\],\s*\[data-section='terminal'\] \{/)
    expect(css).toMatch(/@custom-variant dark \([^)]*\[data-section='terminal'\] \*/)
  })
})

describe('contraste AA en claro y oscuro', () => {
  for (const s of SECTIONS)
    for (const theme of ['dark', 'light'] as const) {
      it(`${s.name}, tema ${theme}`, () => {
        const t = s[theme]
        const bg = t['--section-bg']
        expect(bg).toMatch(/^#[0-9a-f]{6}$/)
        for (const token of [
          '--foreground',
          '--muted-foreground',
          '--accent',
          '--chart-up',
          '--chart-down',
        ])
          expect(ratio(t[token], bg), `${token} sobre ${bg}`).toBeGreaterThanOrEqual(4.5)
      })
    }

  it('respeta el verde y el rojo del tema claro de #589', () => {
    expect(light['--chart-up']).toBe('#0a6643')
    expect(light['--chart-down']).toBe('#a52d30')
  })
})

/** `fg` al `alpha` sobre `bg` (lo que hace `bg-x/10` de Tailwind), como hex. */
const over = (fg: string, alpha: number, bg: string) =>
  '#' +
  [1, 3, 5]
    .map((i) => {
      const f = parseInt(fg.slice(i, i + 2), 16)
      const b = parseInt(bg.slice(i, i + 2), 16)
      return Math.round(f * alpha + b * (1 - alpha))
        .toString(16)
        .padStart(2, '0')
    })
    .join('')

describe('contraste AA de los sellos (#605)', () => {
  for (const s of SECTIONS)
    for (const theme of ['dark', 'light'] as const) {
      it(`sello EJEMPLO en ${s.name}, tema ${theme}`, () => {
        const t = s[theme]
        const signal = t['--chart-signal']
        // la ficha del sello es el mismo color al 10 % sobre el fondo de la sección o la tarjeta
        for (const bg of [t['--section-bg'], t['--background'], t['--card'], t['--surface']]) {
          const chip = over(signal, 0.1, bg)
          expect(ratio(signal, chip), `${signal} sobre ${chip}`).toBeGreaterThanOrEqual(4.5)
        }
      })
    }

  for (const theme of ['dark', 'light'] as const) {
    it(`dorado de Trofeos sobre un trofeo ganado, tema ${theme}`, () => {
      const t = SECTIONS.find((x) => x.name === 'trophies')![theme]
      const gold = t['--accent']
      // tarjeta ganada: acento al 7 % sobre la sección; la ficha "Ganado", acento al 15 % encima
      const card = over(gold, 0.07, t['--section-bg'])
      const chip = over(gold, 0.15, card)
      for (const bg of [card, chip])
        expect(ratio(gold, bg), `${gold} sobre ${bg}`).toBeGreaterThanOrEqual(4.5)
    })
  }
})

describe('vidrio opt-in', () => {
  it('fuera de data-glass="on" es una superficie lisa; adentro, el vidrio', () => {
    const glass = css.slice(css.indexOf('@utility glass {'), css.indexOf('@utility glass-strong'))
    const flat = glass.slice(0, glass.indexOf('&:where'))
    expect(flat).not.toContain('backdrop-filter')
    expect(glass).toMatch(
      /&:where\(\[data-glass='on'\] \*, \[data-glass='on'\]\) \{[^}]*backdrop-filter/,
    )
  })

  it('el Inicio activa el vidrio (las demás vistas, ver home.test.tsx)', () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('sin gateway', { status: 503 })),
    )
    const { container } = render(<App initialStage="app" />)
    expect(container.firstElementChild).toHaveAttribute('data-glass', 'on')
    vi.unstubAllGlobals()
  })
})
