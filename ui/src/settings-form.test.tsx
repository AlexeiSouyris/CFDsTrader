import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import App from '@/App'

describe('Ajustes como formulario por grupos (#647)', () => {
  it('cada grupo tiene título y para qué sirve, y los controles quedan dentro de su grupo', () => {
    render(<App initialStage="app" />)
    fireEvent.click(screen.getByRole('button', { name: 'Ajustes' }))
    // sin vidrio desde #708: el grupo del vidrio ya no está, el del tema sí
    expect(screen.queryByRole('heading', { name: /Vidrio/ })).toBeNull()
    const theme = screen.getByRole('heading', { name: 'Tema' }).closest('section')!
    expect(theme).toHaveTextContent(/La Terminal es siempre oscura/)
    expect(within(theme).getByRole('button', { name: /Pasar al tema/ })).toBeInTheDocument()
    const intro = screen.getByRole('heading', { name: 'Intro' }).closest('section')!
    expect(within(intro).getByRole('button', { name: 'Ver la intro otra vez' })).toBeInTheDocument()
  })
})
