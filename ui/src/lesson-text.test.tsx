/**
 * El texto de la lección (#799): el Markdown de `docs/academia/` (formato que valida el motor)
 * convertido en elementos de React, **sin HTML inyectado**. Se prueba el convertidor, los textos
 * reales de las lecciones y que nada de HTML ni de `javascript:` se vuelva un elemento.
 */
import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import catalogFixture from '../e2e/fixtures/academy-catalog.json'
import docFixture from '../e2e/fixtures/academy-lesson-doc.json'
import progressFixture from '../e2e/fixtures/academy-progress.json'
import { LessonText } from '@/components/ui/lesson-text'
import { resetAcademy } from '@/lib/academy-api'
import type { AcademyCatalog, AcademyProgress, LessonDoc } from '@/lib/academy-contracts'
import { parseLesson } from '@/lib/lesson-markdown'
import { AcademyView } from '@/views/academy'

const LESSONS = import.meta.glob('../../docs/academia/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const SAMPLE = `# 9.9 Ejemplo

> **Nivel 9.** Educativo: no es consejo de inversión.

## La idea

Una línea
que sigue, con **negrita**, *cursiva*, \`código\` y el [spread](../GLOSARIO.md#spread).

- uno
- dos, que sigue
  en otra línea
1. no es de la lista de arriba

3. tres
4. cuatro

| Botón | Precio |
|---|---|
| **BUY** | 6.030 |

\`\`\`text
costo = noches × precio
\`\`\`

> **Probá antes de seguir:** ¿podés?
>
> <details><summary>Ver la respuesta</summary>
>
> No.
>
> </details>

<details><summary>Ver la solución</summary>

Sí, con [ESMA](https://www.esma.europa.eu/).

</details>
`

describe('convertidor del texto de la lección (#799)', () => {
  it('arma títulos, párrafos, listas, tablas, código, citas y plegables', () => {
    const b = parseLesson(SAMPLE)
    expect(b.map((x) => x.t)).toEqual([
      'heading',
      'quote',
      'heading',
      'p',
      'list',
      'list',
      'list',
      'table',
      'code',
      'quote',
      'details',
    ])
    const p = b[3]
    expect(p.t === 'p' && p.c.map((n) => n.t)).toEqual([
      'text',
      'strong',
      'text',
      'em',
      'text',
      'code',
      'text',
      'term',
      'text',
    ])
    const [ul, one, ol] = [b[4], b[5], b[6]]
    expect(ul.t === 'list' && !ul.ordered && ul.items.length).toBe(2)
    expect(ul.t === 'list' && ul.items[1]).toEqual([
      { t: 'text', v: 'dos, que sigue en otra línea' },
    ])
    expect(one.t === 'list' && one.ordered && one.start).toBe(1)
    expect(ol.t === 'list' && ol.ordered && [ol.start, ol.items.length]).toEqual([3, 2])
    const quote = b[9]
    expect(quote.t === 'quote' && quote.c.map((x) => x.t)).toEqual(['p', 'details'])
    const details = b[10]
    expect(details.t === 'details' && details.c[0]).toEqual({
      t: 'p',
      c: [
        { t: 'text', v: 'Sí, con ' },
        { t: 'link', href: 'https://www.esma.europa.eu/', c: [{ t: 'text', v: 'ESMA' }] },
        { t: 'text', v: '.' },
      ],
    })
  })

  it('el HTML queda como texto y solo hay enlaces al glosario o https', () => {
    const evil = [
      '# 9.9 Malo',
      '',
      '<script>alert(1)</script>',
      '',
      'Una <img src=x onerror="alert(1)"> y un <iframe src="https://x.test"></iframe>.',
      '',
      '[clic](javascript:void0) [otro](http://sin-tls.test) [rel](../analysis/x.md) [data](data:text/html,hola)',
      '',
      '[paréntesis](javascript:alert(1))',
      '',
      '<details open ontoggle="alert(1)"><summary>no es el plegable del formato</summary>',
      '',
      '| <b>a</b> | b |',
      '|---|---|',
      '| <svg onload=alert(1)> | `<i>`code`</i>` |',
    ].join('\n')
    const { container } = render(<LessonText markdown={evil} />)
    for (const tag of ['script', 'img', 'iframe', 'svg', 'b', 'i', 'details']) {
      expect(container.querySelector(tag), tag).toBeNull()
    }
    expect(container.querySelectorAll('a')).toHaveLength(0)
    for (const el of container.querySelectorAll('*')) {
      const names = el.getAttributeNames()
      expect(names.filter((n) => n.startsWith('on') || n === 'href' || n === 'src')).toEqual([])
    }
    expect(container).toHaveTextContent('<script>alert(1)</script>')
    expect(container).toHaveTextContent('Una <img src=x onerror="alert(1)"> y un')
    expect(container).toHaveTextContent('clic otro rel data')
    expect(container).toHaveTextContent('[paréntesis](javascript:alert(1))')
  })

  it('los plegables se abren y se cierran; los números van en mono', () => {
    render(<LessonText markdown={SAMPLE} />)
    const answer = screen.getByText('Ver la respuesta').closest('details')!
    expect(answer).not.toHaveAttribute('open')
    fireEvent.click(screen.getByText('Ver la respuesta'))
    expect(answer).toHaveAttribute('open')
    // el título de la lección ya está en la página: no se repite
    expect(screen.queryByText(/Ejemplo/)).toBeNull()
    expect(screen.getByRole('heading', { level: 4, name: 'La idea' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'spread' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'ESMA' })).toHaveAttribute('rel', 'noopener noreferrer')
    const cell = screen.getByRole('cell', { name: '6.030' })
    expect(within(cell).getByText('6.030')).toHaveClass('font-mono')
  })

  it('cada lección real se lee entera: secciones, tablas y respuestas plegadas', () => {
    const files = Object.entries(LESSONS)
    expect(files.length).toBeGreaterThanOrEqual(10)
    for (const [file, md] of files) {
      const { container, unmount } = render(<LessonText markdown={md} />)
      const count = (re: RegExp) => md.match(re)?.length ?? 0
      expect(container.querySelectorAll('details'), file).toHaveLength(count(/<details>/g))
      expect(container.querySelectorAll('table'), file).toHaveLength(count(/^\|---/gm))
      expect(container.querySelectorAll('h4'), file).toHaveLength(count(/^## /gm))
      expect(container.querySelectorAll('pre'), file).toHaveLength(count(/^```\w/gm))
      // ningún resto de Markdown ni de HTML a la vista
      expect(container.textContent, file).not.toMatch(/<\/?(details|summary)>|\*\*|\]\(/)
      unmount()
    }
  })
})

const CATALOG = catalogFixture as AcademyCatalog
const PROGRESS = progressFixture as AcademyProgress
const DOC = docFixture as LessonDoc

/** Gateway falso de prueba: catálogo, progreso y el texto de la lección (o un error). */
function gateway(doc: () => Response) {
  const fetch = vi.fn(async (input: string | URL | Request) => {
    const path = new URL(String(input), 'http://ui.test').pathname
    if (path === '/api/v1/academy/catalog') return Response.json(CATALOG)
    if (path === '/api/v1/academy/progress') return Response.json(PROGRESS)
    if (path === `/api/v1/academy/lessons/${DOC.lesson_id}/doc`) return doc()
    return new Response('sin ruta', { status: 404 })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

beforeEach(() => resetAcademy())
afterEach(() => vi.unstubAllGlobals())

describe('página de la lección con su texto (#799)', () => {
  it('pide el texto al motor y lo muestra, sin la ruta del archivo', async () => {
    const fetch = gateway(() => Response.json(DOC))
    render(<AcademyView />)
    const page = await screen.findByRole('article', { name: /Qué es un mercado/ })
    expect(await within(page).findByRole('heading', { name: 'La idea' })).toBeVisible()
    expect(fetch).toHaveBeenCalledWith('/api/v1/academy/lessons/0.1/doc', expect.anything())
    expect(page).not.toHaveTextContent(DOC.doc)
    const answer = within(page).getByText('Ver la respuesta').closest('details')!
    fireEvent.click(within(answer).getByText('Ver la respuesta'))
    expect(answer).toHaveAttribute('open')
    expect(within(page).getAllByRole('table')).toHaveLength(2)
  })

  it('si el motor no lo manda, lo dice y deja la ruta del repo', async () => {
    gateway(() => new Response('sin texto', { status: 404 }))
    render(<AcademyView />)
    const page = await screen.findByRole('article', { name: /Qué es un mercado/ })
    expect(await within(page).findByRole('alert')).toHaveTextContent(
      `No se pudo leer el texto del motor (el gateway respondió 404: sin texto). Está en el repo, en ${DOC.doc}.`,
    )
  })
})
