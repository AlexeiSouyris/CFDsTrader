import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import App from '@/App'

describe('Ajustes como formulario por grupos (#647)', () => {
  it('cada grupo tiene título y para qué sirve, y los controles quedan dentro de su grupo', () => {
    render(<App initialStage="app" />)
    fireEvent.click(screen.getByRole('button', { name: 'Ajustes' }))
    const glass = screen
      .getByRole('heading', { name: 'Vidrio del Inicio y del Fondo' })
      .closest('section')!
    expect(glass).toHaveTextContent(/Solo se ve en el Inicio, el ingreso y el Fondo/)
    expect(within(glass).getByRole('group', { name: 'Vidrio predefinido' })).toBeInTheDocument()
    expect(within(glass).getByRole('slider', { name: 'Desenfoque' })).toBeInTheDocument()
    const intro = screen.getByRole('heading', { name: 'Intro' }).closest('section')!
    expect(within(intro).getByRole('button', { name: 'Ver la intro otra vez' })).toBeInTheDocument()
  })
})
