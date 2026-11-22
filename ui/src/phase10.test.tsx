import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { ProposalCard } from '@/components/ui/proposal-card'
import { Term } from '@/components/ui/term'
import { GLOSSARY, GLOSSARY_BY_ID, parseGlossary } from '@/lib/glossary'
import { FUND, MEMBERS, PROPOSALS } from '@/mocks'
import { FundView, GlossaryView, ProposalsView } from '@/views/phase10'

describe('glosario (fuente: docs/GLOSARIO.md)', () => {
  it('lee los términos con su definición y su ejemplo, sin las secciones', () => {
    const md = [
      '<a id="seccion"></a>',
      '## Sección',
      '',
      '<a id="r"></a>',
      '### R (múltiplo de riesgo)',
      'La **distancia** al [stop](#stop).',
      '*Ejemplo:* 1R = 50 puntos.',
      '',
      '---',
    ].join('\n')
    expect(parseGlossary(md)).toEqual([
      {
        id: 'r',
        title: 'R (múltiplo de riesgo)',
        body: 'La distancia al stop.',
        example: '1R = 50 puntos.',
      },
    ])
  })

  it('carga el glosario real del repo', () => {
    expect(GLOSSARY.length).toBeGreaterThan(50)
    expect(GLOSSARY_BY_ID.get('sweep')?.title).toBe('Sweep (barrido de liquidez)')
    expect(GLOSSARY.every((e) => e.title && e.body)).toBe(true)
  })

  it('cada término que usa la UI existe en el glosario', () => {
    const files = import.meta.glob('./**/*.tsx', { query: '?raw', import: 'default', eager: true })
    const used = new Set<string>()
    for (const src of Object.values(files) as string[]) {
      for (const m of src.matchAll(/<Term id="([^"]+)"/g)) used.add(m[1])
      for (const m of src.matchAll(/term: '([^']+)'/g)) used.add(m[1])
      for (const m of src.matchAll(/KPI_TERM[^=]*=\s*\{([^}]*)\}/g))
        for (const v of m[1].matchAll(/'([^']+)'/g)) used.add(v[1])
    }
    used.delete('x') // ejemplo de este mismo archivo
    expect(used.size).toBeGreaterThan(10)
    expect([...used].filter((id) => !GLOSSARY_BY_ID.has(id))).toEqual([])
  })

  it('un término se abre con su definición y se cierra con Escape', () => {
    render(<Term id="sweep">sweep</Term>)
    fireEvent.click(screen.getByRole('button', { name: 'sweep' }))
    expect(screen.getByRole('dialog', { name: /Sweep/ })).toHaveTextContent('Ejemplo:')
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('un término que no existe queda como texto', () => {
    render(<Term id="no-existe">algo</Term>)
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    expect(screen.getByText('algo')).toBeInTheDocument()
  })

  it('la vista del glosario busca sin importar los acentos', () => {
    render(<GlossaryView />)
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'deslizamiento' } })
    expect(screen.getByRole('heading', { name: 'Slippage (deslizamiento)' })).toBeInTheDocument()
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'tendencía' } })
    expect(screen.getByRole('heading', { name: 'Tendencia' })).toBeInTheDocument()
  })
})

describe('propuestas', () => {
  it('muestran lo que trae proposal.v1, sin calcular', () => {
    render(<ProposalCard item={PROPOSALS[0]} canApprove />)
    const card = screen.getByRole('article', { name: 'Propuesta US100 largo' })
    expect(card).toHaveTextContent('28.204,6')
    expect(card).toHaveTextContent('5,15:1')
    expect(card).toHaveTextContent('58 %')
    expect(card).toHaveTextContent('49–66 %')
  })

  it('con pocos casos dicen "datos insuficientes"', () => {
    render(<ProposalCard item={PROPOSALS[1]} canApprove />)
    expect(screen.getByText('Datos insuficientes')).toBeInTheDocument()
  })

  it('solo el responsable aprueba, y es una maqueta', () => {
    const { unmount } = render(<ProposalsView member={MEMBERS[1]} />)
    expect(screen.queryByRole('button', { name: /Aprobar/ })).not.toBeInTheDocument()
    expect(screen.getAllByText(/Solo el responsable/).length).toBe(PROPOSALS.length)
    unmount()
    render(<ProposalsView member={MEMBERS[0]} />)
    const first = screen.getByRole('article', { name: 'Propuesta US100 largo' })
    fireEvent.click(within(first).getByRole('button', { name: /Aprobar/ }))
    expect(within(first).getByRole('status')).toHaveTextContent('no se envió nada')
  })

  it('una propuesta vencida no se puede aprobar', () => {
    const old = {
      ...PROPOSALS[0],
      extras: { ...PROPOSALS[0].extras, expiresAt: '2020-01-01T00:00:00Z' },
    }
    render(<ProposalCard item={old} canApprove />)
    expect(screen.getByText('Vencida')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Aprobar/ })).toBeDisabled()
  })
})

describe('fondo', () => {
  it('el historial de ejemplo tiene la forma de fund_movement.v1 y suma las unidades del fondo', () => {
    const decimalRe = /^\d+(\.\d+)?$/
    for (const mv of FUND.history) {
      expect(Object.keys(mv).sort()).toEqual(
        ['amount', 'currency', 'id', 'kind', 'member_id', 'nav', 'settled_at', 'units'].sort(),
      )
      expect(mv.id).toMatch(/^fund:\d{4}-\d{2}-\d{2}:[1-9]\d{0,3}$/)
      expect(mv.settled_at).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/)
      expect(mv.amount).toMatch(/^\d+(\.\d{1,2})?$/)
      expect(mv.units).toMatch(decimalRe)
      expect(mv.nav).toMatch(decimalRe)
    }
    // Coherencia de los datos de ejemplo (el motor es el que suma de verdad).
    const cents = FUND.history.reduce((acc, mv) => acc + Math.round(Number(mv.units) * 100), 0)
    expect(cents).toBe(Math.round(Number(FUND.units) * 100))
  })

  it('muestra el historial, del más reciente al más antiguo', () => {
    render(<FundView />)
    expect(screen.getByRole('heading', { name: 'Historial' })).toBeInTheDocument()
    expect(screen.getAllByText('aporte').length).toBeGreaterThanOrEqual(FUND.history.length)
  })

  it('muestra el valor de la unidad y lo de cada miembro tal como llega', () => {
    render(<FundView />)
    expect(screen.getByText('1,0412 €')).toBeInTheDocument()
    const members = screen.getAllByRole('table')[0]
    expect(within(members).getAllByRole('row')).toHaveLength(FUND.members.length + 1)
    expect(within(members).getByRole('row', { name: /Alexei/ })).toHaveTextContent('+12,36 €')
  })
})
