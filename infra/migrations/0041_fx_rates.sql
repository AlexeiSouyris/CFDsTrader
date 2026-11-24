-- #418 (ADR 0013 §4): tipos de cambio diarios de referencia (EUR/USD del BCE). Columnas =
-- contracts/fx_rate.v1. Tabla de SOLO AGREGAR: una corrección de la fuente es una fila nueva
-- (otra published_at), nunca se pisa la anterior. El trigger lo impone en la base.
-- La escribe Python (cfds_intel.rates.fx); el motor la lee para convertir montos en USD a la
-- moneda de la cuenta con el tipo vigente en cada momento.

CREATE TABLE fx_rates (
    pair         TEXT        NOT NULL CHECK (pair ~ '^[A-Z]{6}$'),
    ref_date     DATE        NOT NULL,
    published_at TIMESTAMPTZ NOT NULL,
    rate         NUMERIC     NOT NULL CHECK (rate > 0),
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- El BCE publica el tipo del día ese mismo día: nunca antes de que empiece (en UTC, con
    -- margen para cualquier zona horaria de Europa).
    CONSTRAINT fx_rates_published_on_or_after_day
        CHECK (published_at >= ref_date::timestamp AT TIME ZONE 'UTC' - INTERVAL '2 hours'),
    CONSTRAINT fx_rates_key UNIQUE (pair, ref_date, published_at)
);

CREATE INDEX fx_rates_pair_published_idx ON fx_rates (pair, published_at);

CREATE FUNCTION fx_rates_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'fx_rates es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER fx_rates_no_update_or_delete
    BEFORE UPDATE OR DELETE ON fx_rates
    FOR EACH ROW EXECUTE FUNCTION fx_rates_append_only();
