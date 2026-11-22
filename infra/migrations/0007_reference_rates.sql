-- Tipos de referencia diarios de la financiación nocturna (#79). Columnas =
-- contracts/reference_rate.v1. Tabla de SOLO AGREGAR: una corrección de la fuente es una fila
-- nueva (otra published_at), nunca se pisa la anterior. El trigger lo impone en la base.
-- La escribe Python (cfds_intel.rates); el motor la lee para el costo de financiación.

CREATE TABLE reference_rates (
    series       TEXT        NOT NULL CHECK (series ~ '^[A-Z][A-Z0-9_]{0,31}$'),
    ref_date     DATE        NOT NULL,
    published_at TIMESTAMPTZ NOT NULL,
    rate_pct     NUMERIC     NOT NULL,
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Un valor nunca está disponible antes de que termine su día de referencia (en UTC, el
    -- límite más temprano posible para cualquier zona horaria de Europa).
    CONSTRAINT reference_rates_published_after_day
        CHECK (published_at >= (ref_date + 1)::timestamp AT TIME ZONE 'UTC' - INTERVAL '2 hours'),
    CONSTRAINT reference_rates_key UNIQUE (series, ref_date, published_at)
);

CREATE INDEX reference_rates_series_published_idx ON reference_rates (series, published_at);

CREATE FUNCTION reference_rates_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'reference_rates es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER reference_rates_no_update_or_delete
    BEFORE UPDATE OR DELETE ON reference_rates
    FOR EACH ROW EXECUTE FUNCTION reference_rates_append_only();
