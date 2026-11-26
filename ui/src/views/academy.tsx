/**
 * Academia (#769, plan de `docs/design/2026-11-academia.md` §8, PR 4): el mapa de niveles con sus
 * lecciones y el progreso, como lo manda el motor (`academy_catalog.v1` y `academy_progress.v1`).
 * La UI no decide si algo se aprobó ni qué herramienta está abierta: cuenta y ordena lo que llega.
 *
 * Sin contenido todavía (#760): lo que no tiene texto ni preguntas lo dice («Contenido en
 * preparación»), sin inventar nada.
 */
import { Check, Circle } from 'lucide-react'
import type { ReactNode } from 'react'

import type {
  AcademyCatalog,
  AcademyProgress,
  Lesson,
  LessonProgress,
  Level,
} from '@/lib/academy-contracts'
import { useAcademy } from '@/lib/academy-api'
import { dateTimeUtc, decimal } from '@/lib/format'
import { cn } from '@/lib/utils'

/** El estado de una lección en una frase corta, tal como lo dejó el motor. */
function statusText(lesson: Lesson, p: LessonProgress | undefined): string {
  if (p?.passed) return lesson.exam.kind === 'none' ? 'Completada' : 'Aprobada'
  if (lesson.exam.kind === 'none') return 'Sin examen'
  if (p && p.attempts > 0) {
    const best = p.best_score_pct === null ? '' : ` · mejor ${decimal(p.best_score_pct)} %`
    return `${p.attempts} ${p.attempts === 1 ? 'intento' : 'intentos'}${best}`
  }
  return lesson.exam.ready ? 'Pendiente' : 'En preparación'
}

// sin texto y sin examen que se pueda rendir: todavía no hay nada para estudiar
const empty = (l: Lesson) => l.doc === null && (l.exam.kind === 'none' || !l.exam.ready)

export function AcademyView() {
  const academy = useAcademy()
  if (academy.status === 'loading')
    return <Quiet role="status">Pidiendo la Academia al motor…</Quiet>
  if (academy.status === 'error') {
    return (
      <Quiet role="alert">
        {`No se pudo leer la Academia del motor (${academy.message}). Sin el motor no hay lecciones, notas ni progreso: no se muestra nada inventado.`}
      </Quiet>
    )
  }
  const { catalog, progress } = academy
  const lessons = catalog.levels.flatMap((l) => l.lessons)
  const byLesson = new Map(progress.lessons.map((l) => [l.lesson_id, l]))
  return (
    <div data-section="hub" className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
      <ProgressStrip catalog={catalog} progress={progress} />
      {lessons.length > 0 && lessons.every(empty) && (
        <Preparing>
          Las lecciones todavía no tienen texto ni preguntas: el temario y lo que abre cada una ya
          están, el contenido llega nivel por nivel, con cada cifra verificada contra su fuente.
        </Preparing>
      )}
      <LevelMap levels={catalog.levels} progress={byLesson} />
    </div>
  )
}

function ProgressStrip({
  catalog,
  progress,
}: {
  catalog: AcademyCatalog
  progress: AcademyProgress
}) {
  const passed = progress.lessons.filter((l) => l.passed).length
  const open = progress.tools.filter((t) => t.unlocked).length
  const { thresholds: t } = catalog
  return (
    <section aria-label="Progreso" className="panel rounded-section px-2.5 py-2">
      <dl className="flex flex-wrap gap-x-6 gap-y-1.5 text-[12px]">
        <Stat label="Lecciones aprobadas">
          {passed} / {progress.lessons.length}
        </Stat>
        <Stat label="Herramientas abiertas">
          {open} / {progress.tools.length}
          {progress.unlock_all && <span className="ml-1.5 font-sans">(todas a mano)</span>}
        </Stat>
        <Stat label="Para aprobar un examen">
          {decimal(t.quiz_pass_pct)} % de respuestas
          {t.provisional && <span className="ml-1.5 font-sans text-accent">provisorio</span>}
        </Stat>
        <Stat label="Calculado por el motor">
          <time dateTime={progress.as_of}>{dateTimeUtc(progress.as_of)}</time>
        </Stat>
      </dl>
    </section>
  )
}

function Stat({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline gap-2">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-mono font-semibold">{children}</dd>
    </div>
  )
}

function LevelMap({
  levels,
  progress,
}: {
  levels: Level[]
  progress: Map<string, LessonProgress>
}) {
  return (
    <section aria-label="Mapa de niveles" className="panel min-w-0 self-start rounded-section">
      <ol>
        {levels.map((level) => {
          const done = level.lessons.filter((l) => progress.get(l.id)?.passed).length
          return (
            <li key={level.level} className="border-b border-border last:border-b-0">
              <h2 className="flex items-baseline gap-2 px-2.5 pt-2 pb-1">
                <span className="font-mono text-[11px] font-bold text-accent">N{level.level}</span>
                <span className="min-w-0 flex-1 text-[12px] font-bold">{level.title}</span>
                <span className="font-mono text-[11px] text-muted-foreground">
                  {done}/{level.lessons.length}
                </span>
              </h2>
              <ol className="pb-1.5">
                {level.lessons.map((lesson) => (
                  <LessonRow key={lesson.id} lesson={lesson} progress={progress.get(lesson.id)} />
                ))}
              </ol>
            </li>
          )
        })}
      </ol>
    </section>
  )
}

function LessonRow({ lesson, progress }: { lesson: Lesson; progress?: LessonProgress }) {
  const passed = progress?.passed ?? false
  const Icon = passed ? Check : Circle
  return (
    <li className="grid min-h-8 grid-cols-[1rem_2.25rem_minmax(0,1fr)_auto] items-center gap-1.5 px-2.5 text-[12.5px]">
      <Icon
        aria-hidden="true"
        className={cn('size-3.5', passed ? 'text-accent' : 'text-muted-foreground')}
      />
      <span className="font-mono text-[11px] text-muted-foreground">{lesson.id}</span>
      <span className="truncate font-semibold">{lesson.title}</span>
      <span className="text-[11px] text-muted-foreground">
        {statusText(lesson, progress)}
        {passed && progress?.passed_at && (
          <span className="sr-only"> el {dateTimeUtc(progress.passed_at)}</span>
        )}
      </span>
    </li>
  )
}

/** «Contenido en preparación»: lo que todavía no existe, dicho sin rodeos. */
export function Preparing({ children }: { children: ReactNode }) {
  return (
    <div
      role="note"
      className="grid max-w-[80ch] gap-1 border border-dashed border-border px-3 py-2.5"
    >
      <strong className="text-[12.5px]">Contenido en preparación</strong>
      <p className="text-[12.5px] leading-relaxed text-muted-foreground">{children}</p>
    </div>
  )
}

function Quiet({ children, role }: { children: string; role: 'status' | 'alert' }) {
  return (
    <p
      role={role}
      className="max-w-[68ch] border border-dashed border-border px-4 py-6 text-[13px] leading-relaxed text-muted-foreground"
    >
      {children}
    </p>
  )
}
