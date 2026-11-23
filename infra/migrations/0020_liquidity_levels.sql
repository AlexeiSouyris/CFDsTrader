-- Va como 0020: la 0019 queda para las estadísticas de FVG y gaps de #231, que van en paralelo.
-- Mapa de liquidez (spec §5.4.1, #231). Columnas = contracts/liquidity_level.v1. Lo calcula y
-- escribe el motor (cfds-liquidity + cfds-ingest, ejemplo liquidity_levels); nadie lo recalcula.
--
-- Dos tablas de solo agregar (trigger: sin UPDATE ni DELETE):
-- - liquidity_levels: el nivel, que no cambia nunca. Clave natural: instrumento, tipo, sesión,
--   precio y exists_from.
-- - liquidity_level_events: a lo sumo un evento por nivel, cuando deja de estar activo:
--   swept o broken (state y state_changed_at del contrato) o expired (expired_at). Un nivel sin
--   evento está intacto y activo. El evento se escribe una sola vez y nunca se reescribe.

CREATE TABLE liquidity_levels (
    id            BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    kind          TEXT        NOT NULL CHECK (kind IN ('eqh', 'eql', 'pdh', 'pdl', 'pwh', 'pwl',
                                                       'session_high', 'session_low')),
    session       TEXT        NULL CHECK (session IN ('asia', 'london', 'ny')),
    price         NUMERIC     NOT NULL CHECK (price >= 0),
    exists_from   TIMESTAMPTZ NOT NULL,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- La sesión va solo (y siempre) en los niveles de sesión.
    CONSTRAINT liquidity_levels_session
        CHECK ((kind IN ('session_high', 'session_low')) = (session IS NOT NULL)),
    CONSTRAINT liquidity_levels_key
        UNIQUE NULLS NOT DISTINCT (instrument_id, kind, session, price, exists_from)
);

CREATE INDEX liquidity_levels_by_time ON liquidity_levels (instrument_id, exists_from);

CREATE TABLE liquidity_level_events (
    level_id    BIGINT      PRIMARY KEY REFERENCES liquidity_levels (id),
    event       TEXT        NOT NULL CHECK (event IN ('swept', 'broken', 'expired')),
    at          TIMESTAMPTZ NOT NULL,
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- El evento siempre es posterior a la existencia del nivel (el CHECK no puede mirar otra tabla).
CREATE FUNCTION liquidity_level_events_after_level() RETURNS trigger AS $$
BEGIN
    IF NEW.at <= (SELECT exists_from FROM liquidity_levels WHERE id = NEW.level_id) THEN
        RAISE EXCEPTION 'el evento de un nivel tiene que ser posterior a su exists_from';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER liquidity_level_events_check_time
    BEFORE INSERT ON liquidity_level_events
    FOR EACH ROW EXECUTE FUNCTION liquidity_level_events_after_level();

CREATE FUNCTION liquidity_levels_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER liquidity_levels_no_update_or_delete
    BEFORE UPDATE OR DELETE ON liquidity_levels
    FOR EACH ROW EXECUTE FUNCTION liquidity_levels_append_only();

CREATE TRIGGER liquidity_level_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON liquidity_level_events
    FOR EACH ROW EXECUTE FUNCTION liquidity_levels_append_only();
