-- Pronóstico de Forex Factory y dato real oficial (ADR 0018, #695). Dos tablas aparte de
-- `economic_events` a propósito (ADR 0018 §3): `economic_event.v1` y el filtro §7.2 no cambian.
-- Columnas = las de los contratos `calendar_estimate.v1` y `release_actual.v1`. Las escribe
-- Python; la sorpresa (dato real − pronóstico) NO se guarda: la calcula Rust al leerlas.
--
-- Las dos son de solo agregar (trigger), como `economic_events` y `official_releases`.

-- Instantáneas del `forecast` y el `previous` (ADR 0018 §1). Hay fila nueva solo cuando el par
-- cambia respecto de la última instantánea del mismo evento (lo decide el guardado, en una
-- transacción). Por eso la clave lleva `seen_at` y no el par de valores: si el valor va de A a B
-- y vuelve a A, son tres filas, y la última antes de la hora del evento es A (con el par en la
-- clave, la vuelta a A no se podría guardar y el consenso quedaría en B).
CREATE TABLE calendar_estimates (
    source         TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    event          TEXT        NOT NULL CHECK (length(event) BETWEEN 1 AND 200),
    country        TEXT        NOT NULL CHECK (country ~ '^[A-Z]{2}$'),
    scheduled_at   TIMESTAMPTZ NOT NULL,
    forecast_raw   TEXT        NULL CHECK (length(forecast_raw) BETWEEN 1 AND 50),
    previous_raw   TEXT        NULL CHECK (length(previous_raw) BETWEEN 1 AND 50),
    forecast_value NUMERIC     NULL,
    previous_value NUMERIC     NULL,
    unit           TEXT        NULL
        CHECK (unit IN ('percent', 'thousands', 'millions', 'billions', 'trillions', 'level')),
    seen_at        TIMESTAMPTZ NOT NULL,
    inserted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Un número solo si hay texto; unidad si y solo si hay algún número.
    CONSTRAINT calendar_estimates_forecast_value_has_raw
        CHECK (forecast_value IS NULL OR forecast_raw IS NOT NULL),
    CONSTRAINT calendar_estimates_previous_value_has_raw
        CHECK (previous_value IS NULL OR previous_raw IS NOT NULL),
    CONSTRAINT calendar_estimates_unit_iff_value
        CHECK ((unit IS NULL) = (forecast_value IS NULL AND previous_value IS NULL)),
    -- Una instantánea por evento y por lectura.
    CONSTRAINT calendar_estimates_key UNIQUE (source, event, country, scheduled_at, seen_at)
);

-- La consulta de la sorpresa: la última instantánea de un evento antes de su hora.
CREATE INDEX calendar_estimates_by_event
    ON calendar_estimates (source, event, country, scheduled_at, seen_at);

CREATE FUNCTION calendar_estimates_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'calendar_estimates es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER calendar_estimates_no_update_or_delete
    BEFORE UPDATE OR DELETE ON calendar_estimates
    FOR EACH ROW EXECUTE FUNCTION calendar_estimates_append_only();

-- Dato real oficial (ADR 0018 §2): hoy CPI y empleo del BLS por su API pública v1.
CREATE TABLE release_actuals (
    source           TEXT        NOT NULL CHECK (source IN ('bls')),
    series_id        TEXT        NOT NULL CHECK (series_id ~ '^[A-Z0-9][A-Z0-9_#-]{0,39}$'),
    reference_period TEXT        NOT NULL CHECK (reference_period ~ '^\d{4}-(0[1-9]|1[0-2])$'),
    value            NUMERIC     NOT NULL,
    unit             TEXT        NOT NULL
        CHECK (unit IN ('percent', 'thousands', 'millions', 'billions', 'trillions', 'level')),
    vintage          TEXT        NOT NULL CHECK (vintage IN ('first', 'revised')),
    released_at      TIMESTAMPTZ NOT NULL,
    retrieved_at     TIMESTAMPTZ NOT NULL,
    source_url       TEXT        NOT NULL
        CHECK (source_url ~ '^https://' AND length(source_url) <= 500),
    inserted_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Anti look-ahead: no se lee antes de que salga.
    CONSTRAINT release_actuals_retrieved_after_release CHECK (retrieved_at >= released_at),
    -- Clave natural (ADR 0018 §3): otra lectura con el mismo valor no agrega nada.
    CONSTRAINT release_actuals_key UNIQUE (source, series_id, reference_period, vintage, value)
);

-- Un solo primer dato por serie y mes.
CREATE UNIQUE INDEX release_actuals_one_first
    ON release_actuals (source, series_id, reference_period) WHERE vintage = 'first';

CREATE FUNCTION release_actuals_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'release_actuals es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER release_actuals_no_update_or_delete
    BEFORE UPDATE OR DELETE ON release_actuals
    FOR EACH ROW EXECUTE FUNCTION release_actuals_append_only();
