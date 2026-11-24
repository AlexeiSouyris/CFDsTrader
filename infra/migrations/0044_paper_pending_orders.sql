-- Órdenes pendientes del broker simulado, guardadas (#479). Una orden manual de ahora todavía no
-- tiene su vela del fill: si el riesgo la aceptaría queda pendiente fuera del registro y se decide
-- cuando llega esa vela (#471). Hasta acá vivía solo en memoria: un reinicio del gateway antes de
-- esa vela la perdía.
--
-- Dos tablas de solo agregar (trigger: sin UPDATE ni DELETE), como `paper_orders` (0028):
-- - paper_pending_orders: cada orden pendiente (`order.v1` entero), una vez por order_id, en orden
--   de llegada (seq). Lo escribe el motor (cfds-gateway) cuando el broker la deja pendiente.
-- - paper_pending_outcomes: cómo terminó, una sola vez por orden:
--   - 'settled': se decidió y entró al registro (`paper_orders`). Se escribe en la **misma**
--     transacción que la orden y su informe (`insert_paper_order`).
--   - 'dropped': no se pudo retomar al arrancar (su instrumento ya no se opera). No entró al
--     registro; `reason` dice por qué.
-- Al arrancar, el motor retoma las que no tienen fila en `paper_pending_outcomes`, en orden, y las
-- decide igual que la sesión en marcha y la reconstrucción: cuando llega su vela del fill.
-- Solo modo paper: no hay órdenes reales.

CREATE TABLE paper_pending_orders (
    seq           BIGINT      GENERATED ALWAYS AS IDENTITY UNIQUE,
    order_id      TEXT        PRIMARY KEY CHECK (order_id ~ '^[A-Za-z0-9._:-]{1,200}$'),
    instrument_id TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL,
    document      JSONB       NOT NULL,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT paper_pending_orders_paper_only CHECK (document ->> 'mode' = 'paper'),
    CONSTRAINT paper_pending_orders_same_id CHECK (document ->> 'order_id' = order_id)
);

CREATE TABLE paper_pending_outcomes (
    order_id    TEXT        PRIMARY KEY REFERENCES paper_pending_orders (order_id),
    outcome     TEXT        NOT NULL CHECK (outcome IN ('settled', 'dropped')),
    reason      TEXT,
    at          TIMESTAMPTZ NOT NULL,
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Una descartada siempre dice por qué; una decidida no lleva motivo (su informe está en
    -- `paper_order_reports`).
    CONSTRAINT paper_pending_outcomes_reason CHECK (
        (outcome = 'dropped' AND reason IS NOT NULL AND length(btrim(reason)) > 0)
        OR (outcome = 'settled' AND reason IS NULL)
    )
);

CREATE TRIGGER paper_pending_orders_no_update_or_delete
    BEFORE UPDATE OR DELETE ON paper_pending_orders
    FOR EACH ROW EXECUTE FUNCTION paper_orders_append_only();

CREATE TRIGGER paper_pending_outcomes_no_update_or_delete
    BEFORE UPDATE OR DELETE ON paper_pending_outcomes
    FOR EACH ROW EXECUTE FUNCTION paper_orders_append_only();
