-- Va como 0028: la 0027 queda libre para las diarias de futuros (#282), que van en paralelo.
-- Registro de órdenes del broker simulado (fase 7, #278). Columnas = contracts/order.v1 y
-- contracts/order_report.v1 (el documento entero en JSONB, más las columnas para buscar).
-- Lo escribe el motor (cfds-broker); con él se reconstruye y se reconcilia el broker al arrancar
-- y cada N minutos (`restore_and_reconcile`). Solo modo paper: no hay órdenes reales.
--
-- Dos tablas de solo agregar (trigger: sin UPDATE ni DELETE):
-- - paper_orders: cada orden, una sola vez por order_id (idempotente), en orden de llegada (seq).
-- - paper_order_reports: el informe de cada orden, uno solo, que nunca se reescribe.

CREATE TABLE paper_orders (
    seq           BIGINT      GENERATED ALWAYS AS IDENTITY UNIQUE,
    order_id      TEXT        PRIMARY KEY CHECK (order_id ~ '^[A-Za-z0-9._:-]{1,200}$'),
    instrument_id TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL,
    document      JSONB       NOT NULL,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT paper_orders_paper_only CHECK (document ->> 'mode' = 'paper'),
    CONSTRAINT paper_orders_same_id CHECK (document ->> 'order_id' = order_id)
);

CREATE TABLE paper_order_reports (
    order_id    TEXT        PRIMARY KEY REFERENCES paper_orders (order_id),
    status      TEXT        NOT NULL CHECK (status IN ('rejected', 'filled')),
    at          TIMESTAMPTZ NOT NULL,
    document    JSONB       NOT NULL,
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT paper_order_reports_same_id CHECK (document ->> 'order_id' = order_id),
    CONSTRAINT paper_order_reports_same_status CHECK (document ->> 'status' = status)
);

CREATE FUNCTION paper_orders_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER paper_orders_no_update_or_delete
    BEFORE UPDATE OR DELETE ON paper_orders
    FOR EACH ROW EXECUTE FUNCTION paper_orders_append_only();

CREATE TRIGGER paper_order_reports_no_update_or_delete
    BEFORE UPDATE OR DELETE ON paper_order_reports
    FOR EACH ROW EXECUTE FUNCTION paper_orders_append_only();
