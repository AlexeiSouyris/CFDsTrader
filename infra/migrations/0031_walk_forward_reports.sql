-- Reportes de la evaluación de la Fase 5 (#298). `report` es el documento entero de
-- contracts/walk_forward_report.v1 (JSONB); las columnas repiten lo que sirve para buscar. Solo
-- el motor (Rust) escribe; Python lee. Tabla de SOLO AGREGAR (lo impone el trigger): volver a
-- guardar el mismo reporte (todo igual salvo computed_at) no guarda nada; con otro contenido es
-- un error del motor.

CREATE TABLE walk_forward_reports (
    report_id     TEXT        PRIMARY KEY CHECK (report_id ~ '^[A-Za-z0-9._-]{1,100}$'),
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    candle_source TEXT        NOT NULL CHECK (candle_source IN ('ig', 'futures')),
    config_sha256 TEXT        NOT NULL CHECK (config_sha256 ~ '^[0-9a-f]{64}$'),
    computed_at   TIMESTAMPTZ NOT NULL,
    report        JSONB       NOT NULL CHECK (jsonb_typeof(report) = 'object'),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT walk_forward_reports_same_id CHECK (report ->> 'report_id' = report_id),
    CONSTRAINT walk_forward_reports_same_instrument
        CHECK (report ->> 'instrument_id' = instrument_id)
);

CREATE INDEX walk_forward_reports_instrument ON walk_forward_reports (instrument_id, computed_at);

CREATE FUNCTION walk_forward_reports_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'walk_forward_reports es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER walk_forward_reports_no_update_or_delete
    BEFORE UPDATE OR DELETE ON walk_forward_reports
    FOR EACH ROW EXECUTE FUNCTION walk_forward_reports_append_only();
