-- Tercio de la volatilidad realizada diaria frente a su propia historia (spec §13, spec ML §3,
-- #158). Columnas = contracts/daily_vol_tercile.v1. La escribe el motor (cfds-ingest); la leen
-- los análogos (Python) y las variables de ML (Rust), y nadie la recalcula (CLAUDE.md §1).
-- Un valor guardado no cambia nunca: recalcular y obtener otro es un error (sin repintado).

CREATE TABLE daily_vol_terciles (
    instrument_id       TEXT        NOT NULL REFERENCES instruments (id),
    day                 DATE        NOT NULL,
    published_at        TIMESTAMPTZ NOT NULL,
    realized_vol_window INTEGER     NOT NULL CHECK (realized_vol_window >= 2),
    history_years       INTEGER     NOT NULL CHECK (history_years >= 1),
    tercile             TEXT        NULL CHECK (tercile IN ('low', 'mid', 'high')),
    inserted_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- El valor del día D existe recién al terminar D (como en daily_indicators).
    CONSTRAINT daily_vol_terciles_published_after_day
        CHECK (published_at >= (day::timestamp AT TIME ZONE 'UTC') + INTERVAL '12 hours'),
    CONSTRAINT daily_vol_terciles_key UNIQUE (instrument_id, day, realized_vol_window, history_years)
);

CREATE FUNCTION daily_vol_terciles_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'daily_vol_terciles es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER daily_vol_terciles_no_update_or_delete
    BEFORE UPDATE OR DELETE ON daily_vol_terciles
    FOR EACH ROW EXECUTE FUNCTION daily_vol_terciles_append_only();
