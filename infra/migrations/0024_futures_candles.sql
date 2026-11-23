-- Velas de 15m y 1h de futuros armadas por el motor desde `futures_bars_1m` (ADR 0011, #259).
-- Las arma y escribe Rust (`cargo run -p cfds-ingest --example futures_candles`); las lee la
-- liquidez cuando se elige la fuente "futuros". Mismas columnas que `candles` (candle.v1) más la
-- fuente y el contrato.
--
-- Tabla APARTE de `candles` a propósito: `candles` no tiene fuente y todo lo que la lee
-- (estrategia, backtest, gateway) supone que son velas del CFD de IG. Una columna `source` ahí
-- obligaría a cambiar la clave primaria y a filtrar en cada consulta, y una que se olvide
-- mezclaría precios del futuro (con su base) con los del CFD.
--
-- De solo agregar (trigger): una vela guardada no cambia nunca (sin repintado).

CREATE TABLE futures_candles (
    source        TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    dataset       TEXT        NOT NULL CHECK (dataset ~ '^[A-Z0-9]+\.[A-Z0-9]+$'),
    symbol        TEXT        NOT NULL CHECK (symbol ~ '^[A-Z0-9]{1,10}\.[cnv]\.[0-9]{1,2}$'),
    -- Todas las barras de la vela son de este contrato (ninguna vela mezcla vencimientos).
    contract_id   BIGINT      NOT NULL CHECK (contract_id > 0),
    -- Nuestro instrumento (CFD de IG) al que corresponde el futuro.
    instrument_id TEXT        NOT NULL CHECK (instrument_id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    timeframe     TEXT        NOT NULL CHECK (timeframe IN ('15m', '1h')),
    -- Inicio de la vela en UTC, alineado igual que las de IG.
    start_at      TIMESTAMPTZ NOT NULL,
    open          NUMERIC     NOT NULL CHECK (open > 0),
    high          NUMERIC     NOT NULL CHECK (high > 0),
    low           NUMERIC     NOT NULL CHECK (low > 0),
    close         NUMERIC     NOT NULL CHECK (close > 0),
    volume        NUMERIC     NOT NULL CHECK (volume >= 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT futures_candles_aligned CHECK (
        (timeframe = '15m' AND extract(epoch FROM start_at)::bigint % 900 = 0)
        OR (timeframe = '1h' AND extract(epoch FROM start_at)::bigint % 3600 = 0)
    ),
    CONSTRAINT futures_candles_ohlc CHECK (
        low <= LEAST(open, close) AND high >= GREATEST(open, close)
    ),
    CONSTRAINT futures_candles_key UNIQUE (source, instrument_id, timeframe, start_at)
);

CREATE FUNCTION futures_candles_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'futures_candles es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER futures_candles_no_update_or_delete
    BEFORE UPDATE OR DELETE ON futures_candles
    FOR EACH ROW EXECUTE FUNCTION futures_candles_append_only();
