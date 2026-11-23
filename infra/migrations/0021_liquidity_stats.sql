-- Va como 0021: la 0020 es del mapa de liquidez (#237) y la 0019 queda vacía, para que ninguna
-- migración quede por debajo de una ya aplicada.
-- Estadísticas de FVG y gaps, agregados y desequilibrio (spec §5.4.1, #231). Columnas =
-- contracts/fvg_event.v1, opening_gap.v1, liquidity_stats.v1 e imbalance.v1. Lo calcula y
-- escribe el motor (cfds-liquidity + cfds-ingest, ejemplo liquidity_stats); nadie lo recalcula.
-- Solo informativo: no suma al puntaje de §6.
--
-- Todas las tablas son de solo agregar (trigger: sin UPDATE ni DELETE):
-- - fvg_zones / opening_gaps: la zona, que no cambia nunca.
-- - fvg_zone_events / opening_gap_events: cada evento (toque, mitad, llenado, vencimiento y
--   reacción) se escribe una sola vez, con la hora desde la que se conoce. Una zona sin llenado
--   ni vencimiento está abierta.
-- - imbalances: un valor por vela cerrada.
-- - liquidity_stats: los agregados de cada corrida (computed_at); una corrida nueva no pisa las
--   anteriores.

CREATE TABLE fvg_zones (
    id            BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    timeframe     TEXT        NOT NULL CHECK (timeframe IN ('15m', '1h', '1d')),
    direction     TEXT        NOT NULL CHECK (direction IN ('bullish', 'bearish')),
    created_at    TIMESTAMPTZ NOT NULL,
    bottom        NUMERIC     NOT NULL CHECK (bottom >= 0),
    top           NUMERIC     NOT NULL,
    atr           NUMERIC     NOT NULL CHECK (atr > 0),
    size_atr      NUMERIC     NOT NULL CHECK (size_atr > 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fvg_zones_top_above_bottom CHECK (top > bottom),
    CONSTRAINT fvg_zones_key UNIQUE (instrument_id, timeframe, direction, created_at)
);

CREATE TABLE fvg_zone_events (
    zone_id      BIGINT      NOT NULL REFERENCES fvg_zones (id),
    event        TEXT        NOT NULL CHECK (event IN ('touch', 'mid', 'fill', 'expiry', 'reaction')),
    -- Solo para reaction: a cuántas velas del primer toque.
    horizon_bars INTEGER     NULL CHECK (horizon_bars >= 1),
    at           TIMESTAMPTZ NOT NULL,
    -- touch, mid, fill y expiry: velas desde la creación; reaction: NULL.
    bars         INTEGER     NULL CHECK (bars >= 1),
    return_atr   NUMERIC     NULL,
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fvg_zone_events_shape CHECK (
        (event = 'reaction') = (horizon_bars IS NOT NULL AND return_atr IS NOT NULL AND bars IS NULL)
        AND (event = 'reaction' OR (bars IS NOT NULL AND horizon_bars IS NULL AND return_atr IS NULL))
    ),
    CONSTRAINT fvg_zone_events_key UNIQUE NULLS NOT DISTINCT (zone_id, event, horizon_bars)
);

CREATE TABLE opening_gaps (
    id            BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    day           DATE        NOT NULL,
    direction     TEXT        NOT NULL CHECK (direction IN ('bullish', 'bearish')),
    prev_close    NUMERIC     NOT NULL CHECK (prev_close >= 0),
    open          NUMERIC     NOT NULL CHECK (open >= 0),
    atr           NUMERIC     NOT NULL CHECK (atr > 0),
    size_atr      NUMERIC     NOT NULL CHECK (size_atr > 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT opening_gaps_key UNIQUE (instrument_id, day)
);

CREATE TABLE opening_gap_events (
    gap_id      BIGINT      NOT NULL REFERENCES opening_gaps (id),
    event       TEXT        NOT NULL CHECK (event IN ('fill', 'expiry')),
    day         DATE        NOT NULL,
    sessions    INTEGER     NOT NULL CHECK (sessions >= 0),
    at          TIMESTAMPTZ NOT NULL,
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Un gap se llena o vence, una sola vez.
    CONSTRAINT opening_gap_events_key PRIMARY KEY (gap_id)
);

CREATE TABLE imbalances (
    instrument_id    TEXT        NOT NULL REFERENCES instruments (id),
    timeframe        TEXT        NOT NULL CHECK (timeframe IN ('15m', '1h', '1d')),
    at               TIMESTAMPTZ NOT NULL,
    window_bars      INTEGER     NOT NULL CHECK (window_bars >= 1),
    displacement     TEXT        NULL CHECK (displacement IN ('bullish', 'bearish')),
    bullish_body_sum NUMERIC     NOT NULL CHECK (bullish_body_sum >= 0),
    bearish_body_sum NUMERIC     NOT NULL CHECK (bearish_body_sum >= 0),
    range_sum        NUMERIC     NOT NULL CHECK (range_sum >= 0),
    imbalance_index  NUMERIC     NOT NULL CHECK (imbalance_index >= -1 AND imbalance_index <= 1),
    inserted_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT imbalances_key PRIMARY KEY (instrument_id, timeframe, at, window_bars)
);

CREATE TABLE liquidity_stats (
    computed_at   TIMESTAMPTZ NOT NULL,
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    subject       TEXT        NOT NULL CHECK (subject IN ('fvg', 'opening_gap')),
    timeframe     TEXT        NOT NULL CHECK (timeframe IN ('15m', '1h', '1d')),
    direction     TEXT        NOT NULL CHECK (direction IN ('bullish', 'bearish')),
    size_tercile  TEXT        NOT NULL CHECK (size_tercile IN ('low', 'mid', 'high', 'all')),
    as_of         TIMESTAMPTZ NOT NULL,
    n             INTEGER     NOT NULL CHECK (n >= 0),
    -- El documento entero de liquidity_stats.v1.
    document      JSONB       NOT NULL,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT liquidity_stats_as_of_before_run CHECK (as_of <= computed_at),
    CONSTRAINT liquidity_stats_key
        PRIMARY KEY (computed_at, instrument_id, subject, timeframe, direction, size_tercile)
);

CREATE INDEX liquidity_stats_latest ON liquidity_stats (instrument_id, subject, timeframe, computed_at);

-- Un evento siempre es posterior a la creación de su zona (el CHECK no puede mirar otra tabla).
CREATE FUNCTION fvg_zone_events_after_creation() RETURNS trigger AS $$
BEGIN
    IF NEW.at <= (SELECT created_at FROM fvg_zones WHERE id = NEW.zone_id) THEN
        RAISE EXCEPTION 'el evento de un FVG tiene que ser posterior a su creación';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER fvg_zone_events_check_time
    BEFORE INSERT ON fvg_zone_events
    FOR EACH ROW EXECUTE FUNCTION fvg_zone_events_after_creation();

CREATE FUNCTION opening_gap_events_not_before_gap() RETURNS trigger AS $$
BEGIN
    IF NEW.day < (SELECT day FROM opening_gaps WHERE id = NEW.gap_id) THEN
        RAISE EXCEPTION 'el evento de un gap no puede ser anterior a su sesión';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER opening_gap_events_check_day
    BEFORE INSERT ON opening_gap_events
    FOR EACH ROW EXECUTE FUNCTION opening_gap_events_not_before_gap();

CREATE FUNCTION liquidity_stats_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER fvg_zones_no_update_or_delete
    BEFORE UPDATE OR DELETE ON fvg_zones
    FOR EACH ROW EXECUTE FUNCTION liquidity_stats_append_only();
CREATE TRIGGER fvg_zone_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON fvg_zone_events
    FOR EACH ROW EXECUTE FUNCTION liquidity_stats_append_only();
CREATE TRIGGER opening_gaps_no_update_or_delete
    BEFORE UPDATE OR DELETE ON opening_gaps
    FOR EACH ROW EXECUTE FUNCTION liquidity_stats_append_only();
CREATE TRIGGER opening_gap_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON opening_gap_events
    FOR EACH ROW EXECUTE FUNCTION liquidity_stats_append_only();
CREATE TRIGGER imbalances_no_update_or_delete
    BEFORE UPDATE OR DELETE ON imbalances
    FOR EACH ROW EXECUTE FUNCTION liquidity_stats_append_only();
CREATE TRIGGER liquidity_stats_no_update_or_delete
    BEFORE UPDATE OR DELETE ON liquidity_stats
    FOR EACH ROW EXECUTE FUNCTION liquidity_stats_append_only();
