-- Tiingo, plan gratis, solo para verificación transitoria (ADR 0009, #206). Los términos del plan
-- prohíben guardar datos de Tiingo y productos derivados (§1.6(a) y (c)): estas tablas NUNCA
-- tienen precios ni números que salgan de Tiingo. Escribe el motor (`cfds-tiingo`). Las dos son de
-- SOLO AGREGAR (lo imponen los triggers).

-- Un pedido a Tiingo por fila: para contar el uso contra los topes de config/tiingo.toml, con
-- ventanas móviles. `symbol` es el símbolo que pedimos nosotros (configuración, no un dato de
-- Tiingo). `http_status` es NULL si no hubo respuesta (error de red).
CREATE TABLE tiingo_usage_log (
    id           BIGSERIAL   PRIMARY KEY,
    requested_at TIMESTAMPTZ NOT NULL,
    endpoint     TEXT        NOT NULL CHECK (endpoint ~ '^[a-z][a-z0-9_]{0,31}$'),
    symbol       TEXT        NOT NULL CHECK (symbol ~ '^[A-Z0-9][A-Z0-9.-]{0,15}$'),
    http_status  INTEGER     CHECK (http_status BETWEEN 100 AND 599),
    bytes        BIGINT      NOT NULL CHECK (bytes >= 0),
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX tiingo_usage_log_requested_idx ON tiingo_usage_log (requested_at);

-- Veredicto de cada día verificado (opción B del ADR 0009): solo "coincide", "no coincide" o
-- "sin dato", sin la diferencia ni ningún precio. Verificar otra vez el mismo día es otra fila.
CREATE TABLE tiingo_verifications (
    id            BIGSERIAL   PRIMARY KEY,
    verified_at   TIMESTAMPTZ NOT NULL,
    instrument_id TEXT        NOT NULL REFERENCES instruments (id),
    day           DATE        NOT NULL,
    etf           TEXT        NOT NULL CHECK (etf ~ '^[A-Z0-9][A-Z0-9.-]{0,15}$'),
    -- Con qué cierre nuestro se comparó: la vela de 15m que termina a la hora de referencia, o
    -- la vela diaria. 'none' si no hubo con qué comparar.
    method        TEXT        NOT NULL CHECK (method IN ('m15_close', 'daily_close', 'none')),
    verdict       TEXT        NOT NULL CHECK (verdict IN ('match', 'mismatch', 'no_data')),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT tiingo_no_data_without_method CHECK ((method = 'none') = (verdict = 'no_data'))
);

CREATE INDEX tiingo_verifications_day_idx ON tiingo_verifications (instrument_id, day);

CREATE FUNCTION tiingo_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER tiingo_usage_log_no_update_or_delete
    BEFORE UPDATE OR DELETE ON tiingo_usage_log
    FOR EACH ROW EXECUTE FUNCTION tiingo_append_only();

CREATE TRIGGER tiingo_verifications_no_update_or_delete
    BEFORE UPDATE OR DELETE ON tiingo_verifications
    FOR EACH ROW EXECUTE FUNCTION tiingo_append_only();
