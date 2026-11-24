-- Etiqueta del modelo de ML v1 sin P/L (docs/spec/ml-v1.md §1, #399). Columnas =
-- contracts/ml_label_run.v1 (`ml_label_runs`, de qué datos sale una corrida) y
-- contracts/ml_label.v1 (`ml_labels`, una fila por propuesta resuelta). Las dos son de SOLO
-- AGREGAR (trigger): una etiqueta ya guardada no cambia nunca. Escribe el motor (Rust, ejemplo `ml_dataset`); lee
-- Python (`cfds_intel.models`).

-- `from` y `to` se llaman `from_at` y `to_at`, como en backtest_runs.
CREATE TABLE ml_label_runs (
    run_id        TEXT        PRIMARY KEY CHECK (run_id ~ '^[A-Za-z0-9._-]{1,100}$'),
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    candle_source TEXT        NOT NULL CHECK (candle_source IN ('futures', 'ig')),
    from_at       TIMESTAMPTZ NOT NULL,
    to_at         TIMESTAMPTZ NOT NULL,
    method        TEXT        NOT NULL CHECK (method = 'mid_no_costs'),
    config_sha256 TEXT        NOT NULL CHECK (config_sha256 ~ '^[0-9a-f]{64}$'),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ml_label_runs_period CHECK (to_at > from_at)
);

CREATE TABLE ml_labels (
    proposal_id TEXT        PRIMARY KEY REFERENCES proposals (id),
    signal_at   TIMESTAMPTZ NOT NULL,
    y           SMALLINT    NOT NULL CHECK (y IN (0, 1)),
    exit_reason TEXT        NOT NULL CHECK (exit_reason IN ('tp1', 'stop', 'time', 'forced')),
    resolved_at TIMESTAMPTZ NOT NULL,
    method      TEXT        NOT NULL CHECK (method = 'mid_no_costs'),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- TP1 es y = 1; cualquier otra salida es y = 0.
    CONSTRAINT ml_labels_y_matches_reason CHECK ((y = 1) = (exit_reason = 'tp1')),
    CONSTRAINT ml_labels_resolved_after_signal CHECK (resolved_at > signal_at)
);

CREATE FUNCTION ml_labels_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ml_labels_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ml_labels
    FOR EACH ROW EXECUTE FUNCTION ml_labels_append_only();

CREATE TRIGGER ml_label_runs_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ml_label_runs
    FOR EACH ROW EXECUTE FUNCTION ml_labels_append_only();
