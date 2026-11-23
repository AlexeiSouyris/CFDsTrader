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
| `backtest_run_curve.v1` | `backtest_run_curves` | La curva de una corrida (#299): capital al cierre de cada día con el drawdown de ese día (lo calcula el motor; su máximo es `max_drawdown`) y comprar y mantener de cada instrumento con los mismos costos. Una por corrida, de solo agregar. |
| `backtest_run_detail.v1` | (respuesta del gateway) | `GET /api/v1/backtest_runs/{run_id}` (#299): la corrida (`backtest_run.v1`), su curva (o `null`) y sus operaciones (`proposal.v1` con su `outcome.v1` simulado o `null`). Solo junta contratos existentes. |

- Las operaciones de una corrida son las filas de `proposals` con el mismo `run_id`: `proposal.v1` no cambia.
- `metrics.kill_switch_events` y `metrics.kill_switch_reset_business_days` (opcionales, #277): cada activación (`activated`) y cada reactivación del backtest (`reset`), con su hora y el capital, y cuántos días hábiles espera el backtest para reactivar (solo backtest; en vivo no existe).
- `metrics.kill_switch_at` (opcional, #273): cuándo se activó el kill switch por drawdown (`config/risk.toml` `max_drawdown_pct`). Desde ahí el recorrido no abre nada más; ausente si no se activó.
- `costs.spread_source` (opcional, #267): `measured` en las corridas con velas de IG (el filtro de spread §7.3 usa el spread medido) y `config` en las corridas con velas de futuros (`backtest_run --fuente futuros`): sin spread medido del CFD para esas fechas, los costos usan el spread típico de config y el filtro §7.3 no se aplica. Esas corridas guardan todo bajo el instrumento derivado (por ejemplo `US500.FUT`).
- Tabla de **solo agregar** (`infra/migrations/0012_backtest_runs.sql`, trigger). `from` y `to` se llaman `from_at` y `to_at`. `costs`, `news` y `metrics` son `JSONB` con la forma del contrato, y `news` es `NULL` sin noticias.
- **Idempotente por `run_id`:** guardar otra vez la misma corrida (todo igual salvo `started_at` y `finished_at`) no guarda nada y conserva la primera. Con otro contenido es un error: un `run_id` no se reusa.
- La escribe el motor al terminar `backtest_run` con `run_id` (`cfds_journal::pg::PgJournal::record_run`). La lee el aprendizaje en Python.

### Modelo de ML v1 (#158, `docs/spec/ml-v1.md`)

| Contrato | Tabla | Contenido |
|---|---|---|
| `ml_features.v1` | `ml_features` | Variables de una propuesta que no viajan en `proposal.v1` (spec ML §3): `proposal_id`, `as_of` (= `signal_at`), `atr14_m15` (string decimal, > 0), `realized_vol_tercile` (`low`/`mid`/`high`) y `session_band` (`open`/`mid`/`close`). |

- Una fila por propuesta, de **solo agregar** (`infra/migrations/0013_ml_features.sql`, trigger): una variable ya armada no cambia (sin repintado). La escribe el motor (`cfds_journal::ml_features`, desde `backtest_run` con `run_id`). Si a una propuesta le falta alguna variable (calentamiento), no hay fila: no se rellena.
- La arma el motor en Rust, solo con información al cierre de la vela de la señal. **Todavía no la escribe nadie:** el productor va en otro PR de #158.
- La lee Python (`cfds_intel.models.dataset`), junto con `proposals` y `outcomes` de una corrida de `backtest_runs`.

| Contrato | Tabla | Contenido |
|---|---|---|
| `ml_experiment.v1` | `ml_experiments` | Un entrenamiento y evaluación walk-forward (spec ML §7): id propio (SHA-256 de código, corrida, configuración y datos), versión del código (commit de git), corrida de `backtest_runs`, variables, hiperparámetros, cada ventana (con su motivo si no entrenó), métricas de B0, B1 y M1 (números: no son dinero) y el resultado: `suspended`, `m1_improves` o `b1_stays`. |

- Se registran **todos**, también los que no funcionan o quedan en suspenso. Sirve para el Sharpe deflactado y el PBO de la fase 5.
- Tabla de **solo agregar** (`infra/migrations/0015_ml_experiments.sql`): las columnas de arriba más `document`, el `JSONB` con el documento entero. El mismo experimento no se repite.
- Lo escribe Python (`python -m cfds_intel.models.experiment <run_id>`).

### Sentimiento (#25, #27)

| Contrato | Tabla | Contenido |
|---|---|---|
| `sentiment_observation.v1` | `sentiment_observations` | Dato **crudo** de una fuente de sentimiento, tal como se publicó: `source`, `series`, `instrument_id` (o `null` si es de todo el mercado), `observed_for`, `published_at` y `value` (string decimal). |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Una corrección de la fuente es otra fila con otra `published_at`.
- `published_at >= observed_for`: nada puede usar el dato antes de que estuviera disponible (CLAUDE.md §5b).
- Hoy la escribe Rust (`cfds-ig`, `source = 'ig_client'`, `series = 'long_pct'`). Python la **lee** para interpretarla en la fase 4b.

| Contrato | Tabla | Contenido |
|---|---|---|
| `sentiment.v1` | `sentiment_index` | Índice de sentimiento v1 de un instrumento (spec §9.1): `instrument_id`, `as_of`, `published_at` (la del dato más reciente que usa), `value` ∈ [−1, 1], `warmup_complete`, `min_components` y cada componente con su estado (`valid`, `warming_up`, `expired`, `missing` o `pending_data`), su último dato crudo, su percentil y su componente. |

- Lo calcula **Python** (`cfds_intel.sentiment.index`, `config/sentiment.toml`) con los valores crudos del COT (`sentiment_observations`) y las medidas de precio de Rust (`daily_indicators` y `daily_drawdowns`). Nadie lo recalcula.
- Solo se emite con al menos `min_components` componentes válidos. Un valor emitido no cambia nunca.
- Tabla de **solo agregar** (trigger), clave `(instrument_id, published_at)`. Los componentes van tal cual el contrato en `components` (JSONB). La escribe `python -m cfds_intel.sentiment.compute_index`: lo idéntico cuenta como "ya estaba", y otro valor para la misma clave es repintado y frena sin guardar.

### Tipos de referencia (#79)

| Contrato | Tabla | Contenido |
|---|---|---|
| `reference_rate.v1` | `reference_rates` | Valor diario de un tipo de referencia de la financiación nocturna, tal como lo publicó la fuente: `series` (mismo nombre que `[funding_benchmarks]`, hoy `ESTR`), `ref_date`, `published_at` y `rate_pct` (string decimal, puede ser negativo). |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Una corrección de la fuente es otra fila con otra `published_at`.
- `published_at` nunca es anterior al fin de `ref_date`: un valor solo se usa desde que se publicó (CLAUDE.md §5b).
- La escribe Python (`cfds_intel.rates`). La lee el motor para el costo de financiación de cada noche.

### Curva del Tesoro de EE. UU. (ADR 0012, #260)

| Contrato | Tabla | Contenido |
|---|---|---|
| `treasury_yield.v1` | `treasury_yields` | Rendimiento par diario de un plazo (`tenor`: `1M`, `1_5M`, `2M`, `3M`, `4M`, `6M`, `1Y`, `2Y`, `3Y`, `5Y`, `7Y`, `10Y`, `20Y` o `30Y`), con `ref_date`, `published_at` y `yield_pct` (string decimal). |

- Contrato aparte de `reference_rate.v1`, que es solo para la financiación nocturna (ADR 0012 §1.2).
- Tabla de **solo agregar** (trigger). Una corrección del Tesoro es otra fila, publicada cuando se leyó.
- `published_at`: en el histórico, el día hábil siguiente de la serie a las 09:30 ET (el Tesoro publica "usually … by 6:00 PM ET" y puede atrasarse); el último valor, la hora de lectura. Un `CHECK` impide que sea anterior a las 15:30 ET de `ref_date`, cuando se toman los precios.
- La escribe Python (`cfds_intel.treasury`). Es contexto: no suma al puntaje.

### VIX diario (ADR 0012 §2, #260)

| Contrato | Tabla | Contenido |
|---|---|---|
| `vix_daily.v1` | `vix_daily` | Un día del VIX de Cboe: `ref_date`, `published_at`, `open`, `high`, `low` y `close` (strings decimales). |

- Contrato propio: el VIX es el nivel de un índice con apertura, máximo, mínimo y cierre, no un rendimiento por plazo (no encaja en `treasury_yield.v1`).
- Se usa **por decisión del dueño**, contra los términos de Cboe (ADR 0012 §2). Los datos viven solo en la base del Mac: nunca en el repo, ni en tests ni en ejemplos.
- Tabla de **solo agregar** (trigger), con un `CHECK` de coherencia entre apertura, máximo, mínimo y cierre, y otro que impide `published_at` antes del cierre (16:15 ET de `ref_date`).
- `published_at`: igual que el Tesoro. En el histórico, el día hábil siguiente a las 09:30 ET; el último valor, la hora de lectura.

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

### Calendario económico (#91, #239, ADR 0006 y ADR 0010)

| Contrato | Tabla | Contenido |
|---|---|---|
| `economic_event.v1` | `economic_events` | Un evento del calendario: `source`, `event`, `country` (ISO 3166-1 alfa-2, `EU` = zona euro) o `instrument_id`, `impact` (`low`/`medium`/`high`), `scheduled_at` y `published_at`, en UTC. |

- Hoy la fuente es el feed JSON oficial de Forex Factory (`source = 'forex_factory'`, ADR 0010). Lo lee y guarda Python con `python -m cfds_intel.calendar`. La moneda del feed pasa a país con `config/calendar.toml`, y los feriados (`Holiday`) no se guardan.
- **`published_at` es la primera vez que se vio el evento** (anti look-ahead, ADR 0006 §3). No hay histórico: el calendario se acumula desde que empieza la ingesta.
- Tabla de **solo agregar** (`infra/migrations/0022_economic_events.sql`, trigger), con clave natural `(source, event, country, instrument_id, impact, scheduled_at)` y `NULLS NOT DISTINCT`. Leer otra vez el mismo evento no lo repite y conserva su primer `published_at`. Si la fuente cambia la hora o el impacto, es **otra fila** con su propio `published_at`; la anterior queda.
- Para leerla sin mirar el futuro: `published_at <= as_of` (`cfds_intel.calendar.store.events_known_at`). La lee el motor (`PgStore::economic_events` y `PgStore::economic_event_sightings`, #243) para el filtro §7.2 de `backtest_run` y `chart_export` (`cfds_market_data::NewsFilter`): un evento cuenta en una vela solo si `published_at ≤` su cierre, y una vela sin una lectura de la ingesta en los `calendar_stale_after_days` anteriores (`config/news_filter.toml`) se informa como "sin calendario".

### Histórico de futuros (#247, ADR 0011 en propuesta)

| Contrato | Tabla | Contenido |
|---|---|---|
| `futures_bar.v1` | `futures_bars_1m` | Una barra de 1 minuto de un futuro, tal como la publica la fuente: `source` (`databento`), `dataset`, `symbol` (contrato continuo, por ejemplo `ES.c.0`), `instrument_id` (nuestro CFD: US500, US100, US30, DE40), `contract_id` (el contrato real: si cambia, hubo vencimiento), `start` (inicio en UTC, justo en un minuto), OHLC como string decimal y `volume`. |

- La escribe Python (`python -m cfds_intel.databento --run`, solo con `CFDS_DATA_MACHINE=1`). La lee Rust con `cfds_core::FuturesBar` (`PgStore::futures_bars`), que acepta lo mismo que el contrato, y la agrupa y verifica `cfds_market_data::futures` (#248).
- Tabla de **solo agregar** (`infra/migrations/0023_futures_bars.sql`, trigger), con clave `(source, dataset, symbol, start_at)`. La misma barra otra vez no cambia nada, y otra con la misma clave es un conflicto: no se guarda nada del tramo. `futures_bar_loads` registra cada tramo bajado y su costo estimado, para no volver a pagarlo.
- Solo barras del **libro central**: en Eurex, OHLCV trae un registro aparte por publisher cuando en el minuto hubo operaciones fuera de libro (`XEUR.EOBI.XOFF`). Esas se descartan y se cuentan (`publisher` en `config/databento.toml`, #254).
- Precios **sin ajustar** por vencimientos.
- El motor arma con estas barras velas de 15m y 1h con la forma de `candle.v1` y las guarda en `futures_candles` (`infra/migrations/0024_futures_candles.sql`, #259), una tabla **aparte** de `candles`: así nada que lea las velas de IG puede mezclarlas con las del futuro. Es de solo agregar, con clave `(source, instrument_id, timeframe, start_at)` y el contrato de cada vela. Solo la lee Rust.
- **Diarias de los futuros** (#282): el motor arma con las velas de 15m de `futures_candles` diarias con la forma de `daily_candle.v1` y la misma ventana que las de IG (medianoche a medianoche en `config/candle_sources.toml` `daily_timezone`, igual a `config/ig.toml` `prices_timezone`), solo los días con sesión del CFD (`config/sessions.toml` y feriados) y, el día del vencimiento (calendario de #270), solo con las velas del contrato nuevo. Se guardan en `daily_candles` bajo el instrumento derivado (por ejemplo `US500.FUT`): nunca pisan las de IG, y `daily_indicators`, `daily_drawdowns`, `daily_vol_terciles`, `sentiment_index` y los análogos las usan con `--fuente futuros` sin cambiar ningún contrato. Limitación: el retorno diario del día del vencimiento incluye el salto de base entre contratos.
- **Liquidez con los futuros** (#259): `liquidity_levels` y `liquidity_stats` con `--fuente futuros` calculan lo mismo con estas velas, y lo guardan bajo un **instrumento derivado** (`<instrumento><id_suffix>`, por ejemplo `US500.FUT`, `config/candle_sources.toml`). Así las tablas de niveles, FVG, gaps, desequilibrio y agregados (y sus contratos) no cambian y nunca mezclan las dos fuentes. El gateway lo sirve con `?source=futures`, y `liquidity_view.v1` y `gateway_instruments.v1` traen el campo opcional `source` / `sources` (`id` `ig` o `futures` y su etiqueta) para que la UI muestre de dónde son los precios. No se mezcla con `candles` (IG): el spread y los costos siguen siendo los de IG.
- Los datos viven solo en la base local de la máquina de datos, **nunca en el repo**.

### Indicadores diarios (#148, #213, #218)

| Contrato | Tabla | Contenido |
|---|---|---|
| `daily_indicator.v1` | `daily_indicators` | Indicadores de una vela diaria cerrada: `instrument_id`, `day`, `published_at` (fin de la vela), `atr_period` y `atr`, `realized_vol_window` y `realized_vol` (los dos valores son `null` durante el calentamiento). |
| `daily_drawdown.v1` | `daily_drawdowns` | Caída desde el máximo de una vela diaria cerrada (#213, spec §9.1): `instrument_id`, `day`, `published_at` (fin de la vela), `drawdown_window` y `drawdown` (fracción ≤ 0, `null` durante el calentamiento). La usa el índice de sentimiento. |
| `daily_vol_tercile.v1` | `daily_vol_terciles` | Tercio de la volatilidad realizada de una vela diaria frente a su propia historia (spec §13, spec ML §3, #218): `instrument_id`, `day`, `published_at` (fin de la vela), `realized_vol_window`, `history_years` y `tercile` (`low`/`mid`/`high`, `null` sin volatilidad o sin años completos de historia). Lo leen el contexto de los análogos (Python) y las variables de ML (`ml_features`, Rust). |

- Los calcula y escribe **Rust** (`cfds-indicators` + `cargo run -p cfds-ingest --example daily_indicators`). Python los **lee** para los análogos (spec §13) y el índice de sentimiento, y no los recalcula (CLAUDE.md §1). El tercio de la volatilidad se calcula solo en Rust (`cfds_indicators::vol_terciles`).
- Solo días con sesión de la bolsa del instrumento.
- Tabla de **solo agregar**. Recalcular y obtener otro valor es un error (sería repintado), y no se guarda nada.

### Mapa de liquidez (#231, spec §5.4.1)

| Contrato | Dónde | Contenido |
|---|---|---|
| `liquidity_level.v1` | tablas `liquidity_levels` y `liquidity_level_events` | Un nivel del mapa de liquidez: `instrument_id`, `kind` (`eqh`, `eql`, `pdh`, `pdl`, `pwh`, `pwl`, `session_high`, `session_low`), `session` (`asia`, `london` o `ny` en los de sesión; `null` en los demás), `price` (string decimal), `exists_from` (UTC), `state` (`intact`, `swept` o `broken`) y `state_changed_at` (cierre de la vela que lo barrió o rompió; `null` si está intacto) y, opcional, `expired_at` (cierre de la vela número `max_age_bars` si llegó intacto hasta ahí: desde ese momento ya no está activo). |

- **Solo informativo** en v1: no suma al puntaje de §6.
- Lo calcula **Rust** (`cfds_liquidity::liquidity_levels`) con velas cerradas de 15m, los pivotes de `config/patterns.toml`, el calendario de `config/sessions.toml` y los parámetros de `config/liquidity.toml`. Nadie lo recalcula; la UI solo lo muestra.
- Un nivel no existe antes de `exists_from`. Su estado cambia a lo sumo una vez y nunca se reescribe (sin repintado). Rust exige además `state_changed_at > exists_from`.
- `expired_at` se agregó a v1 como campo **opcional** (cambio compatible): un documento sin él sigue siendo válido. Hace falta porque saber si un nivel intacto venció exige contar velas, y ese cálculo vive solo en Rust.
- **Tablas** (`infra/migrations/0020_liquidity_levels.sql`), las dos de **solo agregar** (trigger):
  - `liquidity_levels`: el nivel, con las columnas del contrato (sin el estado) y un `id`. Clave natural `(instrument_id, kind, session, price, exists_from)`, con `NULLS NOT DISTINCT`: el mismo nivel no se repite.
  - `liquidity_level_events`: a lo sumo **un evento por nivel** (`level_id` es la clave): `swept` o `broken` (`state` y `state_changed_at`) o `expired` (`expired_at`), con su hora `at`, siempre posterior a `exists_from`. Un nivel sin evento está intacto y activo. El estado nunca se reescribe: no hay `UPDATE`.
- Lo escribe el motor con `cargo run -p cfds-ingest --example liquidity_levels` (`PgStore::insert_liquidity_levels`): idempotente, y si un nivel ya guardado diera otro estado es repintado, error y no se guarda nada de ese instrumento.
- Se lee con `PgStore::liquidity_levels(instrumento, desde, as_of)`: los niveles **tal como se conocían en `as_of`** (un evento posterior no se ve), para no mirar el futuro. Python y la UI leen las mismas tablas con el mismo criterio (`at <= as_of`).

### Estadísticas de FVG y gaps de apertura (#231, spec §5.4.1)

| Contrato | Contenido |
|---|---|
| `fvg_event.v1` | Un FVG (definición de §5.4) en 15m, 1h o 1d: creación, dirección, zona, ATR y tamaño en ATR; primera vela que toca, que llega a la mitad, que lo llena y en la que vence, cada una con las velas desde la creación; reacción a `reaction_bars` velas del primer toque (en ATR, con signo a favor del FVG); estado (`open`, `filled` o `expired`) y `as_of`. |
| `opening_gap.v1` | Un gap de apertura diario: sesión, dirección, cierre anterior, apertura, ATR diario de la sesión anterior y tamaño en ATR; sesión en la que se llenó (y si fue en la misma), sesión en la que venció, estado y `as_of`. |
| `liquidity_stats.v1` | Agregados de un grupo (instrumento, `fvg` u `opening_gap`, timeframe, dirección y tercil de tamaño, o `all`): tasas con IC 95 % de Wilson y su `n`, medianas, cuartiles y rango intercuartílico. Con menos de `min_cases` casos, `insufficient = true` y los valores en `null` ("datos insuficientes"). |

- **Solo informativos** en v1: no suman al puntaje de §6 hasta validarse en la fase 5.
- Los calcula **solo Rust** (`cfds_liquidity::stats`, `config/liquidity.toml` `[fvg_stats]`, `[gaps]` y `[stats]`) con velas cerradas. La UI y Python los leen y no los recalculan.
- Sin repintado: un evento ya registrado (toque, mitad, llenado, vencimiento o una reacción no nula) no cambia con velas nuevas. Un caso abierto cuenta para "tocado", pero no para "llenado" ni "vencido".
- Los gaps son un contrato aparte de los FVG porque se miden en sesiones y se llenan al **tocar** el cierre anterior, no al cerrar más allá del borde.
- **Guardado** (`infra/migrations/0021_liquidity_stats.sql`, todas de solo agregar con trigger): `fvg_zones` + `fvg_zone_events` (cada toque, mitad, llenado, vencimiento y reacción, una vez y con su hora), `opening_gaps` + `opening_gap_events` y `liquidity_stats` (una fila por grupo y corrida, `computed_at`, con el documento entero en `document` JSONB; una corrida igual a la última no se guarda otra vez). Lo escribe el motor con `cargo run -p cfds-ingest --example liquidity_stats` y se lee tal como se conocía en `as_of` (`cfds_storage::PgStore`). Otro valor para algo ya guardado es repintado: error y no se guarda nada del lote.
- Campo opcional `at` (agregado para el guardado, compatible): la hora desde la que se conoce cada reacción (`fvg_event.v1`) y cada llenado o vencimiento de un gap (`opening_gap.v1`). El motor siempre lo llena.

### Desequilibrio aproximado desde el precio (#231, spec §5.4.1)

| Contrato | Contenido |
|---|---|
| `imbalance.v1` | Al cierre de cada vela (15m, 1h o 1d): si es de desplazamiento y en qué dirección, las sumas de cuerpos de desplazamiento alcistas y bajistas y de rangos de la ventana de `window_bars` velas, el índice de −1 a +1 (0 si los rangos suman 0) y el rótulo fijo `"aproximación desde el precio"`. |

- **No es flujo de órdenes** (IG no da volumen ni libro de órdenes): el rótulo viaja en el contrato para que la UI lo muestre siempre.
- Contrato **aparte** de `liquidity_stats.v1`: es una serie, un valor por vela cerrada para el gráfico, y no un agregado de casos.
- Lo calcula **solo Rust** (`cfds_liquidity::imbalance`, `[imbalance]` de `config/liquidity.toml`). Un valor emitido no cambia. Solo informativo.
- Se guarda en `imbalances` (migración 0021, solo agregar, clave instrumento + timeframe + vela + ventana) con el mismo comando `liquidity_stats`. Todavía no se publica en Redis.

### Vista de liquidez del gateway (#231)

| Contrato | Contenido |
|---|---|
| `liquidity_view.v1` | Respuesta de `GET /api/v1/liquidity?instrument=<id>[&timeframe=15m\|1h\|1d]` del gateway: `instrument_id`, `timeframe` (opcional, agregado sin romper v1; ausente = 15m), `generated_at`, `as_of` (cierre de la última vela cerrada guardada de ese timeframe, o `null` si no hay velas) y las listas `candles` (`candle.v1` en 15m y 1h, `daily_candle.v1` en 1d), `levels` (`liquidity_level.v1`), `fvgs` (`fvg_event.v1`), `imbalance` (`imbalance.v1`) y `stats` (`liquidity_stats.v1`), cada elemento con su contrato sin cambios (`$ref`). |

| `gateway_instruments.v1` | Respuesta de `GET /api/v1/instruments`: los instrumentos del catálogo (`instrument.v1`), los timeframes que acepta la vista y lo que abre primero (`[view]` de `config/gateway.toml`). |

- Es un **sobre**: no agrega cálculos. El gateway (`engine/crates/gateway`) lo arma con las lecturas de `cfds_storage::PgStore`, todas tal como se conocían en `as_of`, y la ventana de cada timeframe de `config/gateway.toml`. Los FVG, el desequilibrio y los agregados son los del timeframe pedido (en 1d, también los agregados de los gaps de apertura); los niveles son los del mapa de liquidez (15m), tal como se conocían en `as_of`.
- Sin velas guardadas, `as_of` es `null` y las listas van vacías: la UI muestra "sin datos todavía".
- Los errores HTTP (400, 404, 405, 500) van como texto plano, sin datos.
- El gateway abre la sesión de PostgreSQL en **solo lectura** (`default_transaction_read_only`).

### Broker simulado (fase 7, #278)

| Contrato | Contenido |
|---|---|
| `order.v1` | Una orden de mercado: `order_id` propio e idempotente, `mode` (solo `paper`), instrumento, dirección, **stop obligatorio**, `tp1`, `proposal_id` y `created_at`. **Sin tamaño:** lo calcula solo el riesgo (spec §8). |
| `order_report.v1` | Qué pasó con la orden: `rejected` (con el motivo) o `filled` (posición, tamaño, precio con spread y slippage, comisión), y cuándo. |
| `ticket_verdict.v1` | La respuesta del riesgo a una orden **sin enviarla**: acepta o rechaza, con tamaño, riesgo, % de riesgo, apalancamiento y margen. La UI la muestra. |
| `position.v1` | Una posición abierta valuada al cierre de `as_of`: entrada, stop, `tp1`, último precio, P/L no realizado (en moneda y en R) y margen. |
| `paper_account.v1` | La cuenta simulada al cierre de `as_of`: capital, P/L, margen, apalancamiento, pérdida diaria usada, posiciones y kill switch. |

- **Solo modo paper:** ninguno de estos mensajes puede describir una orden real (CLAUDE.md §5, fase 11).
- **Una sola lista de motivos de rechazo**, compartida por `order_report.v1` y `ticket_verdict.v1`: los del módulo de riesgo más `no_price`, `unknown_instrument` y `stop_wrong_side`. Un test comprueba que las dos listas sean iguales.
- **Sin ejecuciones parciales** en la simulación: la spec §12 las prevé solo para el registro real.
- Los escribe el motor (`cfds-broker`) y los lee la UI (terminal, #275), que hoy los tiene como mocks con `TODO(#275)`.
- **Guardado** (`infra/migrations/0028_paper_orders.sql`, solo agregar con trigger): `paper_orders` (cada `order.v1`, una vez por `order_id`, en orden de llegada, solo `mode = paper`) y `paper_order_reports` (su `order_report.v1`, uno por orden). Otro contenido para un `order_id` ya guardado es repintado: error. Con este registro el broker se reconstruye al arrancar y se reconcilia cada `[reconcile] every_minutes`.

### Análogos históricos (#148, #294)

| Contrato | Tabla | Contenido |
|---|---|---|
| `analog_report.v1` | `analog_reports` | El informe de análogos de la última ventana de un índice: fuente (`ig` o `futures`), día y cierre de la ventana, contexto (volatilidad y tasas), vecinos (índice, día y distancia) y, para "todos" y "mismo contexto", cuántos son, si son insuficientes, los percentiles del retorno por horizonte (fracciones), el % positivo y los toques de +R/−R. |

- **Solo informativo:** es la distribución de lo que pasó después de situaciones parecidas, **nunca un pronóstico ni una recomendación**. Con menos de `min_neighbors` vecinos, `insufficient = true` y se muestra "datos insuficientes".
- Lo calcula y lo escribe **solo Python** (`python -m cfds_intel.analogs --guardar`, migración 0029, solo agregar). Un informe igual al último del mismo día no se repite. Rust (gateway) y la UI lo leen sin recalcular.

### Fondo del grupo (#179)

| Contrato | Tabla | Contenido |
|---|---|---|
| `fund_movement.v1` | `fund_movements` | Un aporte o retiro ya liquidado (spec §14): `id` (`fund:<fecha>:<orden>`), `settled_at`, `member_id`, `kind` (`contribution` o `withdrawal`), `amount` (en céntimos), `units`, `nav` y `currency`, con los decimales como string. |
| `fund_summary.v1` | (respuesta del gateway) | `GET /api/v1/fund` (#312): `currency`, `basis` (`last_settlement`: valuado al cerrar la última liquidación), `valued_at` (o `null`), `nav`, `total_value`, `total_units` y por miembro `units`, `value`, `net_contributions` y `pnl` (estos dos con signo). Lo calcula `cfds_fund::summary` desde el historial; la suma de `value` es `total_value`. |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Guardar el mismo movimiento otra vez no duplica nada, y otro contenido con el mismo id es un error.
- El estado de cada miembro (unidades, aportes y retiros) **no se guarda aparte**: se reconstruye sumando el historial (`cfds_fund::Fund::replay`).
- Lo escribe el motor (`cfds-fund`, `PgFund`). Lo leen la UI (vista Fondo) y el aprendizaje.
