/**
 * Nombres legibles de los instrumentos (#861): la UI muestra el nombre del catálogo del motor
 * (`gateway_instruments.v1`) en vez del id ("DAX 40 (Alemania)", no "DE40"), y el id si el catálogo
 * no lo tiene. Los datos son de prueba, no cotizaciones.
 */
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { InstrumentName } from '@/components/ui/instrument-name'
import { catalogNames, instrumentLabel, instrumentName } from '@/lib/liquidity-api'
import type { GatewayInstruments } from '@/lib/liquidity-contracts'

const CATALOG: GatewayInstruments = {
  default_instrument: 'DE40',
  default_timeframe: '1h',
  timeframes: ['15m', '1h', '1d'],
  instruments: [
    {
      id: 'DE40',
      name: 'DAX 40 (Alemania)',
      asset_class: 'index',
      currency: 'EUR',
      exchange_timezone: 'Europe/Berlin',
    },
    {
      id: 'US500',
      name: 'S&P 500',
      asset_class: 'index',
      currency: 'EUR',
      exchange_timezone: 'America/New_York',
    },
  ],
}

describe('nombres de los instrumentos (#861)', () => {
  it('salen del catálogo; sin catálogo o con una respuesta rara, no hay nombres', () => {
    const names = catalogNames({ status: 'ok', data: CATALOG })
    expect(names).toEqual({ DE40: 'DAX 40 (Alemania)', US500: 'S&P 500' })
    expect(catalogNames({ status: 'loading' })).toEqual({})
    expect(catalogNames({ status: 'error', message: 'sin gateway' })).toEqual({})
    // un gateway falso que responde otra cosa no rompe la vista
    expect(catalogNames({ status: 'ok', data: {} as GatewayInstruments })).toEqual({})
  })

  it('el nombre conocido primero; el id solo si el catálogo no lo tiene', () => {
    const names = catalogNames({ status: 'ok', data: CATALOG })
    expect(instrumentName(names, 'DE40')).toBe('DAX 40 (Alemania)')
    expect(instrumentLabel(names, 'US500')).toBe('S&P 500 (US500)')
    // IBKR (MNQ) no está en el catálogo del CFD: se ve el id, sin inventar un nombre
    expect(instrumentName(names, 'MNQ')).toBe('MNQ')
    expect(instrumentLabel(names, 'MNQ')).toBe('MNQ')
  })

  it('el componente muestra el nombre con el id chico al lado', () => {
    const names = catalogNames({ status: 'ok', data: CATALOG })
    const { container } = render(<InstrumentName id="DE40" names={names} />)
    expect(screen.getByText('DAX 40 (Alemania)')).toHaveAttribute('title', 'DE40')
    expect(container.querySelector('[data-instrument-id]')).toHaveTextContent('DE40')
  })

  it('sin nombre, el id solo y sin repetirlo', () => {
    const { container } = render(<InstrumentName id="MES" names={{}} />)
    expect(container).toHaveTextContent(/^MES$/)
    expect(container.querySelector('[data-instrument-id]')).toBeNull()
  })
})
