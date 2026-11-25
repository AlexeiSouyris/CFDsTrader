-- Correcciones de velas de IBKR ya guardadas (#563, #575). `ibkr_bars` es de solo agregar: una
-- vela guardada no se edita nunca. Antes de #563, `ibkr_load` guardó algunas velas de 15m y 1h con
-- datos a medias (los datos de IBKR sin suscripción llegan con unos 10 minutos de atraso). Esta
-- tabla registra el OHLC correcto de esas velas, con su motivo, su fuente y cuándo se corrigió.
--
-- Las lecturas de Rust (`cfds-storage`: `latest_closed_ibkr_bar`, `ibkr_bars_closed_by`,
-- `last_ibkr_bar_starting_before`, `ibkr_bars_between`; y por ellas `/api/v1/ibkr/bars` y la
-- watchlist del gateway) usan la corrección más reciente de cada vela si existe. La escribe y la
-- lee solo Rust (no cruza lenguajes). Cada corrección apunta a una vela que existe (clave
-- foránea a `ibkr_bars_key`).
--
-- De solo agregar (trigger): una corrección no se edita ni se borra; otra corrección posterior de
-- la misma vela la reemplaza en las lecturas (gana la de `corrected_at` más reciente).
--
-- Idempotente (IF NOT EXISTS / OR REPLACE): se puede aplicar a mano antes de que sqlx la registre
-- sin que falle después.

CREATE TABLE IF NOT EXISTS ibkr_bar_corrections (
    id                BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- Clave de la vela corregida (la de `ibkr_bars_key`).
    source            TEXT        NOT NULL,
    instrument_id     TEXT        NOT NULL,
    contract          TEXT        NOT NULL,
    bar_size          TEXT        NOT NULL,
    what_to_show      TEXT        NOT NULL,
    start_at          TIMESTAMPTZ NOT NULL,
    -- OHLC corregido. `volume` NULL: no se conoce el volumen correcto (se lee NULL).
    open              NUMERIC     NOT NULL CHECK (open > 0),
    high              NUMERIC     NOT NULL CHECK (high > 0),
    low               NUMERIC     NOT NULL CHECK (low > 0),
    close             NUMERIC     NOT NULL CHECK (close > 0),
    volume            NUMERIC              CHECK (volume >= 0),
    -- Por qué se corrigió, en texto para personas.
    reason            TEXT        NOT NULL CHECK (length(reason) BETWEEN 1 AND 500),
    -- De dónde sale el valor corregido, por ejemplo 'ibkr_1m_aggregate' (agregado de las velas de
    -- 1m guardadas de la misma serie).
    correction_source TEXT        NOT NULL CHECK (correction_source ~ '^[a-z][a-z0-9_]{0,63}$'),
    corrected_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ibkr_bar_corrections_ohlc CHECK (
        low <= LEAST(open, close) AND high >= GREATEST(open, close)
    ),
    CONSTRAINT ibkr_bar_corrections_bar
        FOREIGN KEY (source, instrument_id, contract, bar_size, what_to_show, start_at)
        REFERENCES ibkr_bars (source, instrument_id, contract, bar_size, what_to_show, start_at),
    CONSTRAINT ibkr_bar_corrections_once
        UNIQUE (source, instrument_id, contract, bar_size, what_to_show, start_at, corrected_at)
);

CREATE OR REPLACE FUNCTION ibkr_bar_corrections_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'ibkr_bar_corrections es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER ibkr_bar_corrections_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ibkr_bar_corrections
    FOR EACH ROW EXECUTE FUNCTION ibkr_bar_corrections_append_only();
