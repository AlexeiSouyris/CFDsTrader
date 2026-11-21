-- Noticias de fuentes aprobadas (#111, ADR 0006). Columnas = contracts/news_item.v1.
-- Escribe Python (`cfds_intel.news`), dueño de la ingesta de noticias (CLAUDE.md §1); el
-- motor lee. Tabla de SOLO AGREGAR: una noticia no se modifica ni se borra (lo impone el
-- trigger). Una noticia es (fuente, id): puede aparecer en varios feeds de la misma fuente, y
-- se guarda con el primero en el que se leyó.

CREATE TABLE news_items (
    source       TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    feed         TEXT        NOT NULL CHECK (feed ~ '^[a-z][a-z0-9_]{0,31}$'),
    item_id      TEXT        NOT NULL CHECK (length(item_id) BETWEEN 1 AND 500),
    title        TEXT        NOT NULL CHECK (length(title) BETWEEN 1 AND 500),
    url          TEXT        NOT NULL CHECK (url ~ '^https://' AND length(url) <= 1000),
    published_at TIMESTAMPTZ NOT NULL,
    retrieved_at TIMESTAMPTZ NOT NULL,
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (source, item_id),
    -- Una noticia no se puede leer antes de publicarse.
    CONSTRAINT news_published_before_retrieved CHECK (published_at <= retrieved_at)
);

CREATE INDEX news_items_published_idx ON news_items (published_at);

CREATE FUNCTION news_items_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'news_items es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER news_items_no_update_or_delete
    BEFORE UPDATE OR DELETE ON news_items
    FOR EACH ROW EXECUTE FUNCTION news_items_append_only();
