-- Va como 0022: la 0021 queda para el guardado de las estadísticas de liquidez de #231.
-- Calendario económico (contracts/economic_event.v1, ADR 0006 y ADR 0010, #239). Columnas = las
-- del contrato. Hoy lo escribe Python (`python -m cfds_intel.calendar`, feed JSON oficial de
-- Forex Factory); lo va a leer el filtro de noticias del motor (spec §7.2).
--
-- De solo agregar (trigger): un evento existe desde `published_at`, la primera vez que lo vimos.
-- Si la fuente cambia la hora o el impacto, es otra fila con su propio `published_at`.

CREATE TABLE economic_events (
    source        TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    event         TEXT        NOT NULL CHECK (length(event) BETWEEN 1 AND 200),
    country       TEXT        NULL CHECK (country ~ '^[A-Z]{2}$'),
    instrument_id TEXT        NULL REFERENCES instruments (id),
    impact        TEXT        NOT NULL CHECK (impact IN ('low', 'medium', 'high')),
    scheduled_at  TIMESTAMPTZ NOT NULL,
    published_at  TIMESTAMPTZ NOT NULL,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Como en el contrato: al menos uno de país o instrumento.
    CONSTRAINT economic_events_country_or_instrument
        CHECK (country IS NOT NULL OR instrument_id IS NOT NULL),
    -- Clave natural: la misma lectura no repite el evento (y conserva su primer published_at).
    CONSTRAINT economic_events_key
        UNIQUE NULLS NOT DISTINCT (source, event, country, instrument_id, impact, scheduled_at)
);

CREATE INDEX economic_events_by_time ON economic_events (scheduled_at, published_at);

CREATE FUNCTION economic_events_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'economic_events es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER economic_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON economic_events
    FOR EACH ROW EXECUTE FUNCTION economic_events_append_only();
