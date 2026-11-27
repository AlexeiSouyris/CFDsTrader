-- Puntos de control del broker simulado (#463). Sin ellos, la reconstrucción del broker al
-- arrancar y en cada reconciliación reenvía todo el registro de órdenes sobre todas las velas
-- desde la primera orden (#438), y el costo de cada paso crece sin límite.
--
-- Cada fila es el estado del broker después de las primeras `after_orders` órdenes de
-- `paper_orders`: lo que ya no cambia (operaciones cerradas, selladas con su resultado), las
-- operaciones abiertas, el máximo del capital, el kill switch y, por instrumento, desde qué vela
-- se sigue y el estado del ATR de Wilder. Lo escribe el motor (cfds-gateway) solo después de
-- una reconciliación limpia. Ver `engine/crates/broker/src/checkpoint.rs`.
--
-- Verificable: `state` es el texto JSON exacto que se hashea. `hash` = SHA-256 de
-- (`prev_hash` o vacío) + "\n" + `state`, y `prev_hash` es el `hash` de la fila anterior: una
-- cadena. Además `state` lleva el SHA-256 del registro de órdenes que cubre y de la config del
-- broker. Al arrancar, el motor usa el último punto de control cuya cadena, registro y config
-- verifican; si ninguno sirve, reconstruye desde la primera orden como antes.
--
-- Solo agregar (trigger: sin UPDATE ni DELETE). Solo modo paper: no hay órdenes reales.

CREATE TABLE paper_broker_checkpoints (
    seq          BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    after_orders BIGINT      NOT NULL CHECK (after_orders >= 0),
    state        TEXT        NOT NULL,
    prev_hash    TEXT        UNIQUE REFERENCES paper_broker_checkpoints (hash),
    hash         TEXT        NOT NULL UNIQUE CHECK (hash ~ '^[0-9a-f]{64}$'),
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Una sola cadena: un solo punto de control sin anterior.
CREATE UNIQUE INDEX paper_broker_checkpoints_one_genesis
    ON paper_broker_checkpoints ((prev_hash IS NULL)) WHERE prev_hash IS NULL;

CREATE TRIGGER paper_broker_checkpoints_no_update_or_delete
    BEFORE UPDATE OR DELETE ON paper_broker_checkpoints
    FOR EACH ROW EXECUTE FUNCTION paper_orders_append_only();
