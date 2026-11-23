-- Número 0015: la 0014 es de la sesión de Tiingo (coordinado el 23-11-2026); se mergea después de ella.
-- Registro de experimentos del modelo de ML v1 (docs/spec/ml-v1.md §7, ADR 0003 §D, #158).
-- Columnas = contracts/ml_experiment.v1: los campos de arriba como columnas y el resto (variables,
-- hiperparámetros, ventanas, métricas y resultado) como JSONB con la forma del contrato. Se
-- registran todos, también los que no funcionan. Escribe Python (`cfds_intel.models`). Tabla de
-- SOLO AGREGAR (trigger): el mismo experimento otra vez no guarda nada.

CREATE TABLE ml_experiments (
    experiment_id   TEXT        PRIMARY KEY CHECK (experiment_id ~ '^[0-9a-f]{64}$'),
    created_at      TIMESTAMPTZ NOT NULL,
    code_version    TEXT        NOT NULL CHECK (code_version ~ '^[0-9a-f]{7,40}(-dirty)?$'),
    backtest_run_id TEXT        NOT NULL REFERENCES backtest_runs (run_id),
    config_sha256   TEXT        NOT NULL CHECK (config_sha256 ~ '^[0-9a-f]{64}$'),
    status          TEXT        NOT NULL CHECK (status IN ('suspended', 'm1_improves', 'b1_stays')),
    document        JSONB       NOT NULL CHECK (jsonb_typeof(document) = 'object'),
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ml_experiments_run_idx ON ml_experiments (backtest_run_id, created_at);

CREATE FUNCTION ml_experiments_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'ml_experiments es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ml_experiments_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ml_experiments
    FOR EACH ROW EXECUTE FUNCTION ml_experiments_append_only();
