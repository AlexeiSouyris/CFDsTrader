-- Va como 0029: después de la 0028 (registro del broker); la 0027 no se usa para no aplicar una
-- migración vieja después de una más nueva.
-- Análogos históricos (spec §13, #148, #294). El documento entero es contracts/analog_report.v1.
-- Lo calcula y lo escribe Python (`python -m cfds_intel.analogs --guardar`); lo leen el gateway
-- y la UI. Solo informativo: distribución de resultados pasados, no un pronóstico.
-- Tabla de SOLO AGREGAR: un informe guardado no cambia. Si se recalcula igual, no se repite.

CREATE TABLE analog_reports (
    id             BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    instrument_id  TEXT        NOT NULL CHECK (instrument_id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    source         TEXT        NOT NULL CHECK (source IN ('ig', 'futures')),
    window_end_day DATE        NOT NULL,
    as_of          TIMESTAMPTZ NOT NULL,
    computed_at    TIMESTAMPTZ NOT NULL,
    document       JSONB       NOT NULL,
    inserted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Nada se calcula antes de que cierre la ventana.
    CONSTRAINT analog_reports_after_window CHECK (computed_at >= as_of),
    CONSTRAINT analog_reports_same_doc CHECK (
        document ->> 'instrument_id' = instrument_id
        AND document ->> 'source' = source
        AND document ->> 'window_end_day' = window_end_day::text
    )
);

CREATE INDEX analog_reports_latest ON analog_reports (instrument_id, source, as_of DESC, id DESC);

CREATE FUNCTION analog_reports_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'analog_reports es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER analog_reports_no_update_or_delete
    BEFORE UPDATE OR DELETE ON analog_reports
    FOR EACH ROW EXECUTE FUNCTION analog_reports_append_only();
