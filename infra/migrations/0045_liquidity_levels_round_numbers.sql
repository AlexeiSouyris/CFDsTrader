-- Niveles de número redondo en el mapa de liquidez (#504). #497/#503 agregaron a
-- `contracts/liquidity_level.v1` y a `cfds_liquidity` los tipos `round_high` y `round_low` (el
-- múltiplo más cercano de un paso de `config/liquidity.toml [round_numbers]` por encima o por
-- debajo del cierre). El CHECK de `liquidity_levels.kind` (0020) no los aceptaba: se amplía.
--
-- Solo se agregan valores: toda fila ya guardada sigue siendo válida. La regla de la sesión
-- (`liquidity_levels_session`) no cambia: los niveles redondos van sin sesión.

ALTER TABLE liquidity_levels DROP CONSTRAINT liquidity_levels_kind_check;

ALTER TABLE liquidity_levels ADD CONSTRAINT liquidity_levels_kind_check
    CHECK (kind IN ('eqh', 'eql', 'pdh', 'pdl', 'pwh', 'pwl', 'session_high', 'session_low',
                    'round_high', 'round_low'));
