-- Intentos de examen de replay de una lección (#759, docs/design/2026-11-academia.md §4.2 y §5).
-- Contrato: contracts/academy_event.v1 (campo `replay`, exam_result.v1). Lo escribe el gateway al
-- terminar una sesión de replay de una lección (POST /api/v1/academy/replay/{id}/finish), ANTES de
-- responder: sin base no hay nota.
--
-- Solo agrega: una columna nueva (NULL en todas las filas que ya existen) y el chequeo por tipo
-- ampliado para que un `exam_attempt` lleve `quiz` (examen de preguntas) o `replay` (examen de
-- replay), nunca los dos. Las filas guardadas siguen valiendo tal cual: el chequeo nuevo las acepta
-- y no se edita ninguna (el trigger de solo agregar de 0052 sigue igual).

ALTER TABLE academy_events
    -- exam_result.v1 del intento, tal como lo calificó el motor.
    ADD COLUMN replay JSONB CHECK (replay IS NULL OR jsonb_typeof(replay) = 'object');

-- El chequeo por tipo de 0052 no tenía nombre: PostgreSQL lo llamó academy_events_check.
ALTER TABLE academy_events DROP CONSTRAINT academy_events_check;

ALTER TABLE academy_events ADD CONSTRAINT academy_events_kind_fields CHECK (
    (kind = 'lesson_completed' AND lesson_id IS NOT NULL AND tool_id IS NULL AND action IS NULL
        AND quiz IS NULL AND replay IS NULL)
    OR (kind = 'exam_attempt' AND lesson_id IS NOT NULL AND tool_id IS NULL AND action IS NULL
        AND ((quiz IS NOT NULL AND replay IS NULL) OR (quiz IS NULL AND replay IS NOT NULL)))
    OR (kind = 'tool_override' AND lesson_id IS NULL AND tool_id IS NOT NULL AND action IS NOT NULL
        AND quiz IS NULL AND replay IS NULL)
);
