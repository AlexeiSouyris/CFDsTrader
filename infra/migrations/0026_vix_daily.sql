-- VIX diario de Cboe (ADR 0012 §2). Columnas = contracts/vix_daily.v1. Tabla de SOLO AGREGAR:
-- una corrección de Cboe es una fila nueva (otra published_at), nunca se pisa la anterior.
-- Se usa por decisión del dueño, solo en la base del Mac: los datos nunca van al repo.
-- La escribe Python (cfds_intel.vix). Es contexto: no entra al puntaje.

CREATE TABLE vix_daily (
    ref_date     DATE        NOT NULL,
    published_at TIMESTAMPTZ NOT NULL,
    open         NUMERIC     NOT NULL CHECK (open > 0),
    high         NUMERIC     NOT NULL CHECK (high > 0),
    low          NUMERIC     NOT NULL CHECK (low > 0),
    close        NUMERIC     NOT NULL CHECK (close > 0),
    inserted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT vix_daily_ohlc CHECK (low <= open AND low <= close AND high >= open AND high >= close),
    -- El VIX cierra a las 16:15 ET: antes de eso el valor del día no existe.
    CONSTRAINT vix_daily_published_after_close
        CHECK (published_at >= (ref_date + TIME '16:15') AT TIME ZONE 'America/New_York'),
    CONSTRAINT vix_daily_key UNIQUE (ref_date, published_at)
);

CREATE INDEX vix_daily_published_idx ON vix_daily (published_at);

CREATE FUNCTION vix_daily_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'vix_daily es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER vix_daily_no_update_or_delete
    BEFORE UPDATE OR DELETE ON vix_daily
    FOR EACH ROW EXECUTE FUNCTION vix_daily_append_only();
