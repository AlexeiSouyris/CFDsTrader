-- Corridas de backtest del diario (#200). Columnas = contracts/backtest_run.v1 (`from` y `to` se
-- llaman from_at y to_at; `costs`, `news` y `metrics` son JSONB con la forma del contrato, y
-- `news` es NULL si la corrida no usó noticias). Las operaciones de una corrida son las filas de
-- `proposals` con el mismo run_id (proposal.v1 no cambia). Solo el motor (Rust) escribe; Python
-- lee. Tabla de SOLO AGREGAR (lo impone el trigger): volver a guardar la misma corrida no guarda
-- nada; con otro contenido es un error del motor.

CREATE TABLE backtest_runs (
    run_id         TEXT        PRIMARY KEY CHECK (run_id ~ '^[A-Za-z0-9._-]{1,100}$'),
    instruments    JSONB       NOT NULL
        CHECK (jsonb_typeof(instruments) = 'array' AND jsonb_array_length(instruments) >= 1),
    timeframe      TEXT        NOT NULL CHECK (timeframe IN ('15m', '1h')),
    from_at        TIMESTAMPTZ NOT NULL,
    to_at          TIMESTAMPTZ NOT NULL,
    config_sha256  TEXT        NOT NULL CHECK (config_sha256 ~ '^[0-9a-f]{64}$'),
    initial_equity NUMERIC     NOT NULL CHECK (initial_equity > 0),
    costs          JSONB       NOT NULL CHECK (jsonb_typeof(costs) = 'object'),
    news           JSONB                CHECK (news IS NULL OR jsonb_typeof(news) = 'object'),
    metrics        JSONB       NOT NULL CHECK (jsonb_typeof(metrics) = 'object'),
    started_at     TIMESTAMPTZ NOT NULL,
    finished_at    TIMESTAMPTZ NOT NULL,
    inserted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT backtest_runs_period CHECK (to_at > from_at),
    CONSTRAINT backtest_runs_finished_after_started CHECK (finished_at >= started_at)
);

CREATE FUNCTION backtest_runs_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'backtest_runs es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER backtest_runs_no_update_or_delete
    BEFORE UPDATE OR DELETE ON backtest_runs
    FOR EACH ROW EXECUTE FUNCTION backtest_runs_append_only();
