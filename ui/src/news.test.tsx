import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { dateTimeUtc } from '@/lib/format'
import type { GatewayCalendar, GatewayNews } from '@/lib/news-contracts'
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

  it('con datos del motor: noticias reales sin sello, con su enlace y la interpretación tal cual (#600)', async () => {
    vi.stubGlobal('fetch', online())
    const { container } = render(<NewsView />)
    const list = screen.getByRole('navigation', { name: 'Noticias' })
    await waitFor(() => expect(list).toHaveTextContent(NEWS.items[0].item.title))
    expect(within(list).getAllByRole('button')).toHaveLength(2)
    // sin sello en las noticias ni en el calendario, y sin carpetas de ejemplo
    expect(container.querySelector('[data-example-badge]')).toBeNull()
    expect(screen.queryByRole('group', { name: 'Carpetas' })).toBeNull()
    const article = screen.getByRole('article')
    expect(article).toHaveTextContent(`Publicada${dateTimeUtc(NEWS.items[0].item.published_at)}`)
    expect(within(article).getByRole('link', { name: /Leer en Fed/ })).toHaveAttribute(
      'href',
      NEWS.items[0].item.url,
    )
    // la interpretación: el resumen y los números tal cual, como estimación
    expect(article).toHaveTextContent('El FOMC mantuvo la tasa.')
    const row = within(article).getByRole('row', { name: /US500/ })
    expect(row).toHaveTextContent('-0.15')
    expect(row).toHaveTextContent('0.95')
    expect(article).toHaveTextContent(/Puede equivocarse: no es un pronóstico/)
    // nota al margen, no una caja (#639)
    const note = within(article).getByRole('heading', {
      name: 'Interpretación automática',
    }).parentElement!
    expect(note.className).toMatch(/border-l-2/)
    expect(note.className).not.toMatch(/rounded/)
    // la otra todavía no se interpretó
    fireEvent.click(within(list).getByRole('button', { name: /BCE/ }))
    expect(screen.getByRole('article')).toHaveTextContent('Todavía sin interpretación automática.')
    // el calendario del motor, por hora, con las dos tablas
    const aside = screen.getByRole('complementary', { name: 'Contexto' })
    await waitFor(() => expect(aside).toHaveTextContent('Nóminas no agrícolas'))
    expect(aside).toHaveTextContent('Comunicado del FOMC')
    expect(aside).toHaveTextContent('US · impacto alto')
    expect(container.textContent).not.toMatch(/ganancia segura|garantiz|asegur/i)
  })

  it('si el motor responde sin noticias ni eventos, lo dice sin mostrar ejemplos', async () => {
    vi.stubGlobal(
      'fetch',
      online({ ...NEWS, items: [] }, { ...CALENDAR, economic_events: [], official_releases: [] }),
    )
    const { container } = render(<NewsView />)
    await screen.findByText('Todavía no hay noticias guardadas.')
    await screen.findByText(/Sin eventos guardados entre/)
    expect(container.querySelector('[data-example-badge]')).toBeNull()
  })

  it('"Cargar más" pide la página siguiente con next_before y la suma abajo (#605)', async () => {
    const [first, second] = NEWS.items
    const page1: GatewayNews = { ...NEWS, items: [first], next_before: '2026-11-24T19:00:00Z' }
    const page2: GatewayNews = { ...NEWS, items: [second], next_before: null }
    const fetchMock = vi.fn(async (url: string) => {
      const body = url.startsWith('/api/v1/news?before=')
        ? page2
        : url.startsWith('/api/v1/news')
          ? page1
          : url.startsWith('/api/v1/calendar')
            ? CALENDAR
            : []
      return new Response(JSON.stringify(body), { status: 200 })
    })
    vi.stubGlobal('fetch', fetchMock)
    render(<NewsView />)
    const list = await screen.findByRole('navigation', { name: 'Noticias' })
    await within(list).findByRole('button', { name: /FOMC statement/ })
    fireEvent.click(within(list).getByRole('button', { name: 'Cargar más' }))
    await within(list).findByRole('button', { name: /ECB speech/ })
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/v1/news?before=${encodeURIComponent('2026-11-24T19:00:00Z')}`,
      expect.anything(),
    )
    // sin `next_before`, no hay más páginas
    expect(within(list).queryByRole('button', { name: 'Cargar más' })).toBeNull()
  })

  it('sin next_before no se ofrece "Cargar más"', async () => {
    vi.stubGlobal('fetch', online())
    render(<NewsView />)
    const list = await screen.findByRole('navigation', { name: 'Noticias' })
    await within(list).findByRole('button', { name: /FOMC statement/ })
    expect(within(list).queryByRole('button', { name: 'Cargar más' })).toBeNull()
  })
})

const NEWS: GatewayNews = {
  as_of: '2026-11-25T12:00:00Z',
  generated_at: '2026-11-25T12:00:00Z',
  feed: null,
  next_before: null,
  items: [
    {
      item: {
        source: 'fed',
        feed: 'press_monetary',
        item_id: 'https://www.federalreserve.gov/a.htm',
        title: 'Federal Reserve issues FOMC statement',
        url: 'https://www.federalreserve.gov/a.htm',
        published_at: '2026-11-24T19:00:00Z',
        retrieved_at: '2026-11-24T19:03:00Z',
      },
      interpretations: [
        {
          interpretation_id: 'a'.repeat(64),
          source: 'fed',
          item_id: 'https://www.federalreserve.gov/a.htm',
          published_at: '2026-11-24T19:00:00Z',
          interpreted_at: '2026-11-24T19:10:00Z',
          interpreter: { name: 'ollama', version: 'p2.test', model: 'm', prompt_sha256: null },
          summary_es: 'El FOMC mantuvo la tasa.',
          impacts: [{ instrument_id: 'US500', sentiment: '-0.15', relevance: '0.95' }],
        },
      ],
    },
    {
      item: {
        source: 'ecb',
        feed: 'press',
        item_id: 'https://www.ecb.europa.eu/b.html',
        title: 'ECB speech',
        url: 'https://www.ecb.europa.eu/b.html',
        published_at: '2026-11-24T11:00:00Z',
        retrieved_at: '2026-11-24T11:02:00Z',
      },
      interpretations: [],
    },
  ],
}

const CALENDAR: GatewayCalendar = {
  from: '2026-11-24T12:00:00Z',
  to: '2026-12-09T12:00:00Z',
  as_of: '2026-11-25T12:00:00Z',
  generated_at: '2026-11-25T12:00:00Z',
  economic_events: [
    {
      source: 'forex_factory',
      event: 'Nóminas no agrícolas',
      country: 'US',
      instrument_id: null,
      impact: 'high',
      scheduled_at: '2026-12-04T13:30:00Z',
      published_at: '2026-11-23T10:00:00Z',
    },
  ],
  official_releases: [
    {
      source: 'fed_fomc',
      series: 'fomc_statement',
      reference_period: '2026-12-09',
      status: 'scheduled',
      scheduled_at: '2026-12-09T19:00:00Z',
      published_at: '2026-01-01T00:00:00Z',
      time_rule: 'release_page',
      with_press_conference: true,
      source_url: 'https://www.federalreserve.gov/x.htm',
    },
  ],
}

/** Con gateway: noticias y calendario del motor; el resto de las rutas, vacías. */
function online(news: GatewayNews = NEWS, calendar: GatewayCalendar = CALENDAR) {
  return vi.fn(async (url: string) => {
    const body = url.startsWith('/api/v1/news')
      ? news
      : url.startsWith('/api/v1/calendar')
        ? calendar
        : []
    return new Response(JSON.stringify(body), { status: 200 })
  }) as unknown as typeof fetch
}
