-- Barras de 1 minuto de futuros desde Databento (ADR 0011, en propuesta; #247). Columnas =
-- contracts/futures_bar.v1. Las escribe Python (`python -m cfds_intel.databento --run`, solo en
-- la máquina de datos); las agrupa en 15m, 1h y diarias el motor en Rust. No se mezclan con las
-- velas de IG (`candles`).
--
-- Las dos tablas son de solo agregar (trigger): una barra guardada no cambia nunca.

CREATE TABLE futures_bars_1m (
    source        TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    dataset       TEXT        NOT NULL CHECK (dataset ~ '^[A-Z0-9]+\.[A-Z0-9]+$'),
    -- Contrato continuo pedido (por ejemplo ES.v.0) y el contrato real de la barra
    -- (instrument_id de Databento): cuando cambia, hubo un vencimiento (rollover).
    symbol        TEXT        NOT NULL CHECK (symbol ~ '^[A-Z0-9]{1,10}\.[cnv]\.[0-9]{1,2}$'),
    contract_id   BIGINT      NOT NULL CHECK (contract_id > 0),
    -- Nuestro instrumento (CFD de IG) al que corresponde el futuro (config/databento.toml).
    instrument_id TEXT        NOT NULL CHECK (instrument_id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    -- Inicio de la barra, en UTC, justo en un minuto.
    start_at      TIMESTAMPTZ NOT NULL CHECK (date_trunc('minute', start_at) = start_at),
    open          NUMERIC     NOT NULL CHECK (open > 0),
    high          NUMERIC     NOT NULL CHECK (high > 0),
    low           NUMERIC     NOT NULL CHECK (low > 0),
    close         NUMERIC     NOT NULL CHECK (close > 0),
    volume        BIGINT      NOT NULL CHECK (volume >= 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT futures_bars_1m_ohlc CHECK (
        low <= LEAST(open, close) AND high >= GREATEST(open, close)
    ),
    CONSTRAINT futures_bars_1m_key UNIQUE (source, dataset, symbol, start_at)
);

CREATE INDEX futures_bars_1m_by_instrument ON futures_bars_1m (instrument_id, start_at);

-- Un registro por tramo bajado: la corrida siguiente sigue desde el último y no vuelve a pagar.
CREATE TABLE futures_bar_loads (
    source    TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    dataset   TEXT        NOT NULL,
    symbol    TEXT        NOT NULL,
    from_at   TIMESTAMPTZ NOT NULL,
    to_at     TIMESTAMPTZ NOT NULL,
    bars      BIGINT      NOT NULL CHECK (bars >= 0),
    -- Costo estimado con metadata.get_cost antes de bajar (USD).
    cost_usd  NUMERIC     NOT NULL CHECK (cost_usd >= 0),
    loaded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT futures_bar_loads_span CHECK (to_at > from_at),
    CONSTRAINT futures_bar_loads_key UNIQUE (source, dataset, symbol, from_at)
);

CREATE FUNCTION futures_bars_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION '% es de solo agregar: no se modifica ni se borra', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER futures_bars_1m_no_update_or_delete
    BEFORE UPDATE OR DELETE ON futures_bars_1m
    FOR EACH ROW EXECUTE FUNCTION futures_bars_append_only();

CREATE TRIGGER futures_bar_loads_no_update_or_delete
    BEFORE UPDATE OR DELETE ON futures_bar_loads
    FOR EACH ROW EXECUTE FUNCTION futures_bars_append_only();
