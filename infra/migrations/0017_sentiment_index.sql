-- Índice de sentimiento v1 (spec §9.1, #213). Columnas = contracts/sentiment.v1 (los componentes,
-- tal cual el contrato, en `components`). Lo escribe Python (cfds_intel.sentiment); nadie lo
-- recalcula. Tabla de SOLO AGREGAR: un valor emitido no cambia nunca (sin repintado).
-- Va como 0017: la 0015 es de ML y la 0016 de daily_drawdowns.

CREATE TABLE sentiment_index (
    instrument_id   TEXT        NOT NULL REFERENCES instruments (id),
    as_of           TIMESTAMPTZ NOT NULL,
    published_at    TIMESTAMPTZ NOT NULL,
    value           NUMERIC     NOT NULL CHECK (value >= -1 AND value <= 1),
    warmup_complete BOOLEAN     NOT NULL,
    min_components  INTEGER     NOT NULL CHECK (min_components >= 1),
    components      JSONB       NOT NULL CHECK (jsonb_typeof(components) = 'array'),
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Nada existe antes del momento al que corresponde.
    CONSTRAINT sentiment_index_published_after_as_of CHECK (published_at >= as_of),
    CONSTRAINT sentiment_index_key UNIQUE (instrument_id, published_at)
);

CREATE FUNCTION sentiment_index_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'sentiment_index es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER sentiment_index_no_update_or_delete
    BEFORE UPDATE OR DELETE ON sentiment_index
    FOR EACH ROW EXECUTE FUNCTION sentiment_index_append_only();
