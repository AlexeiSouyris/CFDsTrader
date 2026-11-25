import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import App from '@/App'
import { signed } from '@/lib/format'
import { CHART_SIGNALS, CURRENT_MEMBER, MEMBERS, PAPER_ACCOUNT, PAPER_POSITIONS } from '@/mocks'
import { TerminalView } from '@/views/terminal'

const ticket = () => screen.getByRole('form', { name: 'Ticket de orden' })

describe('terminal de trading (fase 10, #275)', () => {
  it('se abre desde la navegación y avisa en grande que es simulación', () => {
    render(<App initialStage="app" />)
    const nav = screen.getByRole('navigation', { name: 'Vistas' })
    fireEvent.click(within(nav).getByText('Terminal'))
    expect(location.hash).toBe('#terminal')
    expect(screen.getByRole('status', { name: /Modo simulación/ })).toHaveTextContent(
      'MODO SIMULACIÓN',
    )
  })

  it('muestra la cuenta y las posiciones tal como las manda el motor', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const account = screen.getByRole('region', { name: 'Cuenta de simulación' })
    expect(account).toHaveTextContent(signed(PAPER_ACCOUNT.unrealized_pl, 2))
    const table = screen.getByRole('table', { name: 'Posiciones abiertas' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows).toHaveLength(PAPER_POSITIONS.length)
    PAPER_POSITIONS.forEach((p, i) => {
      expect(rows[i]).toHaveTextContent(p.instrument_id)
      expect(rows[i]).toHaveTextContent(`${signed(p.unrealized_pl, 2)} €`)
      expect(rows[i]).toHaveTextContent(`${signed(p.unrealized_r)} R`)
    })
  })

  it('el capital va en su propia lista, sin compartir grilla con el P/L (#356)', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const account = screen.getByRole('region', { name: 'Cuenta de simulación' })
    const lists = account.querySelectorAll('dl')
    expect(lists).toHaveLength(2)
    expect(lists[0]).toHaveTextContent('Capital')
    expect(lists[0]).not.toHaveTextContent('P/L abierto')
    expect(lists[1]).toHaveTextContent('P/L abierto')
  })

  it('en el celular cada celda de una posición lleva su nombre (#356)', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const table = screen.getByRole('table', { name: 'Posiciones abiertas' })
    const [row] = within(table).getAllByRole('row').slice(1)
    const labels = within(row)
      .getAllByRole('cell')
      .map((c) => c.getAttribute('data-label'))
      .filter(Boolean)
    expect(labels).toEqual([
      'Tamaño',
      'Entrada',
      'Último',
      'Stop · TP1',
      'P/L',
      'Margen',
      'Abierta',
    ])
  })

  it('marca las señales del motor en el gráfico', () => {
    const { container } = render(<TerminalView member={CURRENT_MEMBER} />)
    expect(container.querySelectorAll('[data-signal]')).toHaveLength(CHART_SIGNALS.length)
  })

  it('el ticket exige stop y muestra la validación del motor, sin calcularla', async () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = ticket()
    const stop = within(form).getByLabelText(/Stop/)
    fireEvent.change(stop, { target: { value: '' } })
    fireEvent.submit(form)
    expect(within(form).getByRole('alert')).toHaveTextContent('obligatorio')

    fireEvent.change(stop, { target: { value: '28150,2' } })
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    const verdict = await within(form).findByRole('region', {
      name: 'Validación de riesgo del motor',
    })
    expect(verdict).toHaveTextContent('El motor la acepta')
    // el tamaño lo decide el riesgo del motor: la UI lo muestra tal cual
    expect(verdict).toHaveTextContent('0,19 €/pt')
    fireEvent.click(within(form).getByRole('button', { name: 'Comprar en simulación' }))
    expect(within(form).getByRole('status')).toHaveTextContent('no se envió nada')
  })

  it('un ticket que el motor rechaza lo dice, y uno sin respuesta no se inventa', async () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    const form = ticket()
    const stop = within(form).getByLabelText(/Stop/)
    fireEvent.change(stop, { target: { value: '28195.0' } })
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    expect(await within(form).findByText('El motor la rechaza')).toBeInTheDocument()
    expect(within(form).getByText('El stop está demasiado cerca.')).toBeInTheDocument()
    expect(within(form).queryByRole('button', { name: /en simulación/ })).not.toBeInTheDocument()

    fireEvent.change(stop, { target: { value: '27000' } })
    fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
    expect(await within(form).findByRole('status')).toHaveTextContent('Sin respuesta del motor')
  })

  it('con el kill switch activo avisa y no deja operar ni aprobar', () => {
    render(<TerminalView member={MEMBERS[0]} />)
    fireEvent.click(screen.getByRole('button', { name: 'Kill switch activo' }))
    fireEvent.click(screen.getByRole('tab', { name: 'Propuestas' }))
    expect(screen.getByRole('alert')).toHaveTextContent('Kill switch activo')
    expect(within(ticket()).getByRole('button', { name: /Validar con el motor/ })).toBeDisabled()
    expect(screen.queryByRole('button', { name: /Aprobar/ })).not.toBeInTheDocument()
    expect(screen.getAllByText(/no se puede operar ni aprobar/).length).toBeGreaterThan(1)
  })

  it('las propuestas muestran la probabilidad con su rango, nunca como certeza', () => {
    render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Propuestas' }))
    const props = screen.getByRole('region', { name: 'Propuestas' })
    expect(props).toHaveTextContent(/rango \d+–\d+ %/)
    expect(props).toHaveTextContent('no una promesa')
  })

  it('ningún texto de la terminal promete ganancias', () => {
    const { container } = render(<TerminalView member={CURRENT_MEMBER} />)
    fireEvent.click(screen.getByRole('button', { name: 'Kill switch activo' }))
    expect(container.textContent).not.toMatch(
      /ganancia (segura|garantizada|asegurada)|sin riesgo|asegurad|garantiz|seguro que gan/i,
    )
  })
})
