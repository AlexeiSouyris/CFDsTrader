/**
 * Diccionario interactivo (fase 10, #171): las definiciones salen de `docs/GLOSARIO.md`, que es la
 * única fuente. Se lee al compilar (`?raw`): la UI no copia ni reescribe ninguna definición.
 */
import source from '../../../docs/GLOSARIO.md?raw'

export type GlossaryEntry = { id: string; title: string; body: string; example: string }

/** Quita el formato de Markdown de una línea: negritas, cursivas y enlaces quedan como texto. */
const plain = (line: string) =>
  line
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/(^|\s)\*([^*]+)\*/g, '$1$2')
    .replace(/`([^`]+)`/g, '$1')
    .trim()

/** Términos del glosario: un `<a id>` seguido de un `###`. Las secciones (`##`) no son términos. */
export function parseGlossary(md: string): GlossaryEntry[] {
  const out: GlossaryEntry[] = []
  const blocks = md.split(/^<a id="([^"]+)"><\/a>\s*$/m)
  for (let i = 1; i < blocks.length; i += 2) {
    const id = blocks[i]
    const lines = blocks[i + 1].split('\n')
    const head = lines.findIndex((l) => l.trim() !== '')
    if (head < 0 || !lines[head].startsWith('### ')) continue
    const rest: string[] = []
    for (const l of lines.slice(head + 1)) {
      if (/^(---|## )/.test(l)) break
      rest.push(l)
    }
    const text = rest.join('\n').trim()
    const cut = text.indexOf('*Ejemplo:*')
    const body = (cut < 0 ? text : text.slice(0, cut)).trim()
    const example = cut < 0 ? '' : text.slice(cut + '*Ejemplo:*'.length).trim()
    out.push({
      id,
      title: plain(lines[head].slice(4)),
      body: body.split('\n').map(plain).filter(Boolean).join('\n'),
      example: example.split('\n').map(plain).filter(Boolean).join('\n'),
    })
  }
  return out
}

export const GLOSSARY = parseGlossary(source)
export const GLOSSARY_BY_ID = new Map(GLOSSARY.map((e) => [e.id, e]))
