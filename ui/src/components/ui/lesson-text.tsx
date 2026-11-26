/**
 * El texto de una lección de la Academia (#799), como lo sirve el motor (`academy_lesson_doc.v1`).
 * Dibuja el árbol de `lib/lesson-markdown.ts` con elementos de React: nada de HTML inyectado (el
 * texto se escapa siempre). Los números van en mono (#708), los términos abren su definición del
 * glosario y las respuestas de «Probá antes de seguir» quedan plegadas (`<details>` nativo, con
 * teclado). El título de la lección (`# id título`) ya está en la página: no se repite.
 */
import { ChevronRight } from 'lucide-react'
import { Fragment, type CSSProperties, type ReactNode } from 'react'

import { ScrollRegion } from '@/components/ui/scroll-region'
import { Term } from '@/components/ui/term'
import { parseLesson, type Block, type Inline } from '@/lib/lesson-markdown'

/** Cifras (6.000 · 0,9 · 09:30 · −5 · +0,5) en mono, dentro del texto. */
const NUMBER = /([−+-]?\d+(?:[.,:]\d+)*)/

function Text({ v }: { v: string }) {
  const parts = v.split(NUMBER)
  if (parts.length === 1) return v
  return parts.map((p, i) =>
    i % 2 === 1 ? (
      <span key={i} className="font-mono text-[0.94em]">
        {p}
      </span>
    ) : (
      <Fragment key={i}>{p}</Fragment>
    ),
  )
}

/** El texto plano de un tramo (para nombrar una tabla). */
const plain = (c: Inline[]): string =>
  c.map((n) => (n.t === 'text' || n.t === 'code' ? n.v : plain(n.c))).join('')

/**
 * Las celdas de una tabla de la lección son texto: en sans (la regla global pone todo `td` en mono)
 * y con sus cifras en mono, como el resto del texto.
 */
const SANS: CSSProperties = { fontFamily: 'var(--font-sans)' }

function Inlines({ c }: { c: Inline[] }) {
  return c.map((n, i) => <InlineNode key={i} n={n} />)
}

function InlineNode({ n }: { n: Inline }): ReactNode {
  switch (n.t) {
    case 'text':
      return <Text v={n.v} />
    case 'strong':
      return (
        <strong className="font-bold">
          <Inlines c={n.c} />
        </strong>
      )
    case 'em':
      return (
        <em>
          <Inlines c={n.c} />
        </em>
      )
    case 'code':
      return <code className="rounded-xs bg-surface px-1 font-mono text-[0.9em]">{n.v}</code>
    case 'term':
      return (
        <Term id={n.id}>
          <Inlines c={n.c} />
        </Term>
      )
    case 'link':
      return (
        <a
          href={n.href}
          target="_blank"
          rel="noopener noreferrer"
          className="text-info underline decoration-from-font underline-offset-2"
        >
          <Inlines c={n.c} />
        </a>
      )
  }
}

/** `##` del texto es un `h4` en la página (el título de la lección es `h2` y la parte, `h3`). */
function Heading({ level, c }: { level: number; c: Inline[] }) {
  const content = <Inlines c={c} />
  if (level <= 2)
    return <h4 className="pt-2 text-[13px] font-extrabold tracking-[0.02em]">{content}</h4>
  return <h5 className="pt-1 text-[12.5px] font-bold">{content}</h5>
}

function Blocks({ c }: { c: Block[] }) {
  return c.map((b, i) => <BlockNode key={i} b={b} />)
}

function BlockNode({ b }: { b: Block }): ReactNode {
  switch (b.t) {
    case 'heading':
      return b.level === 1 ? null : <Heading level={b.level} c={b.c} />
    case 'p':
      return (
        <p>
          <Inlines c={b.c} />
        </p>
      )
    case 'list': {
      const items = b.items.map((it, i) => (
        <li key={i} className="pl-0.5">
          <Inlines c={it} />
        </li>
      ))
      return b.ordered ? (
        <ol start={b.start} className="grid list-decimal gap-1 pl-5 marker:font-mono">
          {items}
        </ol>
      ) : (
        <ul className="grid list-disc gap-1 pl-5">{items}</ul>
      )
    }
    case 'table':
      return (
        <ScrollRegion label={b.head.map(plain).join(' · ')} className="border border-border">
          <table className="w-full border-collapse text-left text-[12.5px]">
            <thead className="bg-surface">
              <tr>
                {b.head.map((h, i) => (
                  <th
                    key={i}
                    scope="col"
                    className="min-w-[7rem] border-b border-rule px-2 py-1.5 font-bold"
                  >
                    <Inlines c={h} />
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {b.rows.map((r, i) => (
                <tr key={i} className="border-b border-border last:border-b-0">
                  {r.map((cell, j) => (
                    <td key={j} style={SANS} className="min-w-[7rem] px-2 py-1.5 align-top">
                      <Inlines c={cell} />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </ScrollRegion>
      )
    case 'quote':
      return (
        <blockquote className="grid gap-2 border-l-2 border-primary bg-surface/60 py-2 pr-2 pl-3">
          <Blocks c={b.c} />
        </blockquote>
      )
    case 'details':
      return (
        <details className="group border border-border bg-card px-2.5 py-1.5">
          <summary className="flex min-h-8 cursor-pointer list-none items-center gap-1.5 font-semibold text-info [&::-webkit-details-marker]:hidden">
            <ChevronRight
              aria-hidden="true"
              className="size-3.5 shrink-0 transition-transform group-open:rotate-90"
            />
            <Inlines c={b.summary} />
          </summary>
          <div className="grid gap-2 pt-1.5 pb-1">
            <Blocks c={b.c} />
          </div>
        </details>
      )
    case 'code':
      return (
        <pre className="max-w-full overflow-x-auto border border-border bg-surface px-3 py-2 font-mono text-[12px] leading-relaxed">
          <code>{b.v}</code>
        </pre>
      )
    case 'rule':
      return <hr className="border-border" />
  }
}

/** El texto de la lección, sin HTML inyectado. */
export function LessonText({ markdown }: { markdown: string }) {
  return (
    <div
      data-lesson-text
      className="grid max-w-[78ch] min-w-0 gap-2.5 text-[13.5px] leading-relaxed"
    >
      <Blocks c={parseLesson(markdown)} />
    </div>
  )
}
