import { waitFor } from '@testing-library/react'
import { expect } from 'vitest'

/**
 * Espera el gráfico de ejemplo: sin gateway (en las pruebas, `fetch` falla), US100 15 m muestra
 * las velas de ejemplo recién cuando el pedido falló, no mientras se pide (#577).
 */
export const exampleChart = () =>
  waitFor(() => expect(document.querySelector('[data-example]')).not.toBeNull())
