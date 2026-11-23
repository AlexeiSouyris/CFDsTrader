-- La probabilidad mostrada (spec §13) de cada propuesta, tal como se mostró (#333). Campo
-- opcional de proposal.v1 (cambio compatible: misma versión). Las propuestas del backtest no la
-- traen (queda NULL) y el informe de aprendizaje la recalcula con la tabla de entrenamiento.
-- La tabla sigue siendo de solo agregar: una propuesta guardada nunca cambia.

ALTER TABLE proposals
    ADD COLUMN shown JSONB NULL
        CHECK (shown IS NULL OR (jsonb_typeof(shown) = 'object'
            AND shown ->> 'status' IN ('estimate', 'insufficient')));
