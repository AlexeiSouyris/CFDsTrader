-- Registro del cupo semanal de históricos de IG (#140, propuestas 1 y 2 del #132).
-- Una fila por respuesta de GET /prices: cuántos precios trajo y lo que IG dijo del cupo de
-- TODA la cuenta (restante, total y renovación). Con eso, la máquina que la escribe (el Mac
-- mini) deduce cuánto se gastó desde otro lado entre dos de sus corridas. Solo se agrega.

CREATE TABLE ig_allowance_log (
    id              BIGSERIAL   PRIMARY KEY,
    observed_at     TIMESTAMPTZ NOT NULL,
    command         TEXT        NOT NULL CHECK (command ~ '^[a-z][a-z0-9_]{0,63}$'),
    points_returned BIGINT      NOT NULL CHECK (points_returned >= 0),
    remaining       BIGINT      NOT NULL CHECK (remaining >= 0),
    total           BIGINT      NOT NULL CHECK (total > 0),
    resets_at       TIMESTAMPTZ NOT NULL CHECK (resets_at >= observed_at),
    inserted_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ig_allowance_log_observed_idx ON ig_allowance_log (observed_at, id);

CREATE FUNCTION ig_allowance_log_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'ig_allowance_log es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ig_allowance_log_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ig_allowance_log
    FOR EACH ROW EXECUTE FUNCTION ig_allowance_log_append_only();
