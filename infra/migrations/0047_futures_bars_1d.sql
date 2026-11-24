-- Velas diarias de futuros tal como las publica Databento (esquema `ohlcv-1d`, #522).
-- Columnas = contracts/futures_bar_1d.v1. Las escribe Python
-- (`python -m cfds_intel.databento --daily --run`, solo en la máquina de datos).
--
-- OJO: la vela diaria de Databento va de medianoche a medianoche UTC (`start_at` a las 00:00 UTC),
-- NO de la hora de liquidación (settlement) de la bolsa. No sirve para un estudio que pida el
-- cierre de liquidación (borrador del programa 3, #513) sin decirlo; tampoco es la diaria de IG
-- (medianoche de Madrid) ni la del motor (`futures_daily`, #282, en `daily_candles`).
--
-- Tabla APARTE de `futures_bars_1m` (otra granularidad, otra regla de alineación) y de
-- `daily_candles` (velas del CFD o derivadas por el motor, con su ventana propia). Hoy son datos
-- de investigación: los `instrument_id` de estos futuros (la raíz, por ejemplo `NKD`) no están en
-- `config/instruments.toml` y nadie los opera.
--
-- De solo agregar (trigger): una vela guardada no cambia nunca.

CREATE TABLE futures_bars_1d (
    source        TEXT        NOT NULL CHECK (source ~ '^[a-z][a-z0-9_]{0,31}$'),
    dataset       TEXT        NOT NULL CHECK (dataset ~ '^[A-Z0-9]+\.[A-Z0-9]+$'),
    -- Contrato continuo pedido (por ejemplo NKD.v.0) y el contrato real de la vela.
    symbol        TEXT        NOT NULL CHECK (symbol ~ '^[A-Z0-9]{1,10}\.[cnv]\.[0-9]{1,2}$'),
    contract_id   BIGINT      NOT NULL CHECK (contract_id > 0),
    instrument_id TEXT        NOT NULL CHECK (instrument_id ~ '^[A-Z0-9][A-Z0-9._-]{0,31}$'),
    -- Inicio de la vela: medianoche UTC del día UTC que cubre.
    start_at      TIMESTAMPTZ NOT NULL CHECK (extract(epoch FROM start_at)::bigint % 86400 = 0),
    open          NUMERIC     NOT NULL CHECK (open > 0),
    high          NUMERIC     NOT NULL CHECK (high > 0),
    low           NUMERIC     NOT NULL CHECK (low > 0),
    close         NUMERIC     NOT NULL CHECK (close > 0),
    volume        BIGINT      NOT NULL CHECK (volume >= 0),
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT futures_bars_1d_ohlc CHECK (
        low <= LEAST(open, close) AND high >= GREATEST(open, close)
    ),
    CONSTRAINT futures_bars_1d_key UNIQUE (source, dataset, symbol, start_at)
);

CREATE INDEX futures_bars_1d_by_instrument ON futures_bars_1d (instrument_id, start_at);

CREATE TRIGGER futures_bars_1d_no_update_or_delete
    BEFORE UPDATE OR DELETE ON futures_bars_1d
    FOR EACH ROW EXECUTE FUNCTION futures_bars_append_only();

-- El registro de tramos bajados sirve para los dos esquemas: se agrega cuál es. Las filas que ya
-- estaban son todas de 1 minuto (el DEFAULT se guarda en el catálogo: no reescribe filas ni
-- dispara el trigger de solo agregar). Así un mismo símbolo puede tener tramos de 1m y de 1d sin
-- que uno haga creer que el otro ya está bajado.
ALTER TABLE futures_bar_loads
    ADD COLUMN schema TEXT NOT NULL DEFAULT 'ohlcv-1m'
        CHECK (schema IN ('ohlcv-1m', 'ohlcv-1d'));
ALTER TABLE futures_bar_loads DROP CONSTRAINT futures_bar_loads_key;
ALTER TABLE futures_bar_loads
    ADD CONSTRAINT futures_bar_loads_key UNIQUE (source, dataset, schema, symbol, from_at);
