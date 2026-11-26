/**
 * El Markdown de una lección de la Academia (#799, formato de `docs/academia/`, que valida el motor)
 * convertido en un árbol de bloques. Es un subconjunto a propósito: títulos, párrafos, negritas,
 * cursivas, código, enlaces, listas, tablas, citas, bloques de código y el plegable
 * `<details><summary>` de las respuestas («Probá antes de seguir», «Ver la solución»).
 *
 * **Sin HTML:** el árbol solo tiene texto y tipos conocidos; el componente que lo dibuja arma
 * elementos de React (que escapan el texto). Cualquier otra etiqueta HTML queda como texto. Un
 * enlace solo es enlace si va al glosario (`../GLOSARIO.md#id`) o es `https://`; si no, queda su
 * texto.
 */

export type Inline =
  | { t: 'text'; v: string }
  | { t: 'strong'; c: Inline[] }
  | { t: 'em'; c: Inline[] }
  | { t: 'code'; v: string }
  /** un término de `docs/GLOSARIO.md` */
  | { t: 'term'; id: string; c: Inline[] }
  /** un enlace externo, siempre `https://` */
  | { t: 'link'; href: string; c: Inline[] }

export type Block =
  | { t: 'heading'; level: number; c: Inline[] }
  | { t: 'p'; c: Inline[] }
  | { t: 'list'; ordered: boolean; start: number; items: Inline[][] }
  | { t: 'table'; head: Inline[][]; rows: Inline[][][] }
  | { t: 'quote'; c: Block[] }
  | { t: 'details'; summary: Inline[]; c: Block[] }
  | { t: 'code'; v: string }
  | { t: 'rule' }

const GLOSSARY = /^\.\.\/GLOSARIO\.md#([a-z0-9-]{1,64})$/
const HTTPS = /^https:\/\/[^\s"'<>`]+$/
const LINK = /^\[([^\]]+)\]\(([^()\s]+)\)/

/** Enlace del texto: al glosario, externo `https://`, o solo su texto. */
function link(text: string, href: string): Inline[] {
  const c = inline(text)
  const term = GLOSSARY.exec(href)
  if (term) return [{ t: 'term', id: term[1], c }]
  if (HTTPS.test(href)) return [{ t: 'link', href, c }]
  return c
}

/** El formato dentro de una línea: código, negritas, cursivas y enlaces. Lo demás es texto. */
export function inline(s: string): Inline[] {
  const out: Inline[] = []
  let text = ''
  const push = (...nodes: Inline[]) => {
    if (text) out.push({ t: 'text', v: text })
    text = ''
    out.push(...nodes)
  }
  let i = 0
  while (i < s.length) {
    const rest = s.slice(i)
    if (s[i] === '`') {
      const end = s.indexOf('`', i + 1)
      if (end > i + 1) {
        push({ t: 'code', v: s.slice(i + 1, end) })
        i = end + 1
        continue
      }
    }
    if (rest.startsWith('**')) {
      const end = s.indexOf('**', i + 2)
      if (end > i + 2) {
        push({ t: 'strong', c: inline(s.slice(i + 2, end)) })
        i = end + 2
        continue
      }
    }
    if (s[i] === '*' && /\S/.test(s[i + 1] ?? ' ')) {
      const end = s.indexOf('*', i + 1)
      if (end > i + 1 && s[end + 1] !== '*') {
        push({ t: 'em', c: inline(s.slice(i + 1, end)) })
        i = end + 1
        continue
      }
    }
    const m = s[i] === '[' ? LINK.exec(rest) : null
    if (m) {
      push(...link(m[1], m[2]))
      i += m[0].length
      continue
    }
    text += s[i]
    i += 1
  }
  if (text) out.push({ t: 'text', v: text })
  return out
}

const FENCE = /^```[\w-]*\s*$/
const HEADING = /^(#{1,4})\s+(.+)$/
const ITEM = /^(?:([-*])|(\d{1,3})\.)\s+(.*)$/
const DETAILS = /^<details>\s*<summary>(.*)<\/summary>\s*$/
const TABLE_RULE = /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/

/** Las celdas de una fila de tabla (`| a | b |`). */
const cells = (row: string) =>
  row
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((c) => inline(c.trim()))

/** ¿Esta línea empieza otro bloque (y corta un párrafo)? */
function startsBlock(lines: string[], i: number): boolean {
  const l = lines[i]
  return (
    FENCE.test(l) ||
    HEADING.test(l) ||
    ITEM.test(l) ||
    l.startsWith('>') ||
    DETAILS.test(l.trim()) ||
    (l.startsWith('|') && TABLE_RULE.test(lines[i + 1] ?? ''))
  )
}

/** Los bloques de un texto (o del interior de una cita o un plegable). */
export function blocks(lines: string[]): Block[] {
  const out: Block[] = []
  let i = 0
  while (i < lines.length) {
    const line = lines[i]
    const trimmed = line.trim()
    if (!trimmed) {
      i += 1
      continue
    }
    if (FENCE.test(line)) {
      const body: string[] = []
      i += 1
      while (i < lines.length && !/^```\s*$/.test(lines[i])) body.push(lines[i++])
      out.push({ t: 'code', v: body.join('\n') })
      i += 1
      continue
    }
    const details = DETAILS.exec(trimmed)
    if (details) {
      // hasta su </details>, contando los plegables de adentro
      let depth = 1
      let j = i + 1
      for (; j < lines.length; j += 1) {
        if (DETAILS.test(lines[j].trim())) depth += 1
        if (lines[j].trim() === '</details>') depth -= 1
        if (depth === 0) break
      }
      out.push({ t: 'details', summary: inline(details[1]), c: blocks(lines.slice(i + 1, j)) })
      i = j + 1
      continue
    }
    const heading = HEADING.exec(line)
    if (heading) {
      out.push({ t: 'heading', level: heading[1].length, c: inline(heading[2].trim()) })
      i += 1
      continue
    }
    if (/^(-{3,}|\*{3,})\s*$/.test(trimmed)) {
      out.push({ t: 'rule' })
      i += 1
      continue
    }
    if (line.startsWith('>')) {
      const inner: string[] = []
      while (i < lines.length && lines[i].startsWith('>')) {
        inner.push(lines[i].replace(/^> ?/, ''))
        i += 1
      }
      out.push({ t: 'quote', c: blocks(inner) })
      continue
    }
    if (line.startsWith('|') && TABLE_RULE.test(lines[i + 1] ?? '')) {
      const head = cells(line)
      const rows: Inline[][][] = []
      i += 2
      while (i < lines.length && lines[i].trim().startsWith('|')) rows.push(cells(lines[i++]))
      out.push({ t: 'table', head, rows })
      continue
    }
    const item = ITEM.exec(line)
    if (item) {
      const ordered = item[2] !== undefined
      const items: string[] = []
      while (i < lines.length) {
        const m = ITEM.exec(lines[i])
        if (m && (m[2] !== undefined) === ordered) {
          items.push(m[3])
        } else if (/^\s+\S/.test(lines[i]) && items.length > 0) {
          // la continuación de un ítem, con sangría
          items[items.length - 1] += ` ${lines[i].trim()}`
        } else {
          break
        }
        i += 1
      }
      out.push({
        t: 'list',
        ordered,
        start: ordered ? Number(item[2]) : 1,
        items: items.map((x) => inline(x.trim())),
      })
      continue
    }
    const para: string[] = []
    while (i < lines.length && lines[i].trim() && (para.length === 0 || !startsBlock(lines, i))) {
      para.push(lines[i].trim())
      i += 1
    }
    out.push({ t: 'p', c: inline(para.join(' ')) })
  }
  return out
}

/** El texto de una lección en bloques. */
export const parseLesson = (md: string): Block[] => blocks(md.replace(/\r\n?/g, '\n').split('\n'))
