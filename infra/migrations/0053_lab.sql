-- Laboratorio de estrategias (#774, docs/design/2026-11-laboratorio.md §2.2 y §5): la cola de
-- trabajos, sus eventos, los informes y las variantes probadas.
-- Contratos: lab_job_request.v1, lab_job_event.v1, lab_report.v1 y lab_trial.v1. Los escriben el
-- gateway (encolar, cancelar) y el lab_worker (tomar, avanzar, guardar), con cfds_journal::lab
-- (PgLab), cada escritura en su propia transacción de escritura.
--
-- SOLO AGREGAR: un trigger no deja modificar ni borrar nunca. El estado de un trabajo es su último
-- evento; un informe nunca se edita (datos nuevos = informe nuevo). Las exploratorias NO suman al
-- N del proyecto: lab_trials solo cuenta variantes de una familia para el laboratorio.

CREATE TABLE lab_jobs (
    job_id          TEXT        PRIMARY KEY CHECK (job_id ~ '^[0-9a-f]{64}$'),
    -- SHA-256 del pedido canónico; el intento sube después de un fallo o una cancelación.
    request_sha256  TEXT        NOT NULL CHECK (request_sha256 ~ '^[0-9a-f]{64}$'),
    attempt         INTEGER     NOT NULL CHECK (attempt >= 1),
    family_sha256   TEXT        NOT NULL CHECK (family_sha256 ~ '^[0-9a-f]{64}$'),
    -- lab_job_request.v1 con las specs normalizadas, tal como se validó al encolar.
    request         JSONB       NOT NULL CHECK (jsonb_typeof(request) = 'object'),
    variants_total  INTEGER     NOT NULL CHECK (variants_total >= 1),
    created_at      TIMESTAMPTZ NOT NULL,
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (request_sha256, attempt)
);

CREATE INDEX lab_jobs_created_idx ON lab_jobs (created_at, job_id);

CREATE TABLE lab_job_events (
    seq             BIGSERIAL   PRIMARY KEY,
    job_id          TEXT        NOT NULL REFERENCES lab_jobs (job_id),
    state           TEXT        NOT NULL CHECK (state IN ('queued', 'running', 'done', 'failed', 'cancelled')),
    stage           TEXT        CHECK (stage IN ('started', 'loading_candles', 'running_variants', 'luck_test', 'building_reports', 'saving')),
    percent         SMALLINT    NOT NULL CHECK (percent BETWEEN 0 AND 100),
    variants_done   INTEGER     NOT NULL CHECK (variants_done >= 0),
    report_ids      TEXT[]      NOT NULL DEFAULT '{}',
    error           TEXT        CHECK (error IS NULL OR length(error) > 0),
    recorded_at     TIMESTAMPTZ NOT NULL,
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((state = 'running') = (stage IS NOT NULL)),
    CHECK ((state = 'failed') = (error IS NOT NULL)),
    CHECK ((state = 'done') = (cardinality(report_ids) > 0)),
    CHECK (state <> 'done' OR percent = 100)
);

CREATE INDEX lab_job_events_job_idx ON lab_job_events (job_id, seq);
-- Un solo "en cola", una sola toma por un worker y un solo final (done, failed o cancelled).
CREATE UNIQUE INDEX lab_job_events_one_queued ON lab_job_events (job_id) WHERE state = 'queued';
CREATE UNIQUE INDEX lab_job_events_one_start ON lab_job_events (job_id) WHERE stage = 'started';
CREATE UNIQUE INDEX lab_job_events_one_end ON lab_job_events (job_id)
    WHERE state IN ('done', 'failed', 'cancelled');

CREATE TABLE lab_reports (
    report_id       TEXT        PRIMARY KEY CHECK (report_id ~ '^[0-9a-f]{64}$'),
    spec_sha256     TEXT        NOT NULL CHECK (spec_sha256 ~ '^[0-9a-f]{64}$'),
    family_sha256   TEXT        NOT NULL CHECK (family_sha256 ~ '^[0-9a-f]{64}$'),
    -- El trabajo que lo guardó primero (el mismo informe exacto no se guarda dos veces).
    job_id          TEXT        NOT NULL REFERENCES lab_jobs (job_id),
    -- lab_report.v1 tal como lo armó el motor.
    report          JSONB       NOT NULL CHECK (jsonb_typeof(report) = 'object'),
    computed_at     TIMESTAMPTZ NOT NULL,
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX lab_reports_spec_idx ON lab_reports (spec_sha256);

CREATE TABLE lab_trials (
    seq             BIGSERIAL   PRIMARY KEY,
    spec_sha256     TEXT        NOT NULL CHECK (spec_sha256 ~ '^[0-9a-f]{64}$'),
    family_sha256   TEXT        NOT NULL CHECK (family_sha256 ~ '^[0-9a-f]{64}$'),
    instrument_id   TEXT        NOT NULL CHECK (instrument_id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    timeframe       TEXT        NOT NULL CHECK (timeframe IN ('15m', '1h')),
    mode            TEXT        NOT NULL CHECK (mode IN ('exploratory', 'confirmatory')),
    catalog_version INTEGER     NOT NULL CHECK (catalog_version >= 1),
    report_id       TEXT        NOT NULL REFERENCES lab_reports (report_id),
    recorded_at     TIMESTAMPTZ NOT NULL,
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX lab_trials_family_idx ON lab_trials (family_sha256, seq);

CREATE FUNCTION lab_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER lab_jobs_no_update_or_delete
    BEFORE UPDATE OR DELETE ON lab_jobs
    FOR EACH ROW EXECUTE FUNCTION lab_append_only();
CREATE TRIGGER lab_job_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON lab_job_events
    FOR EACH ROW EXECUTE FUNCTION lab_append_only();
CREATE TRIGGER lab_reports_no_update_or_delete
    BEFORE UPDATE OR DELETE ON lab_reports
    FOR EACH ROW EXECUTE FUNCTION lab_append_only();
CREATE TRIGGER lab_trials_no_update_or_delete
    BEFORE UPDATE OR DELETE ON lab_trials
    FOR EACH ROW EXECUTE FUNCTION lab_append_only();
