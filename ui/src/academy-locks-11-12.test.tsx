/**
 * Candados de las herramientas de los niveles 11–12 de la Academia (#851): cada uno tapa solo su
 * parte. En Liquidez, la tarjeta «Índice de desequilibrio» (`tool.imbalance`, lección 11.1): el
 * gráfico, los niveles y los FVG tienen los suyos. En el Laboratorio, solo el simulador de cuenta
 * fondeada (`tool.propsim`, lección 12.4): el constructor y el simulador de varianza tienen los
 * suyos y la calculadora de riesgo no tiene.
 *
 * Catálogo y progreso: los fixtures e2e (el catálogo real, todo bloqueado); «abierto» abre a mano,
 * como desde Ajustes, solo los ids de los niveles 11–12.
 */
import { render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress } from '@/lib/academy-contracts'
import type { StrategyCatalog } from '@/lib/lab-contracts'
import { LIQUIDITY_US100 as DATA } from '@/mocks'
import labCatalogJson from '@/mocks/lab-catalog.json'
import { LabView } from '@/views/lab'
import { LiquidityContent } from '@/views/liquidity'

const CATALOG = catalogFixture as AcademyCatalog
const LAB_CATALOG = labCatalogJson as unknown as StrategyCatalog

/** Los ids de los niveles 11–12, tal como están en `config/academy.toml`. */
const IDS = ['tool.imbalance', 'tool.propsim']
const nameOf = (id: string) => CATALOG.tools.find((t) => t.id === id)!.name

/** Todo bloqueado (el fixture) o los ids de 11–12 abiertos a mano. */
const progress = (open: boolean): AcademyProgress => {
  const base = progressFixture as AcademyProgress
  if (!open) return base
  return {
    ...base,
    tools: base.tools.map((t) =>
      IDS.includes(t.tool_id) ? { ...t, unlocked: true as const, by: 'override' as const } : t,
    ),
  }
}

/** Gateway de prueba: la Academia (si se pasa) y el catálogo del laboratorio; lo demás no existe. */
function gateway(p: AcademyProgress | null) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL | Request) => {
      const path = new URL(String(input), 'http://ui.test').pathname
      if (p !== null && path === '/api/v1/academy/catalog') return Response.json(CATALOG)
      if (p !== null && path === '/api/v1/academy/progress') return Response.json(p)
      if (path === '/api/v1/lab/catalog') return Response.json(LAB_CATALOG)
      return new Response('sin ruta', { status: 404 })
    }),
  )
}

/** El candado de una herramienta: su grupo, con el enlace a la lección que la abre. */
async function findLock(id: string, lesson: string) {
  const group = await screen.findByRole('group', { name: `${nameOf(id)}, bloqueada` })
  expect(group).toHaveTextContent(`Se desbloquea en la lección ${lesson}`)
  expect(within(group).getByRole('link', { name: 'Ir a la lección' })).toHaveAttribute(
    'href',
    '#academia',
  )
  return group
}

/** Los ids con candado que hay en la página, en orden. */
const locks = () =>
  [...document.querySelectorAll('[data-tool-lock]')].map((e) => e.getAttribute('data-tool-lock'))

const GAUGE = { name: /^Índice de desequilibrio, / }

beforeEach(() => resetAcademy())
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('Liquidez: índice de desequilibrio (#851)', () => {
  it('bloqueado: la tarjeta queda gris con su candado; el título y el gráfico siguen', async () => {
    gateway(progress(false))
    render(<LiquidityContent data={DATA} />)
    const group = await findLock('tool.imbalance', '11.1 · Flujo de órdenes y desequilibrio')
    // el candado va dentro de la tarjeta: su título se sigue viendo
    expect(group.closest('section')).toContainElement(
      screen.getByRole('heading', { name: 'Índice de desequilibrio' }),
    )
    expect(screen.queryByRole('meter', GAUGE)).toBeNull()
    // el gráfico de precio no se bloquea nunca
    expect(screen.getByRole('img', { name: /con 0 niveles de liquidez y 0 FVG/ })).toBeVisible()
  })

  it('abierto: el índice como hoy (los niveles y los FVG siguen con sus candados)', async () => {
    gateway(progress(true))
    render(<LiquidityContent data={DATA} />)
    await screen.findByRole('group', { name: 'Mapa de liquidez, bloqueada' })
    expect(screen.getByRole('meter', GAUGE)).toBeVisible()
    expect(locks()).not.toContain('tool.imbalance')
  })

  it('sin la Academia del motor no se bloquea nada', async () => {
    gateway(null)
    render(<LiquidityContent data={DATA} />)
    await vi.waitFor(() => expect(fetch).toHaveBeenCalled())
    expect(screen.getByRole('meter', GAUGE)).toBeVisible()
    expect(locks()).toEqual([])
  })
})

describe('Laboratorio: simulador de cuenta fondeada (#851)', () => {
  it('bloqueado: solo el simulador, con el candado de la 12.4', async () => {
    gateway(progress(false))
    render(<LabView />)
    await findLock('tool.propsim', '12.4 · Pasar por suerte: el simulador')
    expect(screen.queryByRole('region', { name: 'Simulador de cuenta fondeada' })).toBeNull()
    // cada herramienta del Laboratorio con su candado; la calculadora de riesgo no tiene
    expect(locks()).toEqual(['tool.lab_builder', 'tool.variance_simulator', 'tool.propsim'])
    expect(screen.getByRole('region', { name: 'Calculadora de riesgo' })).toBeVisible()
  })

  it('abierto: el simulador como hoy (el constructor y la varianza siguen con sus candados)', async () => {
    gateway(progress(true))
    render(<LabView />)
    await findLock('tool.lab_builder', '10.3')
    expect(screen.getByRole('region', { name: 'Simulador de cuenta fondeada' })).toBeVisible()
    expect(locks()).toEqual(['tool.lab_builder', 'tool.variance_simulator'])
  })

  it('sin la Academia del motor no se bloquea nada', async () => {
    gateway(null)
    render(<LabView />)
    expect(await screen.findByRole('form', { name: 'Constructor de estrategias' })).toBeVisible()
    expect(screen.getByRole('region', { name: 'Simulador de cuenta fondeada' })).toBeVisible()
    expect(locks()).toEqual([])
  })
})
