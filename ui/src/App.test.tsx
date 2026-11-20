import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import App from '@/App'
import { Gallery } from '@/components/gallery/Gallery'

describe('prototipo de la interfaz', () => {
  it('avisa siempre que los datos son de ejemplo', () => {
    render(<App />)
    expect(screen.getByRole('note')).toHaveTextContent('datos de ejemplo')
  })

  it('muestra un mensaje cuando todavía no hay componentes', () => {
    render(<Gallery entries={[]} />)
    expect(screen.getByText(/Todavía no hay componentes/)).toBeInTheDocument()
  })

  it('muestra cada componente con su título, para qué sirve y su origen', () => {
    render(
      <Gallery
        entries={[
          {
            id: 'demo',
            title: 'Tarjeta de prueba',
            purpose: 'Probar la galería',
            source: 'https://21st.dev',
            Demo: () => <p>contenido de ejemplo</p>,
          },
        ]}
      />,
    )
    expect(screen.getByRole('heading', { name: 'Tarjeta de prueba' })).toBeInTheDocument()
    expect(screen.getByText('Probar la galería')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Ver original' })).toHaveAttribute(
      'href',
      'https://21st.dev',
    )
    expect(screen.getByText('contenido de ejemplo')).toBeInTheDocument()
  })
})
