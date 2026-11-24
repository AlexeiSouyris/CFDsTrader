-- #417: completar UNA sola vez los indicadores diarios que quedaron sin valor (NULL) por falta de
-- historia en nuestra base. Ejemplo: 775 días de US500.FUT (2021-11 → ~2024-12) guardaron NULL en
-- el tercio de la volatilidad porque la base no tenía 3 años antes de cada día; con la historia
-- desde 2011 (#399, #409) el valor se puede calcular. El mercado sí tenía esa historia en cada
-- día: el valor solo usa velas cerradas hasta ese día, no mira el futuro.
--
-- Aplica a las tablas de indicadores diarios con columnas que pueden quedar en NULL por
-- calentamiento o por falta de historia: `daily_indicators` (atr, realized_vol),
-- `daily_drawdowns` (drawdown) y `daily_vol_terciles` (tercile).
--
-- Regla del trigger nuevo (reemplaza al de solo agregar de cada tabla):
--   * UPDATE: solo si cada columna que cambia es una de las completables, pasa de NULL a un valor
--     no nulo, y el resto de la fila (clave, published_at, inserted_at) queda idéntico. Además,
--     la transacción tiene que declarar el motivo con
--     `SET LOCAL cfds.null_fill_reason = '#417'` (o `set_config(..., true)`).
--     Como un valor ya guardado no se cambia nunca, cada columna se completa una sola vez.
--   * Todo otro UPDATE (valor → otro valor, valor → NULL, cambiar la clave o la publicación,
--     un UPDATE que no cambia nada, o sin motivo) y todo DELETE siguen fallando (CLAUDE.md §5b).
--   * Cada columna completada queda en `daily_null_fills` (de solo agregar): tabla, clave de la
--     fila, columna, valor nuevo (el anterior es siempre NULL), cuándo y el motivo.

CREATE TABLE daily_null_fills (
    id          BIGSERIAL   PRIMARY KEY,
    table_name  TEXT        NOT NULL,
    -- La fila tal como estaba, sin las columnas completables ni inserted_at: la clave y la
    -- publicación (por ejemplo instrument_id, day, published_at, realized_vol_window, history_years).
    row_key     JSONB       NOT NULL,
    column_name TEXT        NOT NULL,
    -- Texto tal como lo guarda PostgreSQL (numeric o text). Antes era NULL.
    new_value   TEXT        NOT NULL,
    changed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    reason      TEXT        NOT NULL CHECK (reason <> '')
);

CREATE INDEX daily_null_fills_table_changed ON daily_null_fills (table_name, changed_at);

CREATE FUNCTION daily_null_fills_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'daily_null_fills es de solo agregar: no se modifica ni se borra';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER daily_null_fills_no_update_or_delete
    BEFORE UPDATE OR DELETE ON daily_null_fills
    FOR EACH ROW EXECUTE FUNCTION daily_null_fills_append_only();

-- Los argumentos del trigger (TG_ARGV) son las columnas completables de la tabla.
CREATE FUNCTION daily_fill_null_once() RETURNS trigger AS $$
DECLARE
    old_row JSONB;
    new_row JSONB;
    col     TEXT;
    filled  TEXT[] := '{}';
    reason  TEXT := current_setting('cfds.null_fill_reason', true);
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION '% es de solo agregar: no se borra', TG_TABLE_NAME;
    END IF;
    old_row := to_jsonb(OLD);
    new_row := to_jsonb(NEW);
    FOR col IN SELECT jsonb_object_keys(old_row) LOOP
        -- Se compara el texto: 1.0 y 1.00 son distintos (no se reescribe ni la escala).
        IF (old_row ->> col) IS DISTINCT FROM (new_row ->> col) THEN
            IF NOT (col = ANY (TG_ARGV)) THEN
                RAISE EXCEPTION '% es de solo agregar: la columna % no se modifica',
                    TG_TABLE_NAME, col;
            END IF;
            IF (old_row ->> col) IS NOT NULL OR (new_row ->> col) IS NULL THEN
                RAISE EXCEPTION '% es de solo agregar: % solo puede pasar de NULL a un valor, una vez (#417)',
                    TG_TABLE_NAME, col;
            END IF;
            filled := filled || col;
        END IF;
    END LOOP;
    IF cardinality(filled) = 0 THEN
        RAISE EXCEPTION '% es de solo agregar: un UPDATE solo puede completar valores NULL (#417)',
            TG_TABLE_NAME;
    END IF;
    IF reason IS NULL OR reason = '' THEN
        RAISE EXCEPTION '% : completar un NULL requiere SET LOCAL cfds.null_fill_reason (#417)',
            TG_TABLE_NAME;
    END IF;
    FOREACH col IN ARRAY filled LOOP
        INSERT INTO daily_null_fills (table_name, row_key, column_name, new_value, reason)
        VALUES (TG_TABLE_NAME, old_row - TG_ARGV - 'inserted_at', col, new_row ->> col, reason);
    END LOOP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER daily_indicators_no_update_or_delete ON daily_indicators;
DROP FUNCTION daily_indicators_append_only();
CREATE TRIGGER daily_indicators_fill_null_once
    BEFORE UPDATE OR DELETE ON daily_indicators
    FOR EACH ROW EXECUTE FUNCTION daily_fill_null_once('atr', 'realized_vol');

DROP TRIGGER daily_drawdowns_no_update_or_delete ON daily_drawdowns;
DROP FUNCTION daily_drawdowns_append_only();
CREATE TRIGGER daily_drawdowns_fill_null_once
    BEFORE UPDATE OR DELETE ON daily_drawdowns
    FOR EACH ROW EXECUTE FUNCTION daily_fill_null_once('drawdown');

DROP TRIGGER daily_vol_terciles_no_update_or_delete ON daily_vol_terciles;
DROP FUNCTION daily_vol_terciles_append_only();
CREATE TRIGGER daily_vol_terciles_fill_null_once
    BEFORE UPDATE OR DELETE ON daily_vol_terciles
    FOR EACH ROW EXECUTE FUNCTION daily_fill_null_once('tercile');
