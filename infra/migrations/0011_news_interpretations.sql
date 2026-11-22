-- Interpretaciones de noticias (#186, #129, ADR 0008). Columnas = contracts/news_interpretation.v1
-- (el objeto `interpreter` va aplanado en interpreter_*; `impacts` es JSONB con la forma del
-- contrato, como `breakdown` en el diario). Escribe Python (`cfds_intel.news`), dueño de la
-- interpretación (CLAUDE.md §1); el motor lee. Tabla de SOLO AGREGAR (lo impone el trigger): una
-- interpretación no se corrige, se hace otra con otra versión del intérprete.

CREATE TABLE news_interpretations (
    interpretation_id         TEXT        PRIMARY KEY CHECK (interpretation_id ~ '^[0-9a-f]{64}$'),
    source                    TEXT        NOT NULL,
    item_id                   TEXT        NOT NULL,
    published_at              TIMESTAMPTZ NOT NULL,
    interpreted_at            TIMESTAMPTZ NOT NULL,
    interpreter_name          TEXT        NOT NULL CHECK (interpreter_name ~ '^[a-z][a-z0-9_-]{0,63}$'),
    interpreter_version       TEXT        NOT NULL
        CHECK (interpreter_version ~ '^[0-9A-Za-z][0-9A-Za-z._+-]{0,63}$'),
    interpreter_model         TEXT        CHECK (length(interpreter_model) BETWEEN 1 AND 200),
    interpreter_prompt_sha256 TEXT        CHECK (interpreter_prompt_sha256 ~ '^[0-9a-f]{64}$'),
    summary_es                TEXT        NOT NULL CHECK (length(summary_es) BETWEEN 1 AND 1000),
    impacts                   JSONB       NOT NULL
        CHECK (jsonb_typeof(impacts) = 'array' AND jsonb_array_length(impacts) <= 64),
    inserted_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Solo se interpreta una noticia guardada.
    FOREIGN KEY (source, item_id) REFERENCES news_items (source, item_id),
    -- Una noticia tiene una sola interpretación por intérprete (nombre y versión). El id ya lo
    -- garantiza (es el SHA-256 de estas cuatro cosas); esto además sirve para buscar las que faltan.
    UNIQUE (source, item_id, interpreter_name, interpreter_version),
    -- La interpretación no existe antes de que se publique la noticia (CLAUDE.md §5b).
    CONSTRAINT news_interpreted_after_published CHECK (interpreted_at >= published_at)
);

CREATE INDEX news_interpretations_interpreted_idx ON news_interpretations (interpreted_at);

CREATE FUNCTION news_interpretations_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'news_interpretations es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER news_interpretations_no_update_or_delete
    BEFORE UPDATE OR DELETE ON news_interpretations
    FOR EACH ROW EXECUTE FUNCTION news_interpretations_append_only();
