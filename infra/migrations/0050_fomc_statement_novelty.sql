-- Novedad de cada comunicado del FOMC respecto del anterior (contracts/fomc_statement_novelty.v1,
-- #668). Columnas = las del contrato. Lo escribe Python (`python -m cfds_intel.fomc_novelty`)
-- con los comunicados de `official_releases` (ADR 0016) y el texto de su página oficial. Es
-- descriptivo: no le pide opinión a ningún modelo ni usa precios.
--
-- Formato largo: una fila por comunicado y método (`word_diff`, `embedding_cosine`), con la
-- versión del método y, si usa un modelo, su nombre y su digest. Un método o un modelo nuevo es
-- otra fila; nada se pisa.
--
-- De solo agregar (trigger): una fila existe desde `available_at`, la hora del comunicado.

CREATE TABLE fomc_statement_novelty (
    reference_period          TEXT        NOT NULL CHECK (reference_period ~ '^\d{4}-\d{2}-\d{2}$'),
    status                    TEXT        NOT NULL CHECK (status IN ('scheduled', 'unscheduled')),
    statement_url             TEXT        NOT NULL
        CHECK (statement_url ~ '^https://www\.federalreserve\.gov/' AND length(statement_url) <= 500),
    available_at              TIMESTAMPTZ NOT NULL,
    previous_reference_period TEXT        NOT NULL
        CHECK (previous_reference_period ~ '^\d{4}-\d{2}-\d{2}$'),
    previous_statement_url    TEXT        NOT NULL
        CHECK (previous_statement_url ~ '^https://www\.federalreserve\.gov/'
            AND length(previous_statement_url) <= 500),
    method                    TEXT        NOT NULL CHECK (method IN ('word_diff', 'embedding_cosine')),
    method_version            TEXT        NOT NULL CHECK (method_version ~ '^[a-z][a-z0-9_.]{0,63}$'),
    model                     TEXT        NULL CHECK (length(model) BETWEEN 1 AND 200),
    model_digest              TEXT        NULL CHECK (model_digest ~ '^[a-z0-9:]{8,100}$'),
    value                     NUMERIC(8, 6) NOT NULL CHECK (value >= 0 AND value <= 2),
    text_sha256               TEXT        NOT NULL CHECK (text_sha256 ~ '^[0-9a-f]{64}$'),
    previous_text_sha256      TEXT        NOT NULL CHECK (previous_text_sha256 ~ '^[0-9a-f]{64}$'),
    word_count                INTEGER     NOT NULL CHECK (word_count >= 1),
    previous_word_count       INTEGER     NOT NULL CHECK (previous_word_count >= 1),
    computed_at               TIMESTAMPTZ NOT NULL,
    inserted_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- El comunicado anterior es anterior.
    CONSTRAINT fomc_statement_novelty_previous_before
        CHECK (previous_reference_period < reference_period),
    -- Con modelo solo los embeddings; el diff de palabras va de 0 a 1.
    CONSTRAINT fomc_statement_novelty_model_of_method CHECK (
        (method = 'word_diff' AND model IS NULL AND model_digest IS NULL AND value <= 1)
        OR (method = 'embedding_cosine' AND model IS NOT NULL AND model_digest IS NOT NULL)
    ),
    -- Clave natural: otra corrida con el mismo método, versión y modelo no agrega nada.
    CONSTRAINT fomc_statement_novelty_key UNIQUE NULLS NOT DISTINCT
        (reference_period, method, method_version, model, model_digest)
);

CREATE INDEX fomc_statement_novelty_by_time
    ON fomc_statement_novelty (method, method_version, available_at);

CREATE FUNCTION fomc_statement_novelty_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'fomc_statement_novelty es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER fomc_statement_novelty_no_update_or_delete
    BEFORE UPDATE OR DELETE ON fomc_statement_novelty
    FOR EACH ROW EXECUTE FUNCTION fomc_statement_novelty_append_only();
