import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import App from '@/App'
import { BAR_VIEWS, MENU_GROUPS, NAV_SECTIONS, VIEWS } from '@/views/nav'

/** "Más" de la barra de abajo (en jsdom también se monta el de la barra lateral). */
const more = () => screen.getAllByRole('button', { name: /^Más vistas/ }).at(-1)!

describe('todas las vistas a dos toques (#618)', () => {
  afterEach(() => {
    history.replaceState(null, '', '/')
  })

  it('cada vista está en la barra de abajo o en "Más"', () => {
    const inMenu = MENU_GROUPS.flatMap((g) => [g.id, ...g.children])
    expect(new Set(inMenu)).toEqual(new Set(VIEWS.map((v) => v.id)))
    expect(inMenu).toHaveLength(VIEWS.length)
    for (const id of BAR_VIEWS) expect(NAV_SECTIONS).toContain(id)
  })

  it('"Más" → Panel abre el Panel y marca el Inicio', () => {
    render(<App initialStage="app" />)
    fireEvent.click(more())
    const dialog = screen.getByRole('dialog', { name: 'Todas las vistas' })
    // las vistas de adentro van bajo su sección
    const research = within(dialog).getByRole('link', { name: 'Investigación' }).closest('li')!
    for (const name of ['Performance', 'Propuestas', 'Liquidez', 'Glosario'])
      expect(within(research).getByRole('link', { name })).toBeInTheDocument()
    fireEvent.click(within(dialog).getByRole('link', { name: 'Panel' }))
    expect(location.hash).toBe('#panel')
    expect(screen.getByRole('heading', { level: 1, name: 'Panel' })).toBeInTheDocument()
    expect(screen.queryByRole('dialog', { name: 'Todas las vistas' })).toBeNull()
  })

  it('si la vista abierta no está en la barra, "Más" queda marcado', () => {
    history.replaceState(null, '', '/#noticias')
    render(<App initialStage="app" />)
    expect(more()).toHaveAccessibleName('Más vistas (la abierta está ahí)')
    fireEvent.click(more())
    const dialog = screen.getByRole('dialog', { name: 'Todas las vistas' })
    expect(within(dialog).getByRole('link', { name: 'Noticias' })).toHaveAttribute(
      'aria-current',
      'page',
    )
  })

  it('se cierra con Esc y "Buscar" abre la paleta', async () => {
    render(<App initialStage="app" />)
    fireEvent.click(more())
    fireEvent(
      screen.getByRole('dialog', { name: 'Todas las vistas' }),
      new Event('cancel', { cancelable: true }),
    )
    expect(screen.queryByRole('dialog', { name: 'Todas las vistas' })).toBeNull()
    fireEvent.click(more())
    fireEvent.click(screen.getByRole('button', { name: /Buscar un instrumento/ }))
    expect(await screen.findByRole('dialog', { name: 'Paleta de comandos' })).toBeInTheDocument()
    expect(screen.queryByRole('dialog', { name: 'Todas las vistas' })).toBeNull()
  })
})
