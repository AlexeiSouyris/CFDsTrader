-- Spread al cierre de cada vela (#95). Columnas = contracts/candle_spread.v1.
-- Complementa a `candles` (que guarda el precio medio): la vela tiene que existir antes.
-- Lo usa el filtro de spread de la spec §7.3 (#90). Un valor guardado no se pisa: otro valor
-- para la misma vela es un error que hay que revisar en la fuente.

CREATE TABLE candle_spreads (
    instrument_id TEXT        NOT NULL,
    timeframe     TEXT        NOT NULL,
    start_at      TIMESTAMPTZ NOT NULL,
    spread        NUMERIC     NOT NULL CHECK (spread >= 0),
    source        TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (instrument_id, timeframe, start_at),
    FOREIGN KEY (instrument_id, timeframe, start_at)
        REFERENCES candles (instrument_id, timeframe, start_at)
);
