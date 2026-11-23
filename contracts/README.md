# Contratos entre capas

Todo mensaje o tabla que cruza entre **Rust (engine)**, **Python (intelligence)** y **TypeScript (ui)** se define acá como **JSON Schema**, antes de implementarlo.

- Un archivo por mensaje: `<nombre>.v<versión>.schema.json` (por ejemplo `candle.v1.schema.json`).
- Cambio compatible (agregar un campo opcional): se edita la misma versión.
- Cambio incompatible: se crea `v2` y se mantiene `v1` hasta migrar a todos los consumidores (requiere ADR).
- Precios y montos van como **string decimal** (por ejemplo `"26747.80"`), nunca como número flotante.
- Timestamps en **ISO 8601 UTC**.

## Dónde viajan los mensajes

### Redis Streams (tiempo real)

| Stream | Contenido | ID de la entrada |
|---|---|---|
| `candles.v1.<instrumento>.<timeframe>` (ej. `candles.v1.US100.15m`) | Campo `data` = JSON de `candle.v1`. **Solo velas cerradas.** | `<inicio de la vela en ms>-0`: ordena y evita duplicados |

Lo publica `cfds-storage::CandlePublisher` (Rust). Los consumidores (Python, UI) leen con `XREAD`/`XRANGE`.

### PostgreSQL (histórico)

El esquema es el de `infra/migrations/` (fuente de verdad), aplicado con `cfds-storage::PgStore::migrate`.
La tabla `candles` tiene las mismas columnas que `candle.v1` (`start` se llama `start_at`; precios `NUMERIC` exactos).
Python puede **leerla**, pero solo el motor en Rust **escribe** velas.

### Diario (spec §15b)

| Contrato | Contenido |
|---|---|
| `proposal.v1` | Una propuesta: instrumento, dirección, hora, entrada de referencia, stop, TP1, tamaño, apalancamiento, puntaje con su desglose por señal y gatillos. Id propio e idempotente. |
| `outcome.v1` | Su resultado, `simulated` o `real`: entrada, cada salida (hora, precio, fracción y motivo), costos, P/L bruto y neto (con signo), si tocó TP1 o el stop. |

Los escribe el motor (`cfds-journal`, tablas `proposals` y `outcomes` de `infra/migrations/0002_journal.sql`); los lee el aprendizaje en Python. Las columnas tienen los mismos nombres que los contratos; `breakdown`, `triggers` y `exits` son `JSONB` con la forma del contrato. Montos en la moneda del instrumento.

| Contrato | Tabla | Contenido |
|---|---|---|
| `backtest_run.v1` | `backtest_runs` | Una corrida de backtest (#200): `run_id`, instrumentos, timeframe de decisión, período `[from, to)`, `config_sha256` (SHA-256 de los archivos de `config/` que leyó, cada uno con su nombre), capital inicial, costos usados (slippage y costos de cada instrumento), noticias (intérprete, modelo, `published_from` del corte del ADR 0008 §5 y cantidad de interpretaciones, o `null`), resumen de métricas (decimales como texto, `null` si no están definidas) y `started_at`/`finished_at` en UTC. |

- Las operaciones de una corrida son las filas de `proposals` con el mismo `run_id`: `proposal.v1` no cambia.
- Tabla de **solo agregar** (`infra/migrations/0012_backtest_runs.sql`, trigger). `from` y `to` se llaman `from_at` y `to_at`. `costs`, `news` y `metrics` son `JSONB` con la forma del contrato, y `news` es `NULL` sin noticias.
- **Idempotente por `run_id`:** guardar otra vez la misma corrida (todo igual salvo `started_at` y `finished_at`) no guarda nada y conserva la primera. Con otro contenido es un error: un `run_id` no se reusa.
- La escribe el motor al terminar `backtest_run` con `run_id` (`cfds_journal::pg::PgJournal::record_run`). La lee el aprendizaje en Python.

### Modelo de ML v1 (#158, `docs/spec/ml-v1.md`)

| Contrato | Tabla | Contenido |
|---|---|---|
| `ml_features.v1` | `ml_features` | Variables de una propuesta que no viajan en `proposal.v1` (spec ML §3): `proposal_id`, `as_of` (= `signal_at`), `atr14_m15` (string decimal, > 0), `realized_vol_tercile` (`low`/`mid`/`high`) y `session_band` (`open`/`mid`/`close`). |

- Una fila por propuesta, de **solo agregar** (`infra/migrations/0013_ml_features.sql`, trigger): una variable ya armada no cambia (sin repintado). Si a una propuesta le falta alguna variable (calentamiento), no hay fila: no se rellena.
- La arma el motor en Rust, solo con información al cierre de la vela de la señal. **Todavía no la escribe nadie:** el productor va en otro PR de #158.
- La lee Python (`cfds_intel.models.dataset`), junto con `proposals` y `outcomes` de una corrida de `backtest_runs`.

| Contrato | Tabla | Contenido |
|---|---|---|
| `ml_experiment.v1` | `ml_experiments` | Un entrenamiento y evaluación walk-forward (spec ML §7): id propio (SHA-256 de código, corrida, configuración y datos), versión del código (commit de git), corrida de `backtest_runs`, variables, hiperparámetros, cada ventana (con su motivo si no entrenó), métricas de B0, B1 y M1 (números: no son dinero) y el resultado: `suspended`, `m1_improves` o `b1_stays`. |

- Se registran **todos**, también los que no funcionan o quedan en suspenso. Sirve para el Sharpe deflactado y el PBO de la fase 5.
- Tabla de **solo agregar** (`infra/migrations/0014_ml_experiments.sql`): las columnas de arriba más `document`, el `JSONB` con el documento entero. El mismo experimento no se repite.
- Lo escribe Python (`python -m cfds_intel.models.experiment <run_id>`).

### Sentimiento (#25, #27)

| Contrato | Tabla | Contenido |
|---|---|---|
| `sentiment_observation.v1` | `sentiment_observations` | Dato **crudo** de una fuente de sentimiento, tal como se publicó: `source`, `series`, `instrument_id` (o `null` si es de todo el mercado), `observed_for`, `published_at` y `value` (string decimal). |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Una corrección de la fuente es otra fila con otra `published_at`.
- `published_at >= observed_for`: nada puede usar el dato antes de que estuviera disponible (CLAUDE.md §5b).
- Hoy la escribe Rust (`cfds-ig`, `source = 'ig_client'`, `series = 'long_pct'`). Python la **lee** para interpretarla en la fase 4b.

### Tipos de referencia (#79)

| Contrato | Tabla | Contenido |
|---|---|---|
| `reference_rate.v1` | `reference_rates` | Valor diario de un tipo de referencia de la financiación nocturna, tal como lo publicó la fuente: `series` (mismo nombre que `[funding_benchmarks]`, hoy `ESTR`), `ref_date`, `published_at` y `rate_pct` (string decimal, puede ser negativo). |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Una corrección de la fuente es otra fila con otra `published_at`.
- `published_at` nunca es anterior al fin de `ref_date`: un valor solo se usa desde que se publicó (CLAUDE.md §5b).
- La escribe Python (`cfds_intel.rates`). La lee el motor para el costo de financiación de cada noche.

### Noticias (#109, #111, #122)

| Contrato | Dónde | Contenido |
|---|---|---|
| `news_item.v1` | tabla `news_items` | Una noticia de una fuente aprobada (ADR 0006): fuente, feed, id, título, enlace, `published_at` y `retrieved_at`. Solo agregar. La escribe Python (`cfds_intel.news`). |
| `news_interpretation.v1` | tabla `news_interpretations` y stream `news_interpretations.v1` (#186) | Interpretación automática de una noticia (spec §9): resumen en español y, por instrumento, `sentiment` ∈ [−1, 1] y `relevance` ∈ [0, 1] como string decimal, con el intérprete registrado (nombre, versión, modelo y SHA-256 del prompt). Id propio e idempotente. |

- Una interpretación existe desde `interpreted_at`, que nunca es anterior a `published_at` de la noticia (CLAUDE.md §5b).
- El **ajuste de −2 a +2** del puntaje (spec §6 y §9) **no** viaja en el evento: lo calcula el motor en Rust a partir de `sentiment` y `relevance`, porque el puntaje vive en un solo lugar.
- La interpreta Python (`cfds_intel.news.interpret`, con un intérprete reemplazable; hoy Ollama local, ADR 0008) y la guarda y publica con `python -m cfds_intel.news.interpret_new` (#186), que toma las noticias de `news_items` sin interpretación del intérprete fijado.
- **Tabla `news_interpretations`** (`infra/migrations/0011_news_interpretations.sql`): de **solo agregar** (trigger), clave `interpretation_id`. Mismas columnas que el contrato, con `interpreter` aplanado en `interpreter_name`, `interpreter_version`, `interpreter_model` e `interpreter_prompt_sha256`, e `impacts` como `JSONB` con la forma del contrato. `CHECK interpreted_at >= published_at` y clave foránea a `news_items`. Una noticia tiene una sola interpretación por intérprete (nombre y versión): guardarla otra vez no guarda nada y se conserva la primera; la misma versión con otro modelo o prompt es un error.
- **Stream de Redis `news_interpretations.v1`** (uno solo para todas las fuentes): una entrada por interpretación **nueva**, con ID automático de Redis (`*`) y dos campos: `data` = JSON de `news_interpretation.v1` e `interpretation_id`. Se publica dentro de la transacción del guardado, antes del commit: si Redis falla no se guarda nada. Entrega **al menos una vez**: el consumidor deduplica por `interpretation_id`, y la tabla es la fuente de verdad.
- **Lo lee el motor** con `cfds_storage::NewsInterpretationConsumer` (#197): consumer group de Redis (`XREADGROUP` + `XACK`), así una entrada entregada y no confirmada se vuelve a entregar después de un reinicio. Deduplica por `interpretation_id` con el set `<stream>.seen.<grupo>`, que se actualiza junto con el `XACK` en una transacción. Una entrada que no cumple el contrato se informa, se confirma y no vuelve. Todavía no está conectado a la estrategia en vivo.
- La lee Rust con `cfds_core::NewsInterpretation`, que acepta lo mismo que el contrato y además exige `interpreted_at >= published_at`. El ajuste del puntaje (`cfds_strategy::news_adjustment`, #130) usa solo las interpretaciones con `interpreted_at` ≤ el cierre de la vela.

### Indicadores diarios (#148)

| Contrato | Tabla | Contenido |
|---|---|---|
| `daily_indicator.v1` | `daily_indicators` | Indicadores de una vela diaria cerrada: `instrument_id`, `day`, `published_at` (fin de la vela), `atr_period` y `atr`, `realized_vol_window` y `realized_vol` (los dos valores son `null` durante el calentamiento). |

- Los calcula y escribe **Rust** (`cfds-indicators` + `cargo run -p cfds-ingest --example daily_indicators`). Python los **lee** para los análogos (spec §13) y no los recalcula (CLAUDE.md §1).
- Solo días con sesión de la bolsa del instrumento.
- Tabla de **solo agregar**. Recalcular y obtener otro valor es un error (sería repintado), y no se guarda nada.

### Fondo del grupo (#179)

| Contrato | Tabla | Contenido |
|---|---|---|
| `fund_movement.v1` | `fund_movements` | Un aporte o retiro ya liquidado (spec §14): `id` (`fund:<fecha>:<orden>`), `settled_at`, `member_id`, `kind` (`contribution` o `withdrawal`), `amount` (en céntimos), `units`, `nav` y `currency`, con los decimales como string. |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Guardar el mismo movimiento otra vez no duplica nada, y otro contenido con el mismo id es un error.
- El estado de cada miembro (unidades, aportes y retiros) **no se guarda aparte**: se reconstruye sumando el historial (`cfds_fund::Fund::replay`).
- Lo escribe el motor (`cfds-fund`, `PgFund`). Lo leen la UI (vista Fondo) y el aprendizaje.
