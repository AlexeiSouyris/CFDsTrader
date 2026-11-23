-- Curva de cada corrida de backtest (#299). Columnas = contracts/backtest_run_curve.v1: `points`
-- (capital y drawdown de cada día) y `buy_and_hold` (comprar y mantener de cada instrumento) son
-- JSONB con la forma del contrato. Una sola curva por corrida (la de backtest_runs). Solo el
-- motor (Rust, cfds-journal) escribe; el gateway y Python leen. Tabla de SOLO AGREGAR (lo impone
-- el trigger): volver a guardar la misma curva no guarda nada; otra distinta es un error.

CREATE TABLE backtest_run_curves (
    run_id       TEXT        PRIMARY KEY REFERENCES backtest_runs (run_id),
    points       JSONB       NOT NULL CHECK (jsonb_typeof(points) = 'array'),
    buy_and_hold JSONB       NOT NULL CHECK (jsonb_typeof(buy_and_hold) = 'array'),
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE FUNCTION backtest_run_curves_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'backtest_run_curves es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER backtest_run_curves_no_update_or_delete
    BEFORE UPDATE OR DELETE ON backtest_run_curves
    FOR EACH ROW EXECUTE FUNCTION backtest_run_curves_append_only();
