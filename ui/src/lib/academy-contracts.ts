/**
 * Tipos de los contratos de la Academia (#709, #717) para la UI: `academy_catalog.v1` y
 * `academy_progress.v1` de `contracts/`. Son la misma forma del contrato: si cambia el contrato, se
 * cambia acá. Las notas, el aprobado y qué herramienta está abierta los decide el motor.
 */

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
  /** false: todavía sin preguntas; no se puede rendir */
  ready: boolean
  questions: Question[]
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
