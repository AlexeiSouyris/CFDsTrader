-- Velas diarias (#103). Columnas = contracts/daily_candle.v1.
-- El día va de medianoche a medianoche en la zona de los precios: start y end son explícitos
-- porque en un cambio de horario el día dura 23 o 25 h. Mismas barreras que `candles`.

CREATE TABLE daily_candles (
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    day           DATE        NOT NULL,
    start_at      TIMESTAMPTZ NOT NULL,
    end_at        TIMESTAMPTZ NOT NULL,
    open          NUMERIC     NOT NULL CHECK (open > 0),
    high          NUMERIC     NOT NULL CHECK (high > 0),
    low           NUMERIC     NOT NULL CHECK (low > 0),
    close         NUMERIC     NOT NULL CHECK (close > 0),
    volume        NUMERIC              CHECK (volume >= 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (instrument_id, day),
    -- Un día local dura entre 23 y 25 h.
    CONSTRAINT daily_candles_length CHECK (
        end_at - start_at BETWEEN INTERVAL '23 hours' AND INTERVAL '25 hours'
    ),
    CONSTRAINT daily_candles_ohlc_consistent CHECK (
        high >= low AND high >= open AND high >= close AND low <= open AND low <= close
    )
);
