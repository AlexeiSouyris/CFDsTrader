-- Eventos de progreso de la Academia (#718, docs/design/2026-11-academia.md §4.2).
-- Contrato: contracts/academy_event.v1. Los escribe el gateway (crate cfds-academy, PgAcademy):
-- intentos de examen con la corrección del motor, lecciones sin examen completadas y desbloqueos o
-- rebloqueos a mano de una herramienta. El estado (aprobadas, herramientas abiertas) NO se guarda:
-- el motor lo reconstruye leyendo estos eventos en orden de `seq`.
--
-- SOLO AGREGAR: el trigger no deja modificar ni borrar nunca. Cada intento queda guardado.

CREATE TABLE academy_events (
    seq             BIGSERIAL   PRIMARY KEY,
    member_id       TEXT        NOT NULL CHECK (member_id ~ '^[A-Za-z0-9._-]{1,64}$'),
    kind            TEXT        NOT NULL CHECK (kind IN ('lesson_completed', 'exam_attempt', 'tool_override')),
    lesson_id       TEXT        CHECK (lesson_id ~ '^(0|[1-9][0-9]?)\.([1-9][0-9]?)$'),
    tool_id         TEXT        CHECK (tool_id ~ '^(\*|tool\.[a-z0-9_]{1,48})$'),
    action          TEXT        CHECK (action IN ('unlock', 'relock')),
    -- quiz_result.v1 del intento, tal como lo corrigió el motor.
    quiz            JSONB       CHECK (quiz IS NULL OR jsonb_typeof(quiz) = 'object'),
    catalog_version TEXT        NOT NULL CHECK (catalog_version ~ '^[0-9a-f]{64}$'),
    recorded_at     TIMESTAMPTZ NOT NULL,
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (kind = 'lesson_completed' AND lesson_id IS NOT NULL AND tool_id IS NULL AND action IS NULL AND quiz IS NULL)
        OR (kind = 'exam_attempt' AND lesson_id IS NOT NULL AND tool_id IS NULL AND action IS NULL AND quiz IS NOT NULL)
        OR (kind = 'tool_override' AND lesson_id IS NULL AND tool_id IS NOT NULL AND action IS NOT NULL AND quiz IS NULL)
    )
);

CREATE INDEX academy_events_member_idx ON academy_events (member_id, seq);

CREATE FUNCTION academy_events_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'academy_events es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER academy_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON academy_events
    FOR EACH ROW EXECUTE FUNCTION academy_events_append_only();
