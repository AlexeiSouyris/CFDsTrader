-- Kill switch del broker simulado, guardado (#441). Hasta acá vivía solo en memoria: después de
-- un reinicio del gateway el sistema volvía a operar sin la votación que pide la spec §1.2.
--
-- Solo agregar (trigger: sin UPDATE ni DELETE). Cada fila es un cambio, en orden (seq):
-- - 'activated': lo escribe el motor (cfds-gateway) cuando el broker activa el kill switch
--   (límite, error de datos, desconexión o reconciliación), con el motivo y la hora.
-- - 'released': la liberación después de la votación de la spec §1.2. El motor **nunca** la
--   escribe: la agrega una persona con el gateway apagado, con el resultado de la votación en
--   `reason`, y después arranca el gateway.
-- `after_orders` es cuántas órdenes había en `paper_orders` cuando pasó: al reconstruir el
-- broker (`restore_with_kill_switch`) el cambio se aplica en ese mismo punto del registro.
-- Solo modo paper: no hay órdenes reales.

CREATE TABLE paper_kill_switch (
    seq          BIGINT      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    change       TEXT        NOT NULL CHECK (change IN ('activated', 'released')),
    after_orders BIGINT      NOT NULL CHECK (after_orders >= 0),
    reason       TEXT        NOT NULL CHECK (length(btrim(reason)) > 0),
    at           TIMESTAMPTZ NOT NULL,
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER paper_kill_switch_no_update_or_delete
    BEFORE UPDATE OR DELETE ON paper_kill_switch
    FOR EACH ROW EXECUTE FUNCTION paper_orders_append_only();
