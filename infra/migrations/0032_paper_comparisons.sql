-- Comparaciones paper vs backtest del mismo período (fase 8, #315). `comparison` es el
-- documento entero de contracts/paper_comparison.v1 (JSONB); las columnas repiten lo que sirve
-- para buscar. Solo el motor (Rust) escribe. Tabla de SOLO AGREGAR (lo impone el trigger):
-- volver a guardar la misma comparación no guarda nada; con otro contenido es un error del motor.

CREATE TABLE paper_comparisons (
    comparison_id TEXT        PRIMARY KEY CHECK (comparison_id ~ '^[A-Za-z0-9._:-]{1,200}$'),
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    period_from   TIMESTAMPTZ NOT NULL,
    period_to     TIMESTAMPTZ NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL,
    comparison    JSONB       NOT NULL CHECK (jsonb_typeof(comparison) = 'object'),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT paper_comparisons_period CHECK (period_to > period_from),
    CONSTRAINT paper_comparisons_same_id CHECK (comparison ->> 'comparison_id' = comparison_id),
    CONSTRAINT paper_comparisons_same_instrument
        CHECK (comparison ->> 'instrument_id' = instrument_id)
);

CREATE INDEX paper_comparisons_instrument ON paper_comparisons (instrument_id, period_from);

CREATE FUNCTION paper_comparisons_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'paper_comparisons es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER paper_comparisons_no_update_or_delete
    BEFORE UPDATE OR DELETE ON paper_comparisons
    FOR EACH ROW EXECUTE FUNCTION paper_comparisons_append_only();
