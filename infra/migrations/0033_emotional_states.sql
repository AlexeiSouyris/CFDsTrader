-- Estados emocionales de la spec §9.1 (#220). Contrato: contracts/emotional_state.v1.schema.json.
-- Una fila por instrumento, fuente, base y momento: los estados activos (`states`, JSONB con la
-- forma del contrato; vacío si no hay ninguno). Los de precio los escribe el motor (Rust) en cada
-- vela cerrada de 15m y 1h; los del índice, Python (sentiment) en cada valor del índice.
-- De SOLO AGREGAR (lo impone el trigger): un estado ya guardado no cambia (sin repintado).
-- (0032 es de paper_comparisons, PR #322, abierto en paralelo.)

CREATE TABLE emotional_states (
    instrument_id TEXT        NOT NULL CHECK (instrument_id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    source        TEXT        NOT NULL CHECK (source IN ('price', 'sentiment')),
    basis         TEXT        NOT NULL CHECK (basis IN ('15m', '1h', 'sentiment_index')),
    as_of         TIMESTAMPTZ NOT NULL,
    states        JSONB       NOT NULL
        CHECK (jsonb_typeof(states) = 'array' AND jsonb_array_length(states) <= 8),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT emotional_states_key PRIMARY KEY (instrument_id, source, basis, as_of),
    CHECK ((source = 'price') = (basis IN ('15m', '1h')))
);

CREATE INDEX emotional_states_latest_idx ON emotional_states (instrument_id, source, basis, as_of DESC);

CREATE FUNCTION emotional_states_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'emotional_states es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER emotional_states_no_update_or_delete
    BEFORE UPDATE OR DELETE ON emotional_states
    FOR EACH ROW EXECUTE FUNCTION emotional_states_append_only();
