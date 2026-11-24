-- #418 (ADR 0013): materias primas en el catálogo, empezando por el oro (XAUUSD).
-- `instrument.v1` agrega "commodity" a `asset_class` sin romper v1; la base acepta el mismo
-- valor. El CHECK de la 0001 no tenía nombre propio: PostgreSQL lo llamó
-- `instruments_asset_class_check`.

ALTER TABLE instruments DROP CONSTRAINT instruments_asset_class_check;

ALTER TABLE instruments
    ADD CONSTRAINT instruments_asset_class_check
        CHECK (asset_class IN ('index', 'share', 'commodity'));
