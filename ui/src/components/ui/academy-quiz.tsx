/**
 * Examen de preguntas de una lección (#781): las respuestas van al motor (`quiz_answer.v1`) y la
 * corrección que vuelve (`quiz_result.v1`) se muestra tal cual: nota, umbral, aprobado y qué
 * pregunta estuvo bien o mal. La UI no corrige ni sabe las claves. Cada intento queda guardado.
 */
import { Check, X } from 'lucide-react'
import { useState } from 'react'

import { postQuiz, reloadAcademy } from '@/lib/academy-api'
import type { Lesson, QuizAnswer, QuizResult } from '@/lib/academy-contracts'
import { dateTimeUtc, decimal } from '@/lib/format'
import { GatewayError } from '@/lib/paper-api'
import { cn } from '@/lib/utils'

/** Un número escrito a mano como decimal del contrato ("-1,5" → "-1.5"), o null. Solo lee texto. */
function numberAnswer(text: string): string | null {
  const t = text.trim().replace(',', '.')
  return /^-?\d{1,12}(\.\d{1,8})?$/.test(t) ? t : null
}

const errorText = (e: unknown) =>
  e instanceof GatewayError && e.status === 409
    ? 'El examen cambió en el motor mientras respondías (o ya no se puede rendir): se recargó. Revisá las preguntas y enviá de nuevo.'
    : e instanceof GatewayError && e.status === 403
      ? 'El gateway rechazó el envío: falta el token local de escritura (CFDS_GATEWAY_TOKEN en el proxy).'
      : `No se pudo enviar al motor (${e instanceof Error ? e.message : String(e)}).`

export function AcademyQuiz({ lesson, version }: { lesson: Lesson; version: string }) {
  const [answers, setAnswers] = useState<Record<string, string>>({})
  const [sending, setSending] = useState(false)
  const [result, setResult] = useState<QuizResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const questions = lesson.exam.questions
  const set = (id: string, value: string) => setAnswers((a) => ({ ...a, [id]: value }))
  const bad = questions.filter(
    (q) => q.kind === 'number' && answers[q.id]?.trim() && numberAnswer(answers[q.id]) === null,
  )
  const missing = questions.filter((q) => !answers[q.id]?.trim()).length
  const graded = new Map(result?.questions.map((q) => [q.question_id, q.correct]))

  const submit = async () => {
    const body: QuizAnswer = {
      lesson_id: lesson.id,
      catalog_version: version,
      answers: questions.flatMap((q): QuizAnswer['answers'] => {
        const a = answers[q.id]?.trim()
        if (!a) return []
        if (q.kind === 'choice') return [{ question_id: q.id, choice: a }]
        const value = numberAnswer(a)
        return value === null ? [] : [{ question_id: q.id, value }]
      }),
    }
    setSending(true)
    setError(null)
    try {
      setResult(await postQuiz(body))
    } catch (e) {
      setResult(null)
      setError(errorText(e))
      if (e instanceof GatewayError && e.status === 409) void reloadAcademy()
    } finally {
      setSending(false)
    }
  }

  return (
    <form
      aria-label={`Examen de la lección ${lesson.id}`}
      className="grid gap-3"
      onSubmit={(e) => {
        e.preventDefault()
        if (!sending && bad.length === 0) void submit()
      }}
    >
      <ol className="grid gap-3">
        {questions.map((q, i) => {
          const ok = graded.get(q.id)
          const Mark = ok ? Check : X
          return (
            <li key={q.id}>
              <fieldset className="grid gap-1.5">
                <legend className="mb-1 text-[13px] font-semibold">
                  <span className="mr-1.5 font-mono text-muted-foreground">{i + 1}.</span>
                  {q.prompt}
                </legend>
                {q.kind === 'choice' ? (
                  q.options.map((o) => (
                    <label key={o.id} className="flex min-h-8 items-center gap-2 text-[13px]">
                      <input
                        type="radio"
                        name={q.id}
                        value={o.id}
                        checked={answers[q.id] === o.id}
                        onChange={() => set(q.id, o.id)}
                        className="size-4 accent-[var(--primary)]"
                      />
                      {o.text}
                    </label>
                  ))
                ) : (
                  <label className="flex items-center gap-2 text-[13px]">
                    <span className="sr-only">Respuesta</span>
                    <input
                      inputMode="decimal"
                      autoComplete="off"
                      value={answers[q.id] ?? ''}
                      onChange={(e) => set(q.id, e.target.value)}
                      aria-invalid={bad.includes(q)}
                      className="min-h-8 w-40 rounded-sm border border-border bg-surface px-2 font-mono text-[13px] aria-invalid:border-down"
                    />
                    {q.unit && <span className="text-muted-foreground">{q.unit}</span>}
                  </label>
                )}
                {ok !== undefined && (
                  <p className="flex items-center gap-1 text-[12px] font-semibold">
                    <Mark aria-hidden="true" className="size-3.5" />
                    {ok ? 'Bien, según el motor' : 'Mal, según el motor'}
                  </p>
                )}
              </fieldset>
            </li>
          )
        })}
      </ol>
      {bad.length > 0 && (
        <p role="alert" className="text-[12px] text-down">
          Escribí un número (por ejemplo 1,17 o -0,5) en{' '}
          {bad.length === 1 ? 'la pregunta' : 'las preguntas'} marcada{bad.length === 1 ? '' : 's'}.
        </p>
      )}
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="submit"
          disabled={sending}
          className="min-h-9 rounded-sm bg-primary px-3 text-[12.5px] font-bold text-primary-foreground disabled:opacity-60"
        >
          {sending ? 'Corrigiendo…' : result ? 'Enviar otro intento' : 'Enviar al motor'}
        </button>
        {missing > 0 && (
          <span className="text-[12px] text-muted-foreground">
            {missing} sin responder: cuentan como incorrectas.
          </span>
        )}
      </div>
      <div aria-live="polite">
        {error && (
          <p role="alert" className="text-[12.5px] text-down">
            {error}
          </p>
        )}
        {result && (
          <p
            className={cn(
              'border-l-2 px-2.5 py-1.5 text-[13px]',
              result.passed ? 'border-primary' : 'border-down',
            )}
          >
            <strong>{result.passed ? 'Aprobado' : 'Todavía no'}</strong>
            {` · ${result.correct} de ${result.total} bien · ${decimal(result.score_pct)} % (para aprobar, ${decimal(result.pass_pct)} %) · corrigió el motor el ${dateTimeUtc(result.graded_at)}. El intento quedó guardado.`}
          </p>
        )}
      </div>
    </form>
  )
}
