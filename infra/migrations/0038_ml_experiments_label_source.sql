-- #409: un experimento de ML puede salir de una corrida de backtest (`backtest_runs`, con P/L) o
-- de una corrida de etiquetas sin P/L (`ml_label_runs`, #399). Antes, `backtest_run_id` tenía
-- que estar en `backtest_runs` y las corridas sin P/L no se podían registrar.
--
-- `label_source` dice de qué tabla es la corrida (contracts/ml_experiment.v1, campo opcional;
-- los experimentos ya guardados son de `backtest_runs`). La integridad sigue en la base: dos
-- columnas generadas, cada una con su clave foránea, de las que solo una tiene valor. La tabla
-- sigue siendo de solo agregar (el trigger no cambia; ALTER TABLE no lo dispara).

ALTER TABLE ml_experiments
    ADD COLUMN label_source TEXT NOT NULL DEFAULT 'backtest_runs'
        CHECK (label_source IN ('backtest_runs', 'ml_label_runs'));

ALTER TABLE ml_experiments DROP CONSTRAINT ml_experiments_backtest_run_id_fkey;

ALTER TABLE ml_experiments
    ADD COLUMN backtest_run_ref TEXT GENERATED ALWAYS AS (
        CASE WHEN label_source = 'backtest_runs' THEN backtest_run_id END
    ) STORED REFERENCES backtest_runs (run_id),
    ADD COLUMN ml_label_run_ref TEXT GENERATED ALWAYS AS (
        CASE WHEN label_source = 'ml_label_runs' THEN backtest_run_id END
    ) STORED REFERENCES ml_label_runs (run_id);
