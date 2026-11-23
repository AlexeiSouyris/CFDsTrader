-- Eventos de la operación del gateway (#337, sigue a #314). Contrato: contracts/ops_event.v1.
-- Cada fila es una línea del log de cfds-obs YA REDACTADA (sin secretos: los tapa el Logger
-- antes de que llegue acá): eventos del broker, alertas y todo lo warn o error. La escribe solo
-- el gateway, en segundo plano, sin frenar nunca el broker. (0034 es de learning_votes, #330.)
--
-- SOLO AGREGAR: el trigger no deja modificar nunca. Borrar solo lo puede el job de retención
-- (`cargo run -p cfds-gateway --example ops_events_retention`, `[ops] retention_days` de
-- config/gateway.toml), que lo pide en su propia transacción con
-- `SET LOCAL cfds.ops_events_retention = 'on'`. Cualquier otro DELETE falla.

CREATE TABLE ops_events (
    id          BIGSERIAL   PRIMARY KEY,
    ts          TIMESTAMPTZ NOT NULL,
    level       TEXT        NOT NULL CHECK (level IN ('info', 'warn', 'error')),
    target      TEXT        NOT NULL CHECK (target ~ '^[a-z_]{1,32}$'),
    event       TEXT        NOT NULL CHECK (length(event) BETWEEN 1 AND 64),
    fields      JSONB       NOT NULL CHECK (jsonb_typeof(fields) = 'object'),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Los últimos primero; a igual hora, el orden en que se escribieron.
CREATE INDEX ops_events_latest_idx ON ops_events (ts DESC, id DESC);

CREATE FUNCTION ops_events_append_only() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'DELETE' AND current_setting('cfds.ops_events_retention', true) = 'on' THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'ops_events es de solo agregar: no se modifica, y solo el job de retención borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ops_events_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ops_events
    FOR EACH ROW EXECUTE FUNCTION ops_events_append_only();
