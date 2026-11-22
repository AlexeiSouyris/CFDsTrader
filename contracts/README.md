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
| `news_interpretation.v1` | por definir en #129 (#122 solo define el contrato) | Interpretación automática de una noticia (spec §9): resumen en español y, por instrumento, `sentiment` ∈ [−1, 1] y `relevance` ∈ [0, 1] como string decimal, con el intérprete registrado (nombre, versión, modelo y SHA-256 del prompt). Id propio e idempotente. |

- Una interpretación existe desde `interpreted_at`, que nunca es anterior a `published_at` de la noticia (CLAUDE.md §5b).
- El **ajuste de −2 a +2** del puntaje (spec §6 y §9) **no** viaja en el evento: lo calcula el motor en Rust a partir de `sentiment` y `relevance`, porque el puntaje vive en un solo lugar.
- La interpreta Python (`cfds_intel.news.interpret`, con un intérprete reemplazable). El stream de Redis y la tabla se definen junto con el primer intérprete real (#129). El ajuste del puntaje va en #130.

### Indicadores diarios (#148)

| Contrato | Tabla | Contenido |
|---|---|---|
| `daily_indicator.v1` | `daily_indicators` | Indicadores de una vela diaria cerrada: `instrument_id`, `day`, `published_at` (fin de la vela), `atr_period` y `atr`, `realized_vol_window` y `realized_vol` (los dos valores son `null` durante el calentamiento). |

- Los calcula y escribe **Rust** (`cfds-indicators` + `cargo run -p cfds-ingest --example daily_indicators`). Python los **lee** para los análogos (spec §13) y no los recalcula (CLAUDE.md §1).
- Solo días con sesión de la bolsa del instrumento.
- Tabla de **solo agregar**. Recalcular y obtener otro valor es un error (sería repintado), y no se guarda nada.
