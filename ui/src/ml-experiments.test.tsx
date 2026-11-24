/**
 * «Modelo (informativo)» en Performance (#426): los experimentos de ML tal como los devuelve el
 * gateway (`ml_experiment_entry.v1`). Datos de ejemplo, no resultados reales.
 */
import { render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { MlExperimentEntry } from '@/lib/ml-api'
import sample from '@/mocks/ml-experiments.json'
import { MlExperimentsSection } from '@/views/phase8'

const ENTRIES = sample.entries as unknown as MlExperimentEntry[]

function gateway(reply: () => Response) {
  const fetch = vi.fn(async (input: string | URL | Request) =>
    String(input) === '/api/v1/ml/experiments' ? reply() : new Response('', { status: 404 }),
  )
  vi.stubGlobal('fetch', fetch)
  return fetch
}

const ok = (body: unknown) => () => new Response(JSON.stringify(body), { status: 200 })

describe('Modelo (informativo) (#426)', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('muestra el último experimento de cada instrumento, tal como llega', async () => {
    const fetch = gateway(ok(ENTRIES))
    render(<MlExperimentsSection />)
    const section = screen.getByRole('region', { name: 'Modelo (informativo)' })
    // solo informativo, sin promesas
    expect(section).toHaveTextContent('solo informativo')
    expect(section).toHaveTextContent('no decide operaciones')
    expect(section).not.toHaveTextContent(/ganancia|garantiz|asegurad/i)

    const de40 = await screen.findByRole('article', { name: 'Modelo de DE40' })
    expect(fetch).toHaveBeenCalledWith('/api/v1/ml/experiments', expect.anything())
    // veredicto con la diferencia y su intervalo, tal como los guardó el motor
    expect(within(de40).getByText('M1 no mejora: queda B1')).toHaveAttribute(
      'data-verdict',
      'b1_stays',
    )
    expect(de40).toHaveTextContent('Log loss de M1 menos el de B1: +0,0037')
    expect(de40).toHaveTextContent('intervalo del 95 %: −0,0021 a 0,0109, bootstrap por 52 semanas')
    expect(de40).toHaveTextContent('23 nov, 10:00 UTC')
    expect(de40).toHaveTextContent('corrida de backtest')
    expect(de40).toHaveTextContent('1 de 2 ventanas entrenaron')
    // B0, B1 y M1 con log loss, Brier y AUC (sin AUC: guion)
    const table = within(de40).getByRole('table', { name: 'Métricas por modelo' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows.map((r) => r.getAttribute('data-model'))).toEqual(['B0', 'B1', 'M1'])
    expect(rows[0]).toHaveTextContent('0,69310,2500—')
    expect(rows[1]).toHaveTextContent('0,68120,24370,552')
    expect(rows[2]).toHaveTextContent('0,68490,24510,561')
  })

  it('en suspenso dice por qué, con los motivos de cada ventana, y sin métricas', async () => {
    gateway(ok(ENTRIES))
    render(<MlExperimentsSection />)
    const us500 = await screen.findByRole('article', { name: 'Modelo de US500.FUT' })
    expect(within(us500).getByText('En suspenso: se sigue mostrando B1')).toBeInTheDocument()
    expect(us500).toHaveTextContent('No hubo datos suficientes para evaluar M1 contra B1.')
    expect(us500).toHaveTextContent(
      '0 de 3 ventanas entrenaron · sin entrenar: 0 propuestas, el mínimo es 300 (2); sin propuestas de validación (1)',
    )
    expect(us500).toHaveTextContent('etiquetas sin P/L')
    expect(us500).not.toHaveTextContent('Log loss de M1 menos')
    expect(within(us500).queryByRole('table')).toBeNull()
    expect(us500).toHaveTextContent('Sin métricas')
  })

  it('de los motivos lista los tres más frecuentes y cuenta el resto', async () => {
    const base = ENTRIES[1]
    const windows = ['a', 'b', 'b', 'c', 'c', 'c', 'd', 'e'].map((why) => ({
      ...base.experiment.windows[0],
      skipped: `motivo ${why}`,
    }))
    gateway(ok([{ ...base, experiment: { ...base.experiment, windows } }]))
    render(<MlExperimentsSection />)
    const card = await screen.findByRole('article', { name: 'Modelo de US500.FUT' })
    expect(card).toHaveTextContent(
      '0 de 8 ventanas entrenaron · sin entrenar: motivo c (3); motivo b (2); motivo a (1) y 2 motivos más',
    )
  })

  it('sin experimentos o sin gateway, lo dice', async () => {
    gateway(ok([]))
    const { unmount } = render(<MlExperimentsSection />)
    expect(screen.getByRole('status')).toHaveTextContent('Pidiendo los experimentos')
    expect(
      await screen.findByText('Todavía no hay experimentos de ML guardados.'),
    ).toBeInTheDocument()
    unmount()

    gateway(() => new Response('', { status: 502 }))
    render(<MlExperimentsSection />)
    expect(
      await screen.findByText('Sin conexión con el gateway: no hay experimentos para mostrar.'),
    ).toBeInTheDocument()
  })
})
