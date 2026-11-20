-- Issue #6: histórico de instrumentos y velas.
-- Las columnas siguen los contratos instrument.v1 y candle.v1 (contracts/).
-- Las restricciones CHECK repiten lo esencial de la validación (#5) como última barrera:
-- aunque alguien se saltee la validación, la base no acepta una vela imposible.

CREATE TABLE instruments (
    id                TEXT PRIMARY KEY CHECK (id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    asset_class       TEXT NOT NULL CHECK (asset_class IN ('index', 'share')),
    currency          CHAR(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
    exchange_timezone TEXT NOT NULL
);

CREATE TABLE candles (
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    timeframe     TEXT        NOT NULL CHECK (timeframe IN ('15m', '1h')),
    start_at      TIMESTAMPTZ NOT NULL,
    open          NUMERIC     NOT NULL CHECK (open > 0),
    high          NUMERIC     NOT NULL CHECK (high > 0),
    low           NUMERIC     NOT NULL CHECK (low > 0),
    close         NUMERIC     NOT NULL CHECK (close > 0),
    volume        NUMERIC              CHECK (volume >= 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (instrument_id, timeframe, start_at),
    CONSTRAINT candles_ohlc_consistent CHECK (
        high >= low AND high >= open AND high >= close AND low <= open AND low <= close
    )
);
