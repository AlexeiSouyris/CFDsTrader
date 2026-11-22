-- Indicadores de las velas diarias (#148, spec §13). Columnas = contracts/daily_indicator.v1.
-- Los escribe el motor; Python los lee (análogos) y no los recalcula (CLAUDE.md §1).
-- Un valor guardado no cambia nunca: recalcular y obtener otro es un error (sin repintado).
-- Va como 0009 porque #145 (abierto) usa la 0008.

CREATE TABLE daily_indicators (
    instrument_id       TEXT        NOT NULL REFERENCES instruments (id),
    day                 DATE        NOT NULL,
    published_at        TIMESTAMPTZ NOT NULL,
    atr_period          INTEGER     NOT NULL CHECK (atr_period >= 1),
    atr                 NUMERIC     NULL CHECK (atr >= 0),
    realized_vol_window INTEGER     NOT NULL CHECK (realized_vol_window >= 2),
    realized_vol        NUMERIC     NULL CHECK (realized_vol >= 0),
    inserted_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- El valor del día D existe recién al terminar D (en cualquier zona de Europa o América).
    CONSTRAINT daily_indicators_published_after_day
        CHECK (published_at >= (day::timestamp AT TIME ZONE 'UTC') + INTERVAL '12 hours'),
    CONSTRAINT daily_indicators_key UNIQUE (instrument_id, day, atr_period, realized_vol_window)
);

CREATE FUNCTION daily_indicators_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'daily_indicators es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER daily_indicators_no_update_or_delete
    BEFORE UPDATE OR DELETE ON daily_indicators
    FOR EACH ROW EXECUTE FUNCTION daily_indicators_append_only();
