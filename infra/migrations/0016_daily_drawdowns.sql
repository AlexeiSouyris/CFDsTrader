-- Caída desde el máximo de las velas diarias (#213, spec §9.1). Columnas = contracts/daily_drawdown.v1.
-- La escribe el motor; Python la lee (índice de sentimiento) y no la recalcula (CLAUDE.md §1).
-- Un valor guardado no cambia nunca: recalcular y obtener otro es un error (sin repintado).
-- Va como 0016 porque ML usa la 0015.

CREATE TABLE daily_drawdowns (
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    day           DATE        NOT NULL,
    published_at  TIMESTAMPTZ NOT NULL,
    drawdown_window   INTEGER     NOT NULL CHECK (drawdown_window >= 1),
    drawdown      NUMERIC     NULL CHECK (drawdown <= 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- El valor del día D existe recién al terminar D (como en daily_indicators).
    CONSTRAINT daily_drawdowns_published_after_day
        CHECK (published_at >= (day::timestamp AT TIME ZONE 'UTC') + INTERVAL '12 hours'),
    CONSTRAINT daily_drawdowns_key UNIQUE (instrument_id, day, drawdown_window)
);

CREATE FUNCTION daily_drawdowns_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'daily_drawdowns es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER daily_drawdowns_no_update_or_delete
    BEFORE UPDATE OR DELETE ON daily_drawdowns
    FOR EACH ROW EXECUTE FUNCTION daily_drawdowns_append_only();
