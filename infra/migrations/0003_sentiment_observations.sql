-- Observaciones crudas de fuentes de sentimiento (#25, #27). Columnas =
-- contracts/sentiment_observation.v1. Tabla de SOLO AGREGAR: una corrección de la fuente es una
-- fila nueva (otra published_at), nunca se pisa la anterior. El trigger lo impone en la base.
-- Solo el motor (Rust) escribe; Python lee.

CREATE TABLE sentiment_observations (
    source        TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    series        TEXT        NOT NULL CHECK (series ~ '^[a-z][a-z0-9_]{0,31}$'),
    instrument_id TEXT        NULL REFERENCES instruments (id),
    observed_for  TIMESTAMPTZ NOT NULL,
    published_at  TIMESTAMPTZ NOT NULL,
    value         NUMERIC     NOT NULL,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Un dato nunca está disponible antes del momento al que corresponde.
    CONSTRAINT sentiment_published_after_observed CHECK (published_at >= observed_for),
    -- Los porcentajes de clientes de IG están entre 0 y 100.
    CONSTRAINT sentiment_ig_client_pct CHECK (source <> 'ig_client' OR (value >= 0 AND value <= 100)),
    CONSTRAINT sentiment_observations_key
        UNIQUE NULLS NOT DISTINCT (source, series, instrument_id, observed_for, published_at)
);

CREATE INDEX sentiment_instrument_published_idx
    ON sentiment_observations (instrument_id, published_at);

CREATE FUNCTION sentiment_observations_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'sentiment_observations es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER sentiment_observations_no_update_or_delete
    BEFORE UPDATE OR DELETE ON sentiment_observations
    FOR EACH ROW EXECUTE FUNCTION sentiment_observations_append_only();
