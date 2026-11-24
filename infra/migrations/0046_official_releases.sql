-- Calendario histórico oficial (contracts/official_release.v1, ADR 0016, #520). Columnas = las
-- del contrato. Lo escribe Python (`python -m cfds_intel.calendar.official`): hoy los
-- comunicados del FOMC (federalreserve.gov); el CPI y el empleo del BLS entran después en la
-- misma tabla, sin cambiarla. Lo van a leer el estudio de la familia E y, si se aprueba, el
-- motor.
--
-- De solo agregar (trigger): una fila existe desde `published_at`. Una revisión del calendario
-- o una reprogramación es otra fila; la anterior no se pisa ni se borra.
-- Aparte de `economic_events` a propósito (ADR 0016 §5): otro contrato y otro uso.

CREATE TABLE official_releases (
    source                TEXT        NOT NULL CHECK (source IN ('fed_fomc', 'bls')),
    series                TEXT        NOT NULL
        CHECK (series IN ('fomc_statement', 'cpi', 'employment_situation')),
    reference_period      TEXT        NOT NULL
        CHECK (reference_period ~ '^\d{4}-(0[1-9]|1[0-2])(-(0[1-9]|[12]\d|3[01]))?$'),
    status                TEXT        NOT NULL
        CHECK (status IN ('scheduled', 'rescheduled', 'cancelled', 'unscheduled')),
    scheduled_at          TIMESTAMPTZ NULL,
    published_at          TIMESTAMPTZ NOT NULL,
    time_rule             TEXT        NOT NULL CHECK (time_rule ~ '^[a-z][a-z0-9_]{0,63}$'),
    with_press_conference BOOLEAN     NULL,
    source_url            TEXT        NOT NULL
        CHECK (source_url ~ '^https://' AND length(source_url) <= 500),
    inserted_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Cada serie con su organismo y su forma de período (fecha de reunión o mes de referencia).
    CONSTRAINT official_releases_series_of_source CHECK (
        (source = 'fed_fomc' AND series = 'fomc_statement'
            AND reference_period ~ '^\d{4}-\d{2}-\d{2}$' AND with_press_conference IS NOT NULL)
        OR (source = 'bls' AND series IN ('cpi', 'employment_situation')
            AND reference_period ~ '^\d{4}-\d{2}$' AND with_press_conference IS NULL)
    ),
    -- Sin hora solo si se canceló.
    CONSTRAINT official_releases_scheduled_unless_cancelled
        CHECK ((status = 'cancelled') = (scheduled_at IS NULL)),
    -- Anti look-ahead (ADR 0016 §4): se sabía antes o en el momento; si no estaba programada,
    -- se supo recién al anunciarse.
    CONSTRAINT official_releases_known_before
        CHECK (scheduled_at IS NULL OR published_at <= scheduled_at),
    CONSTRAINT official_releases_unscheduled_known_at_release
        CHECK (status <> 'unscheduled' OR published_at = scheduled_at),
    -- Clave natural (ADR 0016 §5): otra corrida no agrega nada.
    CONSTRAINT official_releases_key
        UNIQUE NULLS NOT DISTINCT (source, series, reference_period, status, scheduled_at)
);

CREATE INDEX official_releases_by_time ON official_releases (series, scheduled_at, published_at);

CREATE FUNCTION official_releases_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'official_releases es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER official_releases_no_update_or_delete
    BEFORE UPDATE OR DELETE ON official_releases
    FOR EACH ROW EXECUTE FUNCTION official_releases_append_only();
