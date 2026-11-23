-- Rendimientos diarios de la curva par del Tesoro de EE. UU. (ADR 0012). Columnas =
-- contracts/treasury_yield.v1. Tabla de SOLO AGREGAR: una corrección del Tesoro es una fila
-- nueva (otra published_at), nunca se pisa la anterior. El trigger lo impone en la base.
-- La escribe Python (cfds_intel.treasury). Es contexto macro: no entra al puntaje.
-- (Se numera 0025: la 0024 queda para otro PR en curso.)

CREATE TABLE treasury_yields (
    tenor        TEXT        NOT NULL CHECK (tenor IN ('1M', '1_5M', '2M', '3M', '4M', '6M',
                                                       '1Y', '2Y', '3Y', '5Y', '7Y', '10Y',
                                                       '20Y', '30Y')),
    ref_date     DATE        NOT NULL,
    published_at TIMESTAMPTZ NOT NULL,
    yield_pct    NUMERIC     NOT NULL,
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Los precios se toman cerca de las 15:30 ET de ref_date: antes de eso el valor no existe.
    CONSTRAINT treasury_yields_published_after_inputs
        CHECK (published_at >= (ref_date + TIME '15:30') AT TIME ZONE 'America/New_York'),
    CONSTRAINT treasury_yields_key UNIQUE (tenor, ref_date, published_at)
);

CREATE INDEX treasury_yields_tenor_published_idx ON treasury_yields (tenor, published_at);

CREATE FUNCTION treasury_yields_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'treasury_yields es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER treasury_yields_no_update_or_delete
    BEFORE UPDATE OR DELETE ON treasury_yields
    FOR EACH ROW EXECUTE FUNCTION treasury_yields_append_only();
