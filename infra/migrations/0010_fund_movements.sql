-- Historial del fondo del grupo (spec §14, #179): cada aporte o retiro ya liquidado, con las
-- unidades y el NAV. Solo se agrega: el estado de cada miembro se reconstruye sumando esto.
-- Contrato: contracts/fund_movement.v1.schema.json. Lo escribe el motor (cfds-fund).

CREATE TABLE fund_movements (
    id          TEXT        PRIMARY KEY CHECK (id ~ '^fund:\d{4}-\d{2}-\d{2}:[1-9]\d{0,3}$'),
    settled_at  TIMESTAMPTZ NOT NULL,
    member_id   TEXT        NOT NULL CHECK (member_id ~ '^[a-z0-9][a-z0-9_-]{0,63}$'),
    kind        TEXT        NOT NULL CHECK (kind IN ('contribution', 'withdrawal')),
    amount      NUMERIC     NOT NULL CHECK (amount > 0 AND amount = round(amount, 2)),
    units       NUMERIC     NOT NULL CHECK (units > 0),
    nav         NUMERIC     NOT NULL CHECK (nav > 0),
    currency    TEXT        NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
    inserted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX fund_movements_order_idx ON fund_movements (settled_at, id);
CREATE INDEX fund_movements_member_idx ON fund_movements (member_id, settled_at);

CREATE FUNCTION fund_movements_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'fund_movements es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER fund_movements_no_update_or_delete
    BEFORE UPDATE OR DELETE ON fund_movements
    FOR EACH ROW EXECUTE FUNCTION fund_movements_append_only();
