/**
 * Tipos de los contratos de la Academia (#709, #717) para la UI: `academy_catalog.v1`,
 * `academy_lesson_doc.v1`, `academy_progress.v1`, `academy_event.v1`, `quiz_answer.v1` y
 * `quiz_result.v1` de `contracts/`. Son la misma forma del contrato: si cambia el contrato, se
 * cambia acá. Las notas, el aprobado y qué herramienta está abierta los decide el motor.
 */

import type { ExamResult, ReplayKind } from '@/lib/replay-contracts'

/** Texto decimal (sin float) e instante UTC con zona, como en el contrato. */
type Decimal = string
type Utc = string

/** Id de una lección: "0.1", "12.4". */
export type LessonId = string
/** Id de una herramienta con candado: "tool.watchlist". */
export type ToolId = string

export type QuestionOption = { id: string; text: string }
/** Una pregunta del examen, sin la clave (la tiene el motor). */
export type Question =
  | { id: string; kind: 'choice'; prompt: string; options: QuestionOption[]; unit: null }
  | { id: string; kind: 'number'; prompt: string; options: null; unit: string | null }

export type Exam = {
  kind: 'quiz' | 'replay' | 'none'
  /** false: todavía sin preguntas; no se puede rendir (un examen de replay siempre está listo) */
  ready: boolean
  questions: Question[]
  /** el tipo de sesión de un examen de replay (#759); null u omitido en los demás */
  replay_kind?: Extract<ReplayKind, 'levels' | 'trades'> | null
}

export type Lesson = {
  id: LessonId
  title: string
  objective: string
  /** Markdown de la lección en `docs/academia/`, o null si todavía no está escrita */
  doc: string | null
  exam: Exam
  unlocks: ToolId[]
}

export type Level = { level: number; title: string; lessons: Lesson[] }

/**
 * `academy_lesson_doc.v1` (`GET /api/v1/academy/lessons/{lesson_id}/doc`, #798): el texto de la
 * lección en Markdown, leído y validado por el motor. Se muestra sin inyectar HTML (#799).
 */
export type LessonDoc = {
  lesson_id: LessonId
  doc: string
  catalog_version: string
  markdown: string
}

/** `academy_catalog.v1` (`GET /api/v1/academy/catalog`). */
export type AcademyCatalog = {
  catalog_version: string
  levels: Level[]
  tools: { id: ToolId; name: string; lesson_id: LessonId }[]
  thresholds: { quiz_pass_pct: Decimal; locked_tools_visible: boolean; provisional: boolean }
}

export type LessonProgress = {
  lesson_id: LessonId
  passed: boolean
  passed_at: Utc | null
  attempts: number
  best_score_pct: Decimal | null
}

export type ToolProgress =
  | { tool_id: ToolId; unlocked: true; by: 'lesson' | 'override'; lesson_id: LessonId }
  | { tool_id: ToolId; unlocked: false; by: null; lesson_id: LessonId }

/** `academy_progress.v1` (`GET /api/v1/academy/progress`): lo calcula el motor desde los eventos. */
export type AcademyProgress = {
  member_id: string
  catalog_version: string
  as_of: Utc
  lessons: LessonProgress[]
  tools: ToolProgress[]
  unlock_all: boolean
}

/** `academy_event.v1#/$defs/request` (`POST /api/v1/academy/events`). */
export type EventRequest =
  | { kind: 'lesson_completed'; lesson_id: LessonId }
  | { kind: 'tool_override'; tool_id: ToolId | '*'; action: 'unlock' | 'relock' }

/** `quiz_result.v1`: la corrección del motor (qué estuvo bien o mal, nunca la clave). */
export type QuizResult = {
  lesson_id: LessonId
  catalog_version: string
  score_pct: Decimal
  pass_pct: Decimal
  passed: boolean
  correct: number
  total: number
  questions: { question_id: string; correct: boolean }[]
  graded_at: Utc
}

/** `academy_event.v1`: el evento tal como lo guardó el motor. */
export type AcademyEvent = {
  seq: number
  member_id: string
  kind: 'lesson_completed' | 'exam_attempt' | 'tool_override'
  lesson_id: LessonId | null
  tool_id: ToolId | '*' | null
  action: 'unlock' | 'relock' | null
  quiz: QuizResult | null
  /** la calificación de un intento de examen de replay (#759); null u omitido en los demás */
  replay?: ExamResult | null
  catalog_version: string
  recorded_at: Utc
}

/** `quiz_answer.v1` (`POST /api/v1/academy/quiz/{lesson_id}`). */
export type QuizAnswer = {
  lesson_id: LessonId
  catalog_version: string
  answers: ({ question_id: string; choice: string } | { question_id: string; value: string })[]
}
