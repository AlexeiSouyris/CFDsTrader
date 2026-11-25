-- Velas cerradas de IBKR como fuente de datos (#536). Columnas = contracts/ibkr_bar.v1. Las escribe
-- Rust (`cargo run -p cfds-ibkr --example ibkr_load`), solo en la máquina con IB Gateway.
--
-- Tabla APARTE de `candles` (velas del CFD de IG, sin fuente), de `futures_bars_1m` y
-- `futures_candles` (Databento: símbolo continuo `RAÍZ.c.0`, id numérico del contrato, solo futuros,
-- volumen obligatorio). IBKR también da acciones y divisas (sin volumen en MIDPOINT) y nombra cada
-- contrato (MNQZ6).
--
-- Clave con el contrato: el mismo instante puede tener velas de dos vencimientos y ninguna vela
-- los mezcla. El futuro continuo de IBKR NO entra (está ajustado por vencimiento: repinta) ni las
-- diarias (fechadas por sesión, sin hora).
--
-- De solo agregar (trigger): una vela guardada no cambia nunca (sin repintado).

CREATE TABLE ibkr_bars (
    source        TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    instrument_id TEXT        NOT NULL CHECK (instrument_id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    kind          TEXT        NOT NULL CHECK (kind IN ('future', 'stock', 'forex')),
    contract      TEXT        NOT NULL CHECK (contract ~ '^[A-Z0-9][A-Z0-9. ]{0,31}$'),
    bar_size      TEXT        NOT NULL CHECK (bar_size IN ('1m', '15m', '1h')),
    what_to_show  TEXT        NOT NULL CHECK (what_to_show IN ('TRADES', 'MIDPOINT')),
    -- Inicio de la vela, en UTC, alineado a su tamaño.
    start_at      TIMESTAMPTZ NOT NULL,
    open          NUMERIC     NOT NULL CHECK (open > 0),
    high          NUMERIC     NOT NULL CHECK (high > 0),
    low           NUMERIC     NOT NULL CHECK (low > 0),
    close         NUMERIC     NOT NULL CHECK (close > 0),
    volume        NUMERIC              CHECK (volume >= 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ibkr_bars_aligned CHECK (
        (bar_size = '1m' AND extract(epoch FROM start_at)::bigint % 60 = 0)
        OR (bar_size = '15m' AND extract(epoch FROM start_at)::bigint % 900 = 0)
        OR (bar_size = '1h' AND extract(epoch FROM start_at)::bigint % 3600 = 0)
    ),
    CONSTRAINT ibkr_bars_ohlc CHECK (
        low <= LEAST(open, close) AND high >= GREATEST(open, close)
    ),
    CONSTRAINT ibkr_bars_key
        UNIQUE (source, instrument_id, contract, bar_size, what_to_show, start_at)
);

CREATE FUNCTION ibkr_bars_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'ibkr_bars es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ibkr_bars_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ibkr_bars
    FOR EACH ROW EXECUTE FUNCTION ibkr_bars_append_only();
