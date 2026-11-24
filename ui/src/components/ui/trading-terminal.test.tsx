import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { OrderTicket } from '@/components/ui/trading-terminal'
import { dateTimeUtc } from '@/lib/format'
import type { Order, OrderReport, TicketVerdict } from '@/lib/paper-contracts'

/** Respuestas de prueba con la forma de los contratos (no son precios reales). */
const AS_OF = '2026-11-23T21:00:00Z'

const verdict = (o: Order, patch: Partial<TicketVerdict>): TicketVerdict => ({
  order_id: o.order_id,
  accepted: true,
  rejection: null,
  reference: '100.0',
  size: '2.5',
  risk: '50.00',
  risk_pct: '0.50',
  leverage: '1.2',
  margin: '400.00',
  leverage_capped: false,
  as_of: AS_OF,
  ...patch,
})

const rejectedNoCandles = (o: Order) =>
  verdict(o, {
    accepted: false,
    rejection: 'no_recent_candles',
    size: null,
    risk: null,
    risk_pct: null,
    leverage: null,
    margin: null,
  })

function renderTicket(
  preview: (o: Order) => Promise<TicketVerdict>,
  submit?: (o: Order) => Promise<OrderReport>,
) {
  render(
    <OrderTicket
      instruments={['US100']}
      initial={{ instrument_id: 'US100', side: 'long', stop: '99,5', tp1: '' }}
      preview={preview}
      submit={submit}
    />,
  )
  const form = screen.getByRole('form', { name: 'Ticket de orden' })
  fireEvent.click(within(form).getByRole('button', { name: /Validar con el motor/ }))
  return form
}

const NOTICE = `Sin velas en vivo desde ${dateTimeUtc(AS_OF)}.`

describe('ticket sin velas en vivo (no_recent_candles, #384)', () => {
  it('avisa desde cuándo no hay velas, como estado y sin el rojo de error', async () => {
    const form = renderTicket(async (o) => rejectedNoCandles(o))
    const notice = (await within(form).findByText(NOTICE)).closest('[role="status"]')
    expect(notice).not.toBeNull()
    expect(notice).toHaveTextContent('Vuelven cuando se renueve el cupo de IG.')
    expect(notice!.className).not.toMatch(/down/)

    const region = within(form).getByRole('region', { name: 'Validación de riesgo del motor' })
    expect(region.className).not.toMatch(/down/)
    expect(region).toHaveTextContent('En espera de datos')
    expect(region).not.toHaveTextContent('El motor la rechaza')
    // no repite el texto genérico del rechazo
    expect(region).not.toHaveTextContent('calcular el ATR')
    // sin velas no se puede enviar
    expect(within(form).queryByRole('button', { name: /en simulación/ })).toBeNull()
  })

  it('otro rechazo sigue en rojo con su motivo', async () => {
    const form = renderTicket(async (o) =>
      verdict(o, { accepted: false, rejection: 'stop_wrong_side', size: null }),
    )
    const region = await within(form).findByRole('region', {
      name: 'Validación de riesgo del motor',
    })
    expect(region).toHaveTextContent('El motor la rechaza')
    expect(region).toHaveTextContent('El stop está del lado equivocado del precio.')
    expect(region.className).toMatch(/down/)
    expect(within(form).queryByText(/Sin velas en vivo/)).toBeNull()
  })

  it('si el broker rechaza al enviar por falta de velas, avisa con la fecha de la validación', async () => {
    const submit = vi.fn(async (o: Order): Promise<OrderReport> => ({
      order_id: o.order_id,
      status: 'rejected',
      rejection: 'no_recent_candles',
      position_id: null,
      size: null,
      fill_price: null,
      commission: null,
      at: '2026-11-24T08:00:00Z',
    }))
    const form = renderTicket(async (o) => verdict(o, {}), submit)
    fireEvent.click(await within(form).findByRole('button', { name: 'Comprar en simulación' }))
    const notice = (await within(form).findByText(NOTICE)).closest('[role="status"]')
    expect(notice).toHaveTextContent('Vuelven cuando se renueve el cupo de IG.')
    expect(within(form).queryByText('El broker simulado la rechazó.')).toBeNull()
  })
})
