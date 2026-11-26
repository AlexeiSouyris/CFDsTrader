/**
 * Academia (#769, plan de `docs/design/2026-11-academia.md` §8, PR 4): el mapa de niveles con sus
 * lecciones, la página de cada lección (#770: objetivo, texto, examen y qué desbloquea; una lección
 * sin examen se completa con un evento) y el progreso, como lo manda el motor
 * (`academy_catalog.v1` y `academy_progress.v1`). La UI no decide si algo se aprobó ni qué
 * herramienta está abierta: cuenta y ordena lo que llega.
 *
 * Sin contenido todavía (#760): lo que no tiene texto ni preguntas lo dice («Contenido en
 * preparación»), sin inventar nada.
 */
import { Check, Circle, Lock, LockOpen } from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode, type RefObject } from 'react'

import { AcademyQuiz } from '@/components/ui/academy-quiz'
import type {
  AcademyCatalog,
  AcademyProgress,
  Lesson,
  LessonProgress,
  Level,
} from '@/lib/academy-contracts'
import { postEvent, useAcademy } from '@/lib/academy-api'
import { takeFocusedLesson } from '@/lib/academy-locks'
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
  return <Academy catalog={academy.catalog} progress={academy.progress} />
}

function Academy({ catalog, progress }: { catalog: AcademyCatalog; progress: AcademyProgress }) {
  const lessons = catalog.levels.flatMap((l) => l.lessons)
  const byLesson = new Map(progress.lessons.map((l) => [l.lesson_id, l]))
  // la que pidió un candado (#771), o la primera sin aprobar; elegir otra mueve el foco a su página
  const [selected, setSelected] = useState(
    () =>
      takeFocusedLesson() ??
      lessons.find((l) => !byLesson.get(l.id)?.passed)?.id ??
      lessons[0]?.id ??
      null,
  )
  const [moved, setMoved] = useState(false)
  const heading = useRef<HTMLHeadingElement>(null)
  useEffect(() => {
    if (!moved) return
    heading.current?.focus()
    heading.current?.scrollIntoView?.({ block: 'nearest' })
  }, [selected, moved])
  const level = catalog.levels.find((l) => l.lessons.some((x) => x.id === selected))
  const lesson = level?.lessons.find((x) => x.id === selected)
  return (
    <div data-section="hub" className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3">
      <ProgressStrip catalog={catalog} progress={progress} />
      {lessons.length > 0 && lessons.every(empty) && (
        <Preparing>
          Las lecciones todavía no tienen texto ni preguntas: el temario y lo que abre cada una ya
          están, el contenido llega nivel por nivel, con cada cifra verificada contra su fuente.
        </Preparing>
      )}
      <div className="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 lg:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
        <LevelMap
          levels={catalog.levels}
          progress={byLesson}
          selected={selected}
          onSelect={(id) => {
            setSelected(id)
            setMoved(true)
          }}
        />
        {level && lesson && (
          <LessonPage
            key={lesson.id}
            headingRef={heading}
            level={level}
            lesson={lesson}
            progress={byLesson.get(lesson.id)}
            status={statusText(lesson, byLesson.get(lesson.id))}
            catalog={catalog}
            tools={progress.tools}
          />
        )}
      </div>
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
  selected,
  onSelect,
}: {
  levels: Level[]
  progress: Map<string, LessonProgress>
  selected: string | null
  onSelect: (id: string) => void
}) {
  return (
    <nav aria-label="Mapa de niveles" className="panel min-w-0 self-start rounded-section">
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
                  <LessonRow
                    key={lesson.id}
                    lesson={lesson}
                    progress={progress.get(lesson.id)}
                    on={lesson.id === selected}
                    onSelect={() => onSelect(lesson.id)}
                  />
                ))}
              </ol>
            </li>
          )
        })}
      </ol>
    </nav>
  )
}

function LessonRow({
  lesson,
  progress,
  on,
  onSelect,
}: {
  lesson: Lesson
  progress?: LessonProgress
  on: boolean
  onSelect: () => void
}) {
  const passed = progress?.passed ?? false
  const Icon = passed ? Check : Circle
  return (
    <li>
      <button
        type="button"
        aria-current={on ? 'true' : undefined}
        onClick={onSelect}
        className={cn(
          'grid min-h-11 w-full grid-cols-[1rem_2.25rem_minmax(0,1fr)_auto] items-center gap-1.5 border-l-2 px-2.5 text-left text-[12.5px] hover:bg-surface sm:min-h-8',
          on ? 'border-primary bg-surface' : 'border-transparent',
        )}
      >
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
      </button>
    </li>
  )
}

function LessonPage({
  headingRef,
  level,
  lesson,
  progress,
  status,
  catalog,
  tools,
}: {
  headingRef: RefObject<HTMLHeadingElement | null>
  level: Level
  lesson: Lesson
  progress: LessonProgress | undefined
  /** el estado en una frase, del motor */
  status: string
  catalog: AcademyCatalog
  tools: AcademyProgress['tools']
}) {
  const names = new Map(catalog.tools.map((t) => [t.id, t.name]))
  const state = new Map(tools.map((t) => [t.tool_id, t]))
  const { exam } = lesson
  return (
    <article
      aria-labelledby="academia-leccion"
      className="panel grid min-w-0 content-start gap-3 rounded-section px-3 py-3 sm:px-4"
    >
      <header className="grid gap-1">
        <p className="text-[11px] font-bold tracking-[0.08em] text-muted-foreground uppercase">
          Nivel {level.level} · {level.title}
        </p>
        <h2
          id="academia-leccion"
          ref={headingRef}
          tabIndex={-1}
          className="text-[18px] leading-tight font-bold outline-none"
        >
          <span className="mr-2 font-mono text-accent">{lesson.id}</span>
          {lesson.title}
        </h2>
        <p className="text-[12px] text-muted-foreground">
          {progress?.passed && progress.passed_at
            ? `${status} el ${dateTimeUtc(progress.passed_at)}`
            : status}
          {progress && progress.attempts > 0 && ' · cada intento queda guardado'}
        </p>
      </header>

      <Part title="Objetivo">
        <p className="max-w-[68ch] text-[14px] leading-relaxed">{lesson.objective}</p>
      </Part>

      <Part title="Lección">
        {lesson.doc === null ? (
          <Preparing>
            El texto de esta lección todavía no está escrito. Cuando esté, se lee acá, con las
            cifras verificadas contra su fuente.
          </Preparing>
        ) : (
          <p className="text-[12.5px] text-muted-foreground">
            El texto está en <code className="font-mono">{lesson.doc}</code>.
          </p>
        )}
      </Part>

      <Part title="Examen">
        {exam.kind === 'none' ? (
          <Complete lesson={lesson} done={progress?.passed ?? false} />
        ) : !exam.ready ? (
          <Preparing>
            El examen todavía no tiene preguntas, así que no se puede rendir.
            {lesson.unlocks.length > 0 &&
              ' Mientras tanto, lo que desbloquea se puede abrir a mano en Ajustes.'}
          </Preparing>
        ) : exam.kind === 'replay' ? (
          <p className="text-[12.5px]">
            Examen con velas del pasado (replay): lo califica el motor, primero el proceso.
          </p>
        ) : (
          <>
            <p className="text-[12.5px]">
              {exam.questions.length} preguntas; para aprobar,{' '}
              {decimal(catalog.thresholds.quiz_pass_pct)} % de respuestas correctas
              {catalog.thresholds.provisional && ' (umbral provisorio)'}. Corrige el motor.
            </p>
            <AcademyQuiz lesson={lesson} version={catalog.catalog_version} />
          </>
        )}
      </Part>

      <Part title="Desbloquea">
        {lesson.unlocks.length === 0 ? (
          <p className="text-[12.5px] text-muted-foreground">No abre ninguna herramienta.</p>
        ) : (
          <ul className="grid gap-1 text-[12.5px]">
            {lesson.unlocks.map((id) => {
              const t = state.get(id)
              const Icon = t?.unlocked ? LockOpen : Lock
              return (
                <li key={id} className="flex items-center gap-2">
                  <Icon aria-hidden="true" className="size-3.5 text-muted-foreground" />
                  <span className="font-semibold">{names.get(id) ?? id}</span>
                  <span className="text-muted-foreground">
                    {!t?.unlocked
                      ? 'bloqueada'
                      : t.by === 'override'
                        ? 'abierta a mano'
                        : 'abierta por esta lección'}
                  </span>
                </li>
              )
            })}
          </ul>
        )}
      </Part>
    </article>
  )
}

/** Una lección sin examen: se marca como completada y el motor lo guarda como evento. */
function Complete({ lesson, done }: { lesson: Lesson; done: boolean }) {
  const [sending, setSending] = useState(false)
  const [error, setError] = useState<string | null>(null)
  if (done) return <p className="text-[12.5px]">Lección sin examen, ya completada.</p>
  return (
    <div className="grid gap-2">
      <p className="text-[12.5px]">Esta lección no tiene examen: se completa al leerla.</p>
      <button
        type="button"
        disabled={sending}
        onClick={() => {
          setSending(true)
          setError(null)
          postEvent({ kind: 'lesson_completed', lesson_id: lesson.id })
            .catch((e: unknown) =>
              setError(
                `No se pudo guardar en el motor (${e instanceof Error ? e.message : String(e)}).`,
              ),
            )
            .finally(() => setSending(false))
        }}
        className="min-h-9 justify-self-start rounded-sm bg-primary px-3 text-[12.5px] font-bold text-primary-foreground disabled:opacity-60"
      >
        {sending ? 'Guardando…' : 'Marcar como completada'}
      </button>
      {error && (
        <p role="alert" className="text-[12.5px] text-down">
          {error}
        </p>
      )}
    </div>
  )
}

function Part({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="grid gap-1.5 border-t border-border pt-2.5">
      <h3 className="text-[11px] font-extrabold tracking-[0.12em] uppercase">{title}</h3>
      {children}
    </section>
  )
}

/** «Contenido en preparación»: lo que todavía no existe, dicho sin rodeos. */
function Preparing({ children }: { children: ReactNode }) {
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
