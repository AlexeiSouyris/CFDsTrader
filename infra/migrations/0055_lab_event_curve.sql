-- Laboratorio en vivo (#854): la curva de capital de cada variante apenas termina, para dibujarla
-- mientras el trabajo sigue corriendo. Contrato: lab_job_event.v1 (campo opcional `curve`). La
-- escribe el lab_worker (cfds_journal::lab) en el paso "corriendo variantes" de cada variante; el
-- gateway la pasa tal cual por el SSE.
--
-- Solo agrega: una columna nueva (NULL en todas las filas que ya existen). Las filas guardadas
-- siguen valiendo tal cual y no se edita ninguna (el trigger de solo agregar de 0053 sigue igual).
-- La curva es la misma que la del informe (capital compuesto desde 1, mes a mes), reducida a pocos
-- puntos: es solo para mostrar, no vota y no se usa para ningún cálculo.

ALTER TABLE lab_job_events
    ADD COLUMN curve JSONB CHECK (
        curve IS NULL
        OR (jsonb_typeof(curve) = 'object' AND state = 'running' AND stage = 'running_variants')
    );
