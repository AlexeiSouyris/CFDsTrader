-- Variables del modelo de ML v1 que no viajan en la propuesta (docs/spec/ml-v1.md §3, #158).
-- Columnas = contracts/ml_features.v1. Una fila por propuesta, de SOLO AGREGAR (trigger): una
-- variable de una propuesta ya armada no cambia nunca (spec §8, sin repintado). Escribe el motor
-- (Rust); lee Python (`cfds_intel.models`).

CREATE TABLE ml_features (
    proposal_id          TEXT        PRIMARY KEY REFERENCES proposals (id),
    as_of                TIMESTAMPTZ NOT NULL,
    atr14_m15            NUMERIC     NOT NULL CHECK (atr14_m15 > 0),
    realized_vol_tercile TEXT        NOT NULL CHECK (realized_vol_tercile IN ('low', 'mid', 'high')),
    session_band         TEXT        NOT NULL CHECK (session_band IN ('open', 'mid', 'close')),
    inserted_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE FUNCTION ml_features_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'ml_features es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ml_features_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ml_features
    FOR EACH ROW EXECUTE FUNCTION ml_features_append_only();
