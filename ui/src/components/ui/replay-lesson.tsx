/**
 * El examen de replay de una lección (#826, diseño §5): se empieza desde la página de la lección
 * (`POST /api/v1/academy/replay` con `lesson_id` y el `replay_kind` del catálogo), se rinde en la
 * vista Replay y, al terminar, se muestra si la lección quedó aprobada **según el progreso del
 * motor** (`academy_progress.v1`), no según lo que crea la UI. El motor guarda cada intento (#759).
 */
import { ArrowUpRight } from 'lucide-react'
import { useState } from 'react'

import { reloadAcademy, useAcademy } from '@/lib/academy-api'
import type { Exam, Lesson } from '@/lib/academy-contracts'
import { focusLesson } from '@/lib/academy-locks'
import { dateTimeUtc } from '@/lib/format'
import { GatewayError } from '@/lib/paper-api'
import { fresh, kept, keep, openReplay } from '@/lib/replay-api'

type ExamKind = NonNullable<Exam['replay_kind']>

/** Qué se hace en cada tipo de examen de replay, en una frase. */
const WHAT: Record<ExamKind, string> = {
  levels: 'marcar soportes y resistencias; el motor los compara con los suyos',
  pivots:
    'marcar máximos y mínimos locales después de su confirmación; el motor los compara con sus pivotes',
  trades: 'operar con stop, tamaño y plan escrito; se califica el proceso, no si ganó',
}

const ERRORS: Record<number, string> = {
  400: 'El motor dice que esta lección no tiene un examen de replay de ese tipo (¿cambió el catálogo?).',
  403: 'El gateway rechazó el pedido: falta el token local (CFDS_GATEWAY_TOKEN en el proxy).',
  503: 'El motor no pudo armar una sesión con velas completas ahora. Probá de nuevo.',
}

const primary =
  'min-h-9 justify-self-start rounded-sm bg-primary px-3 text-[12.5px] font-bold text-primary-foreground disabled:opacity-60'

/** Ir a la página de una lección en la Academia. */
function LessonLink({ id, children }: { id: string; children: string }) {
  return (
    <a
      href="#academia"
      onClick={() => focusLesson(id)}
      className="inline-flex items-center gap-1 font-semibold underline underline-offset-2"
    >
      {children}
      <ArrowUpRight aria-hidden="true" className="size-3.5" />
    </a>
  )
}

/**
 * La parte «Examen» de una lección con examen de replay: qué se hace y el botón para empezar. La
 * sesión abierta queda guardada (`keep`) y se sigue en la vista Replay.
 */
export function ReplayExamStart({ lesson }: { lesson: Lesson }) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const kind = lesson.exam.replay_kind
  if (!kind) {
    return (
      <p role="note" className="text-[12.5px] text-muted-foreground">
        Examen con velas del pasado (replay), pero el catálogo del motor no dice de qué tipo: no se
        puede empezar desde acá.
      </p>
    )
  }
  const open = kept?.session.lesson_id === lesson.id && kept.result === null
  const start = () => {
    setBusy(true)
    setError(null)
    openReplay({ kind, lesson_id: lesson.id })
      .then((s) => {
        keep(fresh(s))
        location.hash = '#replay'
      })
      .catch((e: unknown) => {
        setError(
          (e instanceof GatewayError && ERRORS[e.status]) ||
            `No se pudo hablar con el motor (${e instanceof Error ? e.message : String(e)}).`,
        )
      })
      .finally(() => setBusy(false))
  }
  return (
    <div className="grid gap-2">
      <p className="max-w-[68ch] text-[12.5px] leading-relaxed">
        Examen con velas del pasado (replay): vas a {WHAT[kind]}. El motor elige instrumento,
        timeframe y fecha (oculta hasta el final, solo del período de investigación) y califica al
        terminar, primero el proceso. Cada intento queda guardado y la lección se aprueba con el
        proceso aprobado.
      </p>
      {open ? (
        <a href="#replay" className={primary + ' inline-flex items-center'}>
          Seguir el examen abierto
        </a>
      ) : (
        <button type="button" disabled={busy} onClick={start} className={primary}>
          {busy ? 'Pidiendo la sesión…' : 'Empezar el examen'}
        </button>
      )}
      {error && (
        <p role="alert" className="text-[12.5px] text-down">
          {error}
        </p>
      )}
    </div>
  )
}

/** Arriba de la sesión: de qué lección es el examen y que cuenta para el progreso. */
export function LessonExamBanner({ lessonId }: { lessonId: string }) {
  const academy = useAcademy()
  const title =
    academy.status === 'ok'
      ? academy.catalog.levels.flatMap((l) => l.lessons).find((l) => l.id === lessonId)?.title
      : undefined
  return (
    <p className="panel flex flex-wrap items-baseline gap-x-3 gap-y-1 px-2.5 py-2 text-[12.5px]">
      <strong>
        Examen de la lección {lessonId}
        {title && ` · ${title}`}
      </strong>
      <span className="text-muted-foreground">
        Cuenta para tu progreso: al terminar, el motor guarda el intento.
      </span>
      <LessonLink id={lessonId}>Ver la lección</LessonLink>
    </p>
  )
}

/**
 * Después de calificar: qué dice el **progreso** del motor de la lección (aprobada o no, intentos).
 * Si el progreso es de antes de este intento (no se pudo volver a pedir), lo dice y deja pedirlo.
 */
export function LessonVerdict({
  lessonId: id,
  gradedAt,
}: {
  lessonId: string
  /** `exam_result.v1#/properties/graded_at`: cuándo calificó el motor este intento */
  gradedAt: string
}) {
  const academy = useAcademy()
  const [asking, setAsking] = useState(false)
  const box = 'panel grid gap-1.5 px-3 py-2.5 text-[12.5px]'
  if (academy.status !== 'ok') {
    return (
      <section aria-label={`Progreso de la lección ${id}`} className={box}>
        <p role="status" className="text-muted-foreground">
          {academy.status === 'loading'
            ? 'Pidiendo el progreso al motor…'
            : `No se pudo leer el progreso del motor (${academy.message}). El intento quedó guardado igual.`}
        </p>
        <LessonLink id={id}>Volver a la lección</LessonLink>
      </section>
    )
  }
  const p = academy.progress.lessons.find((l) => l.lesson_id === id)
  const names = new Map(academy.catalog.tools.map((t) => [t.id, t.name]))
  const opened = academy.progress.tools.filter((t) => t.lesson_id === id && t.unlocked)
  // las dos horas son del motor: si el progreso es de antes de la nota, todavía no la incluye
  const stale = Date.parse(academy.progress.as_of) < Date.parse(gradedAt)
  return (
    <section aria-label={`Progreso de la lección ${id}`} className={box}>
      {stale ? (
        <>
          <p role="status">
            El progreso del motor es de antes de este intento: todavía no dice si la lección quedó
            aprobada.
          </p>
          <button
            type="button"
            disabled={asking}
            onClick={() => {
              setAsking(true)
              void reloadAcademy().finally(() => setAsking(false))
            }}
            className="min-h-9 justify-self-start rounded-sm border border-border px-3 text-[12.5px] font-semibold hover:bg-surface disabled:opacity-60"
          >
            {asking ? 'Pidiendo…' : 'Pedir el progreso otra vez'}
          </button>
        </>
      ) : p?.passed ? (
        <>
          <p role="status" className="font-bold">
            Lección {id} aprobada, según el progreso del motor
            {p.passed_at && ` (el ${dateTimeUtc(p.passed_at)})`}.
          </p>
          {opened.length > 0 && (
            <p className="text-muted-foreground">
              Herramientas abiertas:{' '}
              {opened.map((t) => names.get(t.tool_id) ?? t.tool_id).join(', ')}.
            </p>
          )}
        </>
      ) : (
        <p role="status" className="font-bold">
          Lección {id} todavía no aprobada, según el progreso del motor
          {p
            ? ` · ${p.attempts} ${p.attempts === 1 ? 'intento guardado' : 'intentos guardados'}`
            : ''}
          .
        </p>
      )}
      <LessonLink id={id}>Volver a la lección</LessonLink>
    </section>
  )
}
