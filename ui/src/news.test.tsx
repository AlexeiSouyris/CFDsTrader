import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { dateTimeUtc } from '@/lib/format'
import { NEWS_EXAMPLE } from '@/mocks'
import { NewsView } from '@/views/news'

/** Sin gateway: cada ruta responde 404 (no son datos reales). */
const offline = () =>
  vi.fn(async () => new Response('ruta desconocida', { status: 404 })) as unknown as typeof fetch

afterEach(() => vi.unstubAllGlobals())

describe('Noticias y contexto (#581, etapa 7)', () => {
  it('es un lector: lista, artículo y columna de contexto, con el acento de la sección', () => {
    vi.stubGlobal('fetch', offline())
    const { container } = render(<NewsView />)
    expect(container.querySelector('[data-section="news"]')).not.toBeNull()
    expect(screen.getByRole('note')).toHaveTextContent('EJEMPLO')
    expect(
      within(screen.getByRole('navigation', { name: 'Noticias' })).getAllByRole('button'),
    ).toHaveLength(NEWS_EXAMPLE.length)
    expect(screen.getByRole('article')).toHaveTextContent(NEWS_EXAMPLE[0].title)
    expect(screen.getByRole('complementary', { name: 'Contexto' })).toHaveTextContent(
      /no es un pronóstico ni una recomendación/,
    )
  })

  it('cada noticia muestra su hora de publicación real en UTC y la hora en que se leyó', () => {
    vi.stubGlobal('fetch', offline())
    render(<NewsView />)
    const fomc = NEWS_EXAMPLE.find((n) => n.item_id === 'ej-fed-1119')!
    const item = screen.getByRole('button', { name: new RegExp(fomc.title) })
    expect(item.querySelector('time')).toHaveAttribute('dateTime', fomc.published_at)
    expect(item).toHaveTextContent(dateTimeUtc(fomc.published_at))
    fireEvent.click(item)
    expect(item).toHaveAttribute('aria-current', 'true')
    const article = screen.getByRole('article')
    expect(article).toHaveTextContent(fomc.title)
    expect(article).toHaveTextContent(`Publicada${dateTimeUtc(fomc.published_at)}`)
    expect(article).toHaveTextContent(`Leída por el sistema${dateTimeUtc(fomc.retrieved_at)}`)
  })

  it('las carpetas son filtros: solo cambian qué noticias se ven', () => {
    vi.stubGlobal('fetch', offline())
    render(<NewsView />)
    fireEvent.click(screen.getByRole('button', { name: 'Mi carpeta' }))
    expect(screen.getByRole('button', { name: 'Mi carpeta' })).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    const mine = NEWS_EXAMPLE.filter((n) => n.folders.includes('alexei'))
    const list = screen.getByRole('navigation', { name: 'Noticias' })
    expect(within(list).getAllByRole('button')).toHaveLength(mine.length)
    expect(screen.getByRole('article')).toHaveTextContent(mine[0].title)
  })

  it('el calendario lleva el sello EJEMPLO y ningún texto promete ganancias', () => {
    vi.stubGlobal('fetch', offline())
    const { container } = render(<NewsView />)
    const aside = screen.getByRole('complementary', { name: 'Contexto' })
    expect(within(aside).getByText(/Calendario/)).toHaveTextContent('EJEMPLO')
    expect(container.textContent).not.toMatch(/ganancia segura|garantiz|asegur/i)
  })
})
