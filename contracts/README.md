# Contratos entre capas

Todo mensaje o tabla que cruza entre **Rust (engine)**, **Python (intelligence)** y **TypeScript (ui)** se define acá como **JSON Schema**, antes de implementarlo.

- Un archivo por mensaje: `<nombre>.v<versión>.schema.json` (por ejemplo `candle.v1.schema.json`).
- Cambio compatible (agregar un campo opcional): se edita la misma versión.
- Cambio incompatible: se crea `v2` y se mantiene `v1` hasta migrar a todos los consumidores (requiere ADR).
- Precios y montos van como **string decimal** (por ejemplo `"26747.80"`), nunca como número flotante.
- Timestamps en **ISO 8601 UTC**.

## Índice de contratos

Lista de todos los contratos, generada desde `contracts/*.schema.json` (`title` y `description`). Al agregar o cambiar un contrato, regenerala con `python3 tools/docs_index.py` (el CI falla si no está al día). Dónde viaja cada uno y sus reglas se explican en las secciones de abajo.

<!-- indice-de-contratos:inicio (generado por `python3 tools/docs_index.py`; no editar a mano) -->
| Contrato | Título | Qué es (primera oración de `description`) |
|---|---|---|
| [`analog_report.v1`](analog_report.v1.schema.json) | Analog report v1 | Análogos históricos de la última ventana de un índice (spec §13, #148, #294) |
| [`backtest_run.v1`](backtest_run.v1.schema.json) | Backtest run v1 | Una corrida de backtest del diario (spec §15b, #200) |
| [`backtest_run_curve.v1`](backtest_run_curve.v1.schema.json) | Backtest run curve v1 | La curva de una corrida de backtest (#299) |
| [`backtest_run_detail.v1`](backtest_run_detail.v1.schema.json) | Backtest run detail v1 | El detalle de una corrida de backtest que devuelve el gateway en GET /api/v1/backtest_runs/{run_id} (#299) |
| [`candle.v1`](candle.v1.schema.json) | Candle v1 | Una vela OHLC |
| [`candle_spread.v1`](candle_spread.v1.schema.json) | Candle spread v1 | Spread (ask − bid) al cierre de una vela, tal como lo dio la fuente (#95) |
| [`change_proposal.v1`](change_proposal.v1.schema.json) | Change proposal v1 | Una propuesta de cambio de un informe de aprendizaje (learning_report.v1, fase 8b, #327), guardada como nació |
| [`change_vote.v1`](change_vote.v1.schema.json) | Change vote v1 | El voto de un miembro sobre una propuesta de cambio (change_proposal.v1, fase 8b, #327) |
| [`change_vote_status.v1`](change_vote_status.v1.schema.json) | Change vote status v1 | Estado de una propuesta de cambio (change_proposal.v1) en un momento, contando sus votos (change_vote.v1) con las reglas de config/votes.toml (spec §1.2) |
| [`chart_export.v1`](chart_export.v1.schema.json) | Chart export v1 | Lo que el motor exporta para la verificación visual y la comparación con una referencia (ROADMAP 3b, #81) |
| [`daily_candle.v1`](daily_candle.v1.schema.json) | Daily candle v1 | Vela diaria de un instrumento (#103), para los análogos v1 y las medidas propias de sentimiento (ADR 0004 y 0005) |
| [`daily_drawdown.v1`](daily_drawdown.v1.schema.json) | Daily drawdown v1 | Caída desde el máximo de una vela diaria cerrada de un instrumento (#213, spec §9.1) |
| [`daily_indicator.v1`](daily_indicator.v1.schema.json) | Daily indicator v1 | Indicadores de una vela diaria cerrada de un instrumento (#148, spec §13) |
| [`daily_vol_tercile.v1`](daily_vol_tercile.v1.schema.json) | Daily volatility tercile v1 | Tercio de la volatilidad realizada de una vela diaria cerrada frente a su propia historia (spec §13 y spec ML §3) |
| [`economic_event.v1`](economic_event.v1.schema.json) | Economic event v1 | Un evento del calendario económico (#91) |
| [`emotional_state.v1`](emotional_state.v1.schema.json) | Emotional state v1 | Estados emocionales de la spec §9.1 (#220) de un instrumento en un momento |
| [`fund_movement.v1`](fund_movement.v1.schema.json) | Fund movement v1 | Un aporte o retiro del fondo del grupo ya liquidado (spec §14, #179) |
| [`fund_summary.v1`](fund_summary.v1.schema.json) | Fund summary v1 | Resumen del fondo del grupo (spec §14, #312) |
| [`futures_bar.v1`](futures_bar.v1.schema.json) | Futures bar v1 | Una barra de 1 minuto de un futuro, tal como la publica la fuente (ADR 0011, #247; hoy Databento, esquema ohlcv-1m) |
| [`futures_bar_1d.v1`](futures_bar_1d.v1.schema.json) | Futures daily bar v1 | Una vela diaria de un futuro tal como la publica la fuente (#522; hoy Databento, esquema ohlcv-1d) |
| [`fvg_event.v1`](fvg_event.v1.schema.json) | FVG event v1 | Un fair value gap (definición de la spec §5.4) con su creación, toque, mitad, llenado, vencimiento y reacción, evaluado con velas cerradas hasta as_of (spec §5… |
| [`fx_rate.v1`](fx_rate.v1.schema.json) | FX rate v1 | Un tipo de cambio diario de referencia, tal como lo publicó la fuente (ADR 0013 §4 |
| [`gateway_calendar.v1`](gateway_calendar.v1.schema.json) | Gateway calendar v1 | Respuesta de GET /api/v1/calendar[?from=<UTC>][&to=<UTC>] del gateway (#600) |
| [`gateway_ibkr_bars.v1`](gateway_ibkr_bars.v1.schema.json) | Gateway IBKR bars v1 | Respuesta de GET /api/v1/ibkr/bars?instrument=<id>&timeframe=<tf>[&limit=<n>][&before=<UTC>] del gateway (#551) |
| [`gateway_ibkr_watchlist.v1`](gateway_ibkr_watchlist.v1.schema.json) | Gateway IBKR watchlist v1 | Respuesta de GET /api/v1/ibkr/watchlist del gateway (#551) |
| [`gateway_instruments.v1`](gateway_instruments.v1.schema.json) | Gateway instruments v1 | Respuesta de GET /api/v1/instruments del gateway (#231) |
| [`gateway_journal.v1`](gateway_journal.v1.schema.json) | Gateway journal v1 | El diario de paper que devuelve el gateway en GET /api/v1/journal (#615) para la vista Diario de la UI |
| [`gateway_news.v1`](gateway_news.v1.schema.json) | Gateway news v1 | Respuesta de GET /api/v1/news[?limit=<n>][&before=<UTC>][&feed=<feed>] del gateway (#600) |
| [`ibkr_bar.v1`](ibkr_bar.v1.schema.json) | IBKR bar v1 | Una vela CERRADA tal como la da IBKR como fuente de datos (#536) |
| [`ibkr_live_bar.v1`](ibkr_live_bar.v1.schema.json) | IBKR live bar v1 | Cada mensaje (campo data de un evento SSE) de GET /api/v1/ibkr/live?instrument=<id>&timeframe=<tf> del gateway (#558) |
| [`imbalance.v1`](imbalance.v1.schema.json) | Imbalance v1 | Desequilibrio aproximado desde el precio al cierre de una vela (spec §5.4.1, #231) |
| [`instrument.v1`](instrument.v1.schema.json) | Instrument v1 | Un instrumento operable o de contexto |
| [`learning_report.v1`](learning_report.v1.schema.json) | Learning report v1 | Informe de aprendizaje continuo (fase 8b, spec §15b, #327), armado solo con el diario (proposal.v1 + outcome.v1) de un período |
| [`liquidity_level.v1`](liquidity_level.v1.schema.json) | Liquidity level v1 | Un nivel del mapa de liquidez (spec §5.4.1, #231) |
| [`liquidity_stats.v1`](liquidity_stats.v1.schema.json) | Liquidity stats v1 | Agregados de FVG (fvg_event.v1) o de gaps de apertura (opening_gap.v1) de un grupo |
| [`liquidity_view.v1`](liquidity_view.v1.schema.json) | Liquidity view v1 | Respuesta de GET /api/v1/liquidity?instrument=<id>[&timeframe=15m\|1h\|1d][&source=ig\|futures\|ibkr\|auto][&before=<UTC>] del gateway (#231) |
| [`ml_experiment.v1`](ml_experiment.v1.schema.json) | ML experiment v1 | Un entrenamiento y evaluación walk-forward del modelo de ML v1 (docs/spec/ml-v1.md §7, ADR 0003 §D) |
| [`ml_experiment_entry.v1`](ml_experiment_entry.v1.schema.json) | ML experiment entry v1 | Respuesta del gateway en GET /api/v1/ml/experiments (#426) |
| [`ml_features.v1`](ml_features.v1.schema.json) | ML features v1 | Variables de una propuesta para el modelo de ML v1 que no viajan en proposal.v1 (docs/spec/ml-v1.md §3, #158) |
| [`ml_label.v1`](ml_label.v1.schema.json) | ML label v1 | Etiqueta de una propuesta para el modelo de ML v1, sin P/L (docs/spec/ml-v1.md §1, #399, opción C del dueño) |
| [`ml_label_run.v1`](ml_label_run.v1.schema.json) | ML label run v1 | Una corrida de etiquetas sin P/L (#399) |
| [`news_interpretation.v1`](news_interpretation.v1.schema.json) | News interpretation v1 | Interpretación automática de una noticia de news_item.v1 (spec §9, #122) |
| [`news_item.v1`](news_item.v1.schema.json) | News item v1 | Una noticia de una fuente aprobada (ADR 0006), tal como la publicó la fuente (#109) |
| [`official_release.v1`](official_release.v1.schema.json) | Official release v1 | Una publicación oficial programada o no programada (ADR 0016) |
| [`opening_gap.v1`](opening_gap.v1.schema.json) | Opening gap v1 | Un gap de apertura diario (spec §5.4.1, #231) |
| [`ops_event.v1`](ops_event.v1.schema.json) | Ops event v1 | Un evento de la operación (#314) |
| [`ops_status.v1`](ops_status.v1.schema.json) | Ops status v1 | Estado de la operación y del riesgo del broker simulado (fase 9, #314), para la UI |
| [`order.v1`](order.v1.schema.json) | Order v1 | Una orden de mercado para el broker (fase 7, #278) |
| [`order_report.v1`](order_report.v1.schema.json) | Order report v1 | Qué pasó con una orden (order.v1) en el broker simulado (fase 7, #278) |
| [`outcome.v1`](outcome.v1.schema.json) | Outcome v1 | El resultado de una propuesta del diario (spec §15b) |
| [`paper_account.v1`](paper_account.v1.schema.json) | Paper account v1 | Estado de la cuenta del broker simulado (fase 7, #278) al cierre de la última vela (as_of) |
| [`paper_comparison.v1`](paper_comparison.v1.schema.json) | Paper comparison v1 | Comparación de paper trading (registro del broker simulado) contra el backtest del mismo período, sobre las mismas velas cerradas y la misma config (fase 8, #3… |
| [`position.v1`](position.v1.schema.json) | Position v1 | Una posición abierta del broker simulado (fase 7, #278), valuada al cierre de la última vela (as_of) |
| [`proposal.v1`](proposal.v1.schema.json) | Proposal v1 | Una propuesta de operación del diario (spec §15b) |
| [`reference_rate.v1`](reference_rate.v1.schema.json) | Reference rate v1 | Un valor diario de un tipo de referencia de la financiación nocturna (#79), tal como lo publicó la fuente |
| [`sentiment.v1`](sentiment.v1.schema.json) | Sentiment index v1 | Índice de sentimiento v1 de un instrumento (spec §9.1, #213) |
| [`sentiment_observation.v1`](sentiment_observation.v1.schema.json) | Sentiment observation v1 | Un dato crudo de una fuente de sentimiento (#25, #27), tal como lo publicó la fuente |
| [`ticket_verdict.v1`](ticket_verdict.v1.schema.json) | Ticket verdict v1 | Lo que respondería el módulo de riesgo a una orden (order.v1) **sin enviarla** (fase 7, #278) |
| [`treasury_yield.v1`](treasury_yield.v1.schema.json) | Treasury yield v1 | Un rendimiento diario de la curva par del Tesoro de EE |
| [`vix_daily.v1`](vix_daily.v1.schema.json) | VIX daily v1 | Un día del Cboe Volatility Index (VIX) |
| [`walk_forward_report.v1`](walk_forward_report.v1.schema.json) | Walk-forward report v1 | Evaluación de la Fase 5 (ROADMAP, #298) de un instrumento |
<!-- indice-de-contratos:fin -->

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
| `proposal.v1` | Una propuesta: instrumento, dirección, hora, entrada de referencia, stop, TP1, tamaño, apalancamiento, puntaje con su desglose por señal y gatillos. Id propio e idempotente. Opcional `shown` (#333, cambio compatible): la probabilidad mostrada §13 tal como se mostró (`estimate` con su intervalo, o `insufficient`); la guardan las propuestas de paper (columna `shown`, migración 0036), las del backtest no. |
| `outcome.v1` | Su resultado, `simulated` o `real`: entrada, cada salida (hora, precio, fracción y motivo), costos, P/L bruto y neto (con signo), si tocó TP1 o el stop. |

Los escribe el motor (`cfds-journal`, tablas `proposals` y `outcomes` de `infra/migrations/0002_journal.sql`); los lee el aprendizaje en Python. Las columnas tienen los mismos nombres que los contratos; `breakdown`, `triggers` y `exits` son `JSONB` con la forma del contrato. Montos en la moneda del instrumento.

| Contrato | Tabla | Contenido |
|---|---|---|
| `backtest_run.v1` | `backtest_runs` | Una corrida de backtest (#200): `run_id`, instrumentos, timeframe de decisión, período `[from, to)`, `config_sha256` (SHA-256 de los archivos de `config/` que leyó, cada uno con su nombre), capital inicial, costos usados (slippage y costos de cada instrumento), noticias (intérprete, modelo, `published_from` del corte del ADR 0008 §5 y cantidad de interpretaciones, o `null`), resumen de métricas (decimales como texto, `null` si no están definidas) y `started_at`/`finished_at` en UTC. |
| `backtest_run_curve.v1` | `backtest_run_curves` | La curva de una corrida (#299): capital al cierre de cada día con el drawdown de ese día (lo calcula el motor; su máximo es `max_drawdown`) y comprar y mantener de cada instrumento con los mismos costos. Una por corrida, de solo agregar. |
| `backtest_run_detail.v1` | (respuesta del gateway) | `GET /api/v1/backtest_runs/{run_id}` (#299): la corrida (`backtest_run.v1`), su curva (o `null`) y sus operaciones (`proposal.v1` con su `outcome.v1` simulado o `null`). Solo junta contratos existentes. |
| `gateway_journal.v1` | (respuesta del gateway) | `GET /api/v1/journal[?limit=<n>]` (#615): el diario de paper para la vista Diario: las propuestas con `source = paper` (`proposal.v1`), de la señal más nueva a la más vieja, con su `outcome.v1` simulado o `null`, y el `limit` aplicado. Solo junta contratos existentes; solo lectura. |

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
- La arma el motor en Rust, solo con información al cierre de la vela de la señal. La escriben `backtest_run` con `run_id` y el ejemplo `ml_dataset` (#399).
- La lee Python (`cfds_intel.models.dataset`), junto con `proposals` y `outcomes` de una corrida de `backtest_runs`, o con `ml_labels` (abajo).

| Contrato | Tabla | Contenido |
|---|---|---|
| `ml_label_run.v1` | `ml_label_runs` | Una corrida de etiquetas sin P/L (#399): `run_id`, `instrument_id` (por ejemplo `US500.FUT`), `candle_source` (`futures` o `ig`), período de los datos `[from, to)` (inicio de la primera vela de 15m y fin de la última), `method` y `config_sha256`. Idempotente por `run_id`: otro contenido con el mismo `run_id` es un error. |
| `ml_label.v1` | `ml_labels` | Etiqueta de una propuesta sin P/L (spec ML §1, #399): `proposal_id`, `signal_at`, `y` (0 o 1), `exit_reason` (`tp1`, `stop`, `time` o `forced`), `resolved_at` (fin de la vela en la que se conoce el resultado, para la purga) y `method` (`mid_no_costs`: precio medio, sin costos ni financiación). |

- Las dos tablas son de **solo agregar** (`infra/migrations/0037_ml_labels.sql`, trigger). Una etiqueta por propuesta **resuelta**. Si los datos terminan antes del resultado, no hay fila.
- La escribe el motor (`cfds_journal::ml_label`, ejemplo `ml_dataset` de `cfds-backtest`) junto con las `proposals` (`source = 'backtest'`, con su `run_id`) y las `ml_features` de la misma corrida. No hay fila en `backtest_runs` ni en `outcomes`: esas corridas no tienen P/L (las tasas de financiación no cubren 2011–2019).
- La lee Python (#400): el período de `ml_label_runs` y las `proposals` del `run_id` con `ml_labels` y `ml_features`.

| Contrato | Tabla | Contenido |
|---|---|---|
| `ml_experiment.v1` | `ml_experiments` | Un entrenamiento y evaluación walk-forward (spec ML §7): id propio (SHA-256 de código, corrida, configuración y datos), versión del código (commit de git), corrida de `backtest_runs` o, con `label_source = ml_label_runs` (opcional, #409), de `ml_label_runs`, variables, hiperparámetros, cada ventana (con su motivo si no entrenó), métricas de B0, B1 y M1 (números: no son dinero) y el resultado: `suspended`, `m1_improves` o `b1_stays`. |

- Se registran **todos**, también los que no funcionan o quedan en suspenso. Sirve para el Sharpe deflactado y el PBO de la fase 5.
- Tabla de **solo agregar** (`infra/migrations/0015_ml_experiments.sql`): las columnas de arriba más `document`, el `JSONB` con el documento entero. El mismo experimento no se repite.
- Lo escribe Python (`python -m cfds_intel.models.experiment <run_id>`).
- `calibration` (opcional, #433, cambio compatible): la calibración por grupos de la predicción (spec ML §6) sobre todas las validaciones juntas, la misma que muestra el informe. Lleva `bins` (de `config/ml.toml`), `interval` (`wilson_95`) y, por modelo en `pooled`, cada grupo con `p_min` y `p_max` (desde/hasta), `n`, `mean_predicted`, `observed` y el intervalo `low`/`high`. Falta en los experimentos guardados antes de #433 y en los que quedaron en suspenso; quien lo lea (la UI) lo muestra tal cual, sin calcular nada.

| Contrato | Tabla | Contenido |
|---|---|---|
| `ml_experiment_entry.v1` | (respuesta del gateway) | `GET /api/v1/ml/experiments[?instrument=<id>]` (#426): cada `ml_experiment.v1` tal como está guardado, con el instrumento de su corrida (`instrument_id` de `ml_label_runs` o cada uno de `instruments` de `backtest_runs`) y su `label_source`. Sin `instrument`, el último experimento de cada instrumento; con `instrument`, los de ese instrumento, del más nuevo al más viejo (hasta 50). Solo lectura; lo muestra Performance como «Modelo (informativo)». |

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

### Estados emocionales (spec §9.1, #220)

| Contrato | Tabla | Contenido |
|---|---|---|
| `emotional_state.v1` | `emotional_states` | Los estados emocionales activos de un instrumento en un momento, cada uno con los valores de su regla como texto: `instrument_id`, `source` (`price` o `sentiment`), `basis` (`15m`, `1h` o `sentiment_index`), `as_of` (desde cuándo se conoce) y `states` (vacío si no hay ninguno). |

- **Precio (Rust):** FOMO de precio y clímax de venta en 1h (`cfds_regime`), rechazo en 15m y 1h (`cfds_liquidity`), juntados por `cfds_strategy::price_emotional_states`. Un registro por cada vela **cerrada**, con `as_of` = su cierre. Lo guarda `cargo run -p cfds-ingest --example emotional_states` (`[-- --fuente futuros]`), sin IG ni cupo.
- **Índice (Python):** miedo extremo, euforia extrema y complacencia (`cfds_intel.sentiment.states`, `config/sentiment.toml` [states]), en cada valor del índice y en cada momento sin índice (donde solo puede haber complacencia), con `as_of` = `published_at`. Lo guarda `python -m cfds_intel.sentiment.compute_index`.
- **Distribución:** no se calcula todavía (pendiente de volumen real, spec §9.1).
- Tabla de **solo agregar** (trigger), clave `(instrument_id, source, basis, as_of)` (`infra/migrations/0033_emotional_states.sql`). Guardar lo mismo otra vez no hace nada; el mismo momento con otros estados es un error (sería repintado).
- **Sin mirar el futuro:** se lee lo conocido hasta un momento (`as_of <= t`). El gateway lo sirve en `GET /api/v1/emotions` (el último de cada instrumento, fuente y base) y la UI lo muestra al lado del índice de sentimiento. Lo puede leer el arnés de hipótesis (H1–H7, otro Issue).
- **Informativo:** ningún estado suma al puntaje hasta validarse en la fase 5 y votarse. No es un pronóstico.

### Tipos de referencia (#79)

| Contrato | Tabla | Contenido |
|---|---|---|
| `reference_rate.v1` | `reference_rates` | Valor diario de un tipo de referencia de la financiación nocturna, tal como lo publicó la fuente: `series` (mismo nombre que `[funding_benchmarks]`, hoy `ESTR`), `ref_date`, `published_at` y `rate_pct` (string decimal, puede ser negativo). |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Una corrección de la fuente es otra fila con otra `published_at`.
- `published_at` nunca es anterior al fin de `ref_date`: un valor solo se usa desde que se publicó (CLAUDE.md §5b).
- La escribe Python (`cfds_intel.rates`). La lee el motor para el costo de financiación de cada noche.

### Tipos de cambio (ADR 0013 §4, #418)

| Contrato | Tabla | Contenido |
|---|---|---|
| `fx_rate.v1` | `fx_rates` | Tipo de cambio diario de referencia, tal como lo publicó la fuente: `pair` (BASE+COTIZADA, hoy `EURUSD`: dólares por 1 euro, BCE), `ref_date`, `published_at` y `rate` (string decimal positivo). |

- Tabla de **solo agregar** (trigger). Una corrección es otra fila, publicada cuando se leyó. `rate > 0` y `published_at` no anterior al día (`CHECK`).
- `published_at`: el mismo día a las 17:00 de Fráncfort (`config/fx_rates.toml`; el BCE publica «alrededor de las 16:00 CET», se deja una hora de margen). Nunca antes.
- La escribe Python (`python -m cfds_intel.rates.fx`). La lee el motor (`PgStore::fx_rates`, `cfds_backtest::FxRates`) para convertir montos de un contrato en USD a la cuenta en EUR con el tipo vigente en cada momento.

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
| `gateway_news.v1` | (respuesta del gateway) | `GET /api/v1/news[?limit=<n>][&before=<UTC>][&feed=<feed>]` (#600): noticias de `news_items` (`news_item.v1`) de la más nueva a la más vieja, cada una con sus interpretaciones (`news_interpretation.v1`) ya existentes, y `next_before` para la página siguiente. Solo junta contratos existentes; nada aparece antes de su `published_at` / `interpreted_at`. |
| `gateway_calendar.v1` | (respuesta del gateway) | `GET /api/v1/calendar[?from=<UTC>][&to=<UTC>]` (#600): eventos de `economic_events` (`economic_event.v1`) y publicaciones de `official_releases` (`official_release.v1`) con hora en `[from, to]`, publicados a más tardar ahora. Solo lectura. |

- Una interpretación existe desde `interpreted_at`, que nunca es anterior a `published_at` de la noticia (CLAUDE.md §5b).
- El **ajuste de −2 a +2** del puntaje (spec §6 y §9) **no** viaja en el evento: lo calcula el motor en Rust a partir de `sentiment` y `relevance`, porque el puntaje vive en un solo lugar.
- La interpreta Python (`cfds_intel.news.interpret`, con un intérprete reemplazable; hoy Ollama local, ADR 0008) y la guarda y publica con `python -m cfds_intel.news.interpret_new` (#186), que toma las noticias de `news_items` sin interpretación del intérprete fijado.
- **Tabla `news_interpretations`** (`infra/migrations/0011_news_interpretations.sql`): de **solo agregar** (trigger), clave `interpretation_id`. Mismas columnas que el contrato, con `interpreter` aplanado en `interpreter_name`, `interpreter_version`, `interpreter_model` e `interpreter_prompt_sha256`, e `impacts` como `JSONB` con la forma del contrato. `CHECK interpreted_at >= published_at` y clave foránea a `news_items`. Una noticia tiene una sola interpretación por intérprete (nombre y versión): guardarla otra vez no guarda nada y se conserva la primera; la misma versión con otro modelo o prompt es un error.
- **Stream de Redis `news_interpretations.v1`** (uno solo para todas las fuentes): una entrada por interpretación **nueva**, con ID automático de Redis (`*`) y dos campos: `data` = JSON de `news_interpretation.v1` e `interpretation_id`. Se publica dentro de la transacción del guardado, antes del commit: si Redis falla no se guarda nada. Entrega **al menos una vez**: el consumidor deduplica por `interpretation_id`, y la tabla es la fuente de verdad.
- **Lo lee el motor** con `cfds_storage::NewsInterpretationConsumer` (#197): consumer group de Redis (`XREADGROUP` + `XACK`), así una entrada entregada y no confirmada se vuelve a entregar después de un reinicio. Deduplica por `interpretation_id` con el set `<stream>.seen.<grupo>`, que se actualiza junto con el `XACK` en una transacción. Una entrada que no cumple el contrato se informa, se confirma y no vuelve. Todavía no está conectado a la estrategia en vivo.
- La lee Rust con `cfds_core::NewsInterpretation`, que acepta lo mismo que el contrato y además exige `interpreted_at >= published_at`. El ajuste del puntaje (`cfds_strategy::news_adjustment`, #130) usa solo las interpretaciones con `interpreted_at` ≤ el cierre de la vela.
- **Look-ahead del modelo en el backtest (ADR 0008 §5, #129):** un modelo entrenado después de una noticia puede "saber" qué pasó. En vivo, cada interpretación vale desde `interpreted_at`. En el backtest y en la validación de la fase 5 solo entran las noticias publicadas desde la fecha de corte del modelo (`training_cutoff` de `config/news_interpreter.toml`; si el fabricante no la publica, la fecha de publicación del modelo). Lo aplica el motor (`InterpreterCutoff::backtest_published_from` y `cfds_backtest::run_with_news`), y `backtest_run.v1` registra ese corte en `published_from`. Las noticias anteriores se pueden interpretar para mostrarlas, pero no entran.

### Calendario económico (#91, #239, ADR 0006 y ADR 0010)

| Contrato | Tabla | Contenido |
|---|---|---|
| `economic_event.v1` | `economic_events` | Un evento del calendario: `source`, `event`, `country` (ISO 3166-1 alfa-2, `EU` = zona euro) o `instrument_id`, `impact` (`low`/`medium`/`high`), `scheduled_at` y `published_at`, en UTC. |

- Hoy la fuente es el feed JSON oficial de Forex Factory (`source = 'forex_factory'`, ADR 0010). Lo lee y guarda Python con `python -m cfds_intel.calendar`. La moneda del feed pasa a país con `config/calendar.toml`, y los feriados (`Holiday`) no se guardan.
- **`published_at` es la primera vez que se vio el evento** (anti look-ahead, ADR 0006 §3). No hay histórico: el calendario se acumula desde que empieza la ingesta.
- Tabla de **solo agregar** (`infra/migrations/0022_economic_events.sql`, trigger), con clave natural `(source, event, country, instrument_id, impact, scheduled_at)` y `NULLS NOT DISTINCT`. Leer otra vez el mismo evento no lo repite y conserva su primer `published_at`. Si la fuente cambia la hora o el impacto, es **otra fila** con su propio `published_at`; la anterior queda.
- Para leerla sin mirar el futuro: `published_at <= as_of` (`cfds_intel.calendar.store.events_known_at`). La lee el motor (`PgStore::economic_events` y `PgStore::economic_event_sightings`, #243) para el filtro §7.2 de `backtest_run` y `chart_export` (`cfds_market_data::NewsFilter`): un evento cuenta en una vela solo si `published_at ≤` su cierre, y una vela sin una lectura de la ingesta en los `calendar_stale_after_days` anteriores (`config/news_filter.toml`) se informa como "sin calendario".

### Calendario histórico oficial (#520, ADR 0016)

| Contrato | Tabla | Contenido |
|---|---|---|
| `official_release.v1` | `official_releases` | Una publicación oficial: `source` (`fed_fomc` o `bls`), `series` (`fomc_statement`, `cpi`, `employment_situation`), `reference_period` (día de la reunión del FOMC `AAAA-MM-DD` o mes de referencia del BLS `AAAA-MM`), `status` (`scheduled`, `rescheduled`, `cancelled`, `unscheduled`), `scheduled_at` (null solo si se canceló) y `published_at` en UTC, `time_rule` (`release_page` o la regla de `config/official_calendar.toml`), `with_press_conference` (solo FOMC) y `source_url`. |

- Aparte de `economic_event.v1` a propósito (ADR 0016 §5): otra fuente, otro uso (estudio de la familia E) y campos que ese contrato no tiene. Dice **cuándo** hubo un anuncio, no **qué** dijo.
- Hoy la escribe Python con `python -m cfds_intel.calendar.official`: los comunicados del FOMC 2011–2025 de federalreserve.gov, con la hora de su página o, si no la trae, la regla de `config/official_calendar.toml` (2:15 p.m. hasta marzo de 2011; desde abril de 2011, ~12:30 con conferencia y 2:15 sin; 2:00 p.m. desde marzo de 2013). La hora del Este pasa a UTC con `America/New_York`. Con `--source bls` escribe el CPI y el empleo del BLS 2011–2026 (#608), leídos **solo** de las páginas que el dueño guardó a mano (sin robot, ADR 0016 §1.3): 08:30 ET a UTC, `published_at` = el informe anterior de la serie, reprogramaciones (`rescheduled`, la fila original queda) y cancelaciones (`cancelled`, sin hora) de los cortes de 2013, 2025 y 2026; `time_rule` `bls_calendar_0830_et` o `bls_notice_last_modified`.
- **`published_at` (anti look-ahead, ADR 0016 §4):** una reunión programada, desde el comunicado del calendario tentativo que la anunció con sus fechas exactas (sin hora en la página, el final de ese día ET); una no programada (`unscheduled`, por ejemplo las de marzo de 2020), desde su propio comunicado (`published_at = scheduled_at`). Una reunión que no se puede documentar queda fuera. `CHECK published_at <= scheduled_at`.
- Tabla de **solo agregar** (`infra/migrations/0046_official_releases.sql`, trigger), clave natural `(source, series, reference_period, status, scheduled_at)` con `NULLS NOT DISTINCT`. Una segunda carga no agrega nada; una fila guardada con otros datos es un error (no se pisa).
- Para leerla sin mirar el futuro: `published_at <= as_of` (`cfds_intel.calendar.official.store.releases_known_at`).

### Histórico de futuros (#247, ADR 0011 en propuesta)

| Contrato | Tabla | Contenido |
|---|---|---|
| `futures_bar.v1` | `futures_bars_1m` | Una barra de 1 minuto de un futuro, tal como la publica la fuente: `source` (`databento`), `dataset`, `symbol` (contrato continuo, por ejemplo `ES.c.0`), `instrument_id` (nuestro CFD: US500, US100, US30, DE40), `contract_id` (el contrato real: si cambia, hubo vencimiento), `start` (inicio en UTC, justo en un minuto), OHLC como string decimal y `volume`. |
| `futures_bar_1d.v1` | `futures_bars_1d` | Una vela **diaria** de un futuro tal como la publica la fuente (#522; Databento `ohlcv-1d`): mismos campos que `futures_bar.v1`, pero `start` es la **medianoche UTC** del día que cubre. No es la vela de la hora de liquidación de la bolsa ni la diaria de IG. Hoy son datos de investigación: `instrument_id` es la raíz del futuro (por ejemplo `NKD`) y no está en `config/instruments.toml`. |

- La escribe Python (`python -m cfds_intel.databento --run`, solo con `CFDS_DATA_MACHINE=1`). La lee Rust con `cfds_core::FuturesBar` (`PgStore::futures_bars`), que acepta lo mismo que el contrato, y la agrupa y verifica `cfds_market_data::futures` (#248).
- `futures_bar_1d.v1` la escribe Python (`python -m cfds_intel.databento --daily --run`, config `config/databento_daily.toml`, tope de USD 1 por corrida). Tabla de solo agregar (`infra/migrations/0047_futures_bars_1d.sql`). `futures_bar_loads` lleva desde #522 la columna `schema` (`ohlcv-1m` u `ohlcv-1d`) en su clave.
- Tabla de **solo agregar** (`infra/migrations/0023_futures_bars.sql`, trigger), con clave `(source, dataset, symbol, start_at)`. La misma barra otra vez no cambia nada, y otra con la misma clave es un conflicto: no se guarda nada del tramo. `futures_bar_loads` registra cada tramo bajado y su costo estimado, para no volver a pagarlo.
- Solo barras del **libro central**: en Eurex, OHLCV trae un registro aparte por publisher cuando en el minuto hubo operaciones fuera de libro (`XEUR.EOBI.XOFF`). Esas se descartan y se cuentan (`publisher` en `config/databento.toml`, #254).
- Precios **sin ajustar** por vencimientos.
- El motor arma con estas barras velas de 15m y 1h con la forma de `candle.v1` y las guarda en `futures_candles` (`infra/migrations/0024_futures_candles.sql`, #259), una tabla **aparte** de `candles`: así nada que lea las velas de IG puede mezclarlas con las del futuro. Es de solo agregar, con clave `(source, instrument_id, timeframe, start_at)` y el contrato de cada vela. Solo la lee Rust.
- **Diarias de los futuros** (#282): el motor arma con las velas de 15m de `futures_candles` diarias con la forma de `daily_candle.v1` y la misma ventana que las de IG (medianoche a medianoche en `config/candle_sources.toml` `daily_timezone`, igual a `config/ig.toml` `prices_timezone`), solo los días con sesión del CFD (`config/sessions.toml` y feriados) y, el día del vencimiento (calendario de #270), solo con las velas del contrato nuevo. Se guardan en `daily_candles` bajo el instrumento derivado (por ejemplo `US500.FUT`): nunca pisan las de IG, y `daily_indicators`, `daily_drawdowns`, `daily_vol_terciles`, `sentiment_index` y los análogos las usan con `--fuente futuros` sin cambiar ningún contrato. Limitación: el retorno diario del día del vencimiento incluye el salto de base entre contratos.
- **Liquidez con los futuros** (#259): `liquidity_levels` y `liquidity_stats` con `--fuente futuros` calculan lo mismo con estas velas, y lo guardan bajo un **instrumento derivado** (`<instrumento><id_suffix>`, por ejemplo `US500.FUT`, `config/candle_sources.toml`). Así las tablas de niveles, FVG, gaps, desequilibrio y agregados (y sus contratos) no cambian y nunca mezclan las dos fuentes. Desde #630, igual con las barras de IBKR (`--fuente ibkr`, `ibkr_bars`) de los CFD con mapeo en `[ibkr.instruments]` (US100 → MNQ, US500 → MES), bajo `<instrumento>.IBKR`, con `id` `ibkr`. El gateway lo sirve con `?source=futures`, y `liquidity_view.v1` y `gateway_instruments.v1` traen el campo opcional `source` / `sources` (`id` `ig` o `futures` y su etiqueta) para que la UI muestre de dónde son los precios. No se mezcla con `candles` (IG): el spread y los costos siguen siendo los de IG.
- Los datos viven solo en la base local de la máquina de datos, **nunca en el repo**.

| Contrato | Tabla | Contenido |
|---|---|---|
| `ibkr_bar.v1` | `ibkr_bars` | Una vela **cerrada** de IBKR como fuente de datos (#536): `source` (`ibkr`), `instrument_id` (el id de `config/ibkr.toml`: MNQ, MES, QQQ, EURUSD), `kind` (`future`, `stock` o `forex`), `contract` (como lo nombra IBKR: MNQZ6, QQQ, EUR.USD), `bar_size` (`1m`, `15m` o `1h`), `what_to_show` (`TRADES` o `MIDPOINT`), `start` (inicio en UTC, alineado), OHLC como string decimal y `volume` (null si IBKR no lo da). |

- La escribe Rust (`cargo run -p cfds-ibkr --example ibkr_load`, solo en la máquina con IB Gateway paper) con `PgStore::insert_ibkr_bars`. Tabla de **solo agregar** (`infra/migrations/0048_ibkr_bars.sql`, trigger), clave `(source, instrument_id, contract, bar_size, what_to_show, start_at)`: la misma vela otra vez no cambia nada y con otros valores es un conflicto (sería repintado) y no se guarda nada del lote.
- Precios **sin ajustar**. El futuro continuo de IBKR (CONTFUT) **no entra**: está ajustado por vencimiento y repinta. Tampoco las diarias (IBKR las fecha por sesión, sin hora). Ver `docs/analysis/2026-11-ibkr-medicion-historia.md`.
- Tabla aparte de `candles` (IG), `futures_bars_1m` y `futures_candles` (Databento): ninguna lectura las mezcla sin querer.

#### Vela en formación de IBKR (#558, solo visual)

| Contrato | Canal | Contenido |
|---|---|---|
| `ibkr_live_bar.v1` | SSE `GET /api/v1/ibkr/live?instrument=<id>&timeframe=1m\|5m\|15m\|1h` del gateway (no se guarda) | La vela **en formación** de un instrumento y timeframe, armada por el motor (`cfds_ibkr::live`) con los precios por streaming de IBKR, y el estado: `status` (`disabled`, `connecting`, `disconnected`, `stopped`, `rejected`, `waiting`, `streaming`) con `reason`, `data_type` (`live`, `delayed`, `frozen`, `delayed_frozen`) tal como lo informa IBKR, `delay_seconds` medido, `contract`, `forming` y `bar` (`start`, `end`, OHLC como string decimal, `partial` si empezó a mitad de su período, `ticks`). Como mucho un mensaje por segundo por conexión; la vela cerrada llega una vez con `forming = false`. |

- **Solo visual**: ningún indicador, patrón, señal ni estrategia la usa (un test de `cfds-ibkr` revisa que ningún crate que decide dependa de ella). La vela que vale es la histórica de `ibkr_bars`.
- IBKR manda muestras del último precio (medido en demorados: una cada ~5 s), no cada operado: al cerrar, la apertura, el máximo y el mínimo pueden quedar adentro del rango de la histórica (ver `docs/analysis/2026-11-ibkr-medicion-historia.md`, #558).

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
| `liquidity_level.v1` | tablas `liquidity_levels` y `liquidity_level_events` | Un nivel del mapa de liquidez: `instrument_id`, `kind` (`eqh`, `eql`, `pdh`, `pdl`, `pwh`, `pwl`, `session_high`, `session_low` y, desde #497, `round_high` y `round_low`: el número redondo más cercano por encima o por debajo del cierre; se guardan desde #504, migración `0045`), `session` (`asia`, `london` o `ny` en los de sesión; `null` en los demás), `price` (string decimal), `exists_from` (UTC), `state` (`intact`, `swept` o `broken`) y `state_changed_at` (cierre de la vela que lo barrió o rompió; `null` si está intacto) y, opcional, `expired_at` (cierre de la vela número `max_age_bars` si llegó intacto hasta ahí: desde ese momento ya no está activo). |

- **Solo informativo** en v1: no suma al puntaje de §6.
- Lo calcula **Rust** (`cfds_liquidity::liquidity_levels`) con velas cerradas de 15m, los pivotes de `config/patterns.toml`, el calendario de `config/sessions.toml` y los parámetros de `config/liquidity.toml`. Nadie lo recalcula; la UI solo lo muestra.
- Un nivel no existe antes de `exists_from`. Su estado cambia a lo sumo una vez y nunca se reescribe (sin repintado). Rust exige además `state_changed_at > exists_from`.
- `expired_at` se agregó a v1 como campo **opcional** (cambio compatible): un documento sin él sigue siendo válido. Hace falta porque saber si un nivel intacto venció exige contar velas, y ese cálculo vive solo en Rust.
- **Tablas** (`infra/migrations/0020_liquidity_levels.sql`; `0045` amplía el CHECK de `kind` con `round_high` y `round_low`), las dos de **solo agregar** (trigger):
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
| `liquidity_view.v1` | Respuesta de `GET /api/v1/liquidity?instrument=<id>[&timeframe=15m\|1h\|1d]` del gateway: `instrument_id`, `timeframe` (opcional, agregado sin romper v1; ausente = 15m), `generated_at`, `as_of` (cierre de la última vela cerrada guardada de ese timeframe, o `null` si no hay velas) y las listas `candles` (`candle.v1` en 15m y 1h, `daily_candle.v1` en 1d), `levels` (`liquidity_level.v1`), `fvgs` (`fvg_event.v1`), `imbalance` (`imbalance.v1`) y `stats` (`liquidity_stats.v1`), cada elemento con su contrato sin cambios (`$ref`). Opcional `analysis` (#501, sin romper v1): el mismo bloque `analysis` de `chart_export.v1` (`$ref`), armado sobre las velas de la vista, solo en 15m con `source` `ig`. Opcional `fallback` (#628, sin romper v1): con `?source=auto`, si la vista no sale de IG por falta de velas, `from` (`ig`) y `note` (el motivo, de `[view.auto.notes]` de `config/gateway.toml`); `source` es la fuente que se usó. |

| `gateway_instruments.v1` | Respuesta de `GET /api/v1/instruments`: los instrumentos del catálogo (`instrument.v1`), los timeframes que acepta la vista y lo que abre primero (`[view]` de `config/gateway.toml`). Opcional `sources` (#259): las fuentes de velas con su etiqueta; desde #628 la primera es `auto` (IG, o la primera de reemplazo con velas), la que la UI abre por omisión. |

- Es un **sobre**: no agrega cálculos. El gateway (`engine/crates/gateway`) lo arma con las lecturas de `cfds_storage::PgStore`, todas tal como se conocían en `as_of`, y la ventana de cada timeframe de `config/gateway.toml`. Los FVG, el desequilibrio y los agregados son los del timeframe pedido (en 1d, también los agregados de los gaps de apertura); los niveles son los del mapa de liquidez (15m), tal como se conocían en `as_of`.
- Sin velas guardadas, `as_of` es `null` y las listas van vacías: la UI muestra "sin datos todavía".
- Los errores HTTP (400, 404, 405, 500) van como texto plano, sin datos.
- El gateway abre la sesión de PostgreSQL en **solo lectura** (`default_transaction_read_only`).

### Broker simulado (fase 7, #278)

| Contrato | Contenido |
|---|---|
| `order.v1` | Una orden de mercado: `order_id` propio e idempotente, `mode` (solo `paper`), instrumento, dirección, **stop obligatorio**, `tp1`, `proposal_id` y `created_at`. **Sin tamaño:** lo calcula solo el riesgo (spec §8). |
| `order_report.v1` | Qué pasó con la orden: `rejected` (con el motivo) o `filled` (posición, tamaño, precio con spread y slippage, comisión), y cuándo. `pending` (#471) solo en la respuesta del envío: el riesgo la aceptaría pero la vela del fill todavía no llegó (una orden manual de ahora); se decide con esa vela y recién ahí entra al registro. |
| `ticket_verdict.v1` | La respuesta del riesgo a una orden **sin enviarla**: acepta o rechaza, con tamaño, riesgo, % de riesgo, apalancamiento y margen. La UI la muestra. |
| `position.v1` | Una posición abierta valuada al cierre de `as_of`: entrada, stop, `tp1`, último precio, P/L no realizado (en moneda y en R) y margen. |
| `paper_account.v1` | La cuenta simulada al cierre de `as_of`: capital, P/L, margen, apalancamiento, pérdida diaria usada, posiciones y kill switch. |
| `ops_status.v1` | `GET /api/v1/ops/status` (#314): compuerta del broker (`starting`, `ready`, `stopping` o `unavailable`), si acepta órdenes, la cuenta (kill switch con el motivo ya redactado, pérdida diaria usada y su límite de `config/risk.toml`, apalancamiento y posiciones; `null` sin broker) y la última reconciliación (al arrancar o periódica). Solo lectura; los números son los de `paper_account.v1`. |
| `ops_event.v1` | `GET /api/v1/ops/events` (#314): una línea del log de `cfds-obs` ya redactada (`ts`, `level`, `target`, `event` y el resto en `fields`). Entran los eventos del broker, las alertas y lo `warn` o `error`; en memoria, los últimos `[ops] feed_size`, y guardados en la tabla `ops_events` (#337, ver abajo). Desde #576 los eventos iguales (mismo `level`, `target`, `event` y `fields`) van **agrupados** en una fila con `count` y `last_seen` (opcionales: si faltan, 1 y `ts`; `ts` es la primera vez); la ruta devuelve `[ops] events_limit` grupos por defecto y `?limit=` hasta `[ops] feed_size`. Que un cliente cierre la conexión (recarga, cambio de vista, fin de un SSE) no es un evento: el gateway lo registra como `info` `client_closed`, fuera del feed y de la base. |

- **`ops_events`** (`infra/migrations/0035_ops_events.sql`, #337): los eventos de `ops_event.v1` **ya redactados** por el `Logger` de `cfds-obs`. Los guarda el gateway en segundo plano (`cfds_gateway::ops_store`): el broker nunca espera a la base; si la base no responde, el feed sigue en memoria, se reintenta cada `[ops] retry_secs` y salta la alerta de desconexión. `GET /api/v1/ops/events` lee de la base y, si falla, de memoria. **Solo agregar** con trigger: nunca se modifica, y solo borra el job de retención (`cargo run -p cfds-gateway --example ops_events_retention`, `[ops] retention_days`), que lo pide con `SET LOCAL cfds.ops_events_retention = 'on'`.
- **Solo modo paper:** ninguno de estos mensajes puede describir una orden real (CLAUDE.md §5, fase 11).
- **Una sola lista de motivos de rechazo**, compartida por `order_report.v1` y `ticket_verdict.v1`: los del módulo de riesgo más `no_price`, `unknown_instrument`, `stop_wrong_side` y `no_recent_candles` (#361: no alcanzan las velas cerradas recientes para el ATR, por ejemplo porque `ig_live` está en pausa sin cupo; antes salía como `invalid_input`) y `missing_fx_rate` (ADR 0013, #418: contrato en otra moneda que la cuenta sin tipo de cambio publicado a la hora de la señal; nunca se inventa uno). `group_risk_limit` (#670) es del riesgo: un grupo correlacionado del instrumento (`[correlation]` de `config/risk.toml`) ya llegó a su tope de riesgo abierto; apagado mientras no haya grupos. Un test comprueba que las dos listas sean iguales.
- **Sin ejecuciones parciales** en la simulación: la spec §12 las prevé solo para el registro real.
- Los escribe el motor (`cfds-broker`) y los lee la UI (terminal, #275), que hoy los tiene como mocks con `TODO(#275)`.
- **Guardado** (`infra/migrations/0028_paper_orders.sql`, solo agregar con trigger): `paper_orders` (cada `order.v1`, una vez por `order_id`, en orden de llegada, solo `mode = paper`) y `paper_order_reports` (su `order_report.v1`, uno por orden). Otro contenido para un `order_id` ya guardado es repintado: error. Con este registro el broker se reconstruye al arrancar y se reconcilia cada `[reconcile] every_minutes`. Las órdenes pendientes de su vela del fill (#479, `infra/migrations/0044_paper_pending_orders.sql`, solo agregar) se guardan como `order.v1` en `paper_pending_orders`, con su final en `paper_pending_outcomes` (`settled` al entrar al registro, en la misma transacción, o `dropped` con el motivo); las lee y escribe solo Rust, así que no hay contrato nuevo: la UI sigue viendo solo `pending` en `order_report.v1`.

### Análogos históricos (#148, #294)

| Contrato | Tabla | Contenido |
|---|---|---|
| `analog_report.v1` | `analog_reports` | El informe de análogos de la última ventana de un índice: fuente (`ig` o `futures`), día y cierre de la ventana, contexto (volatilidad y tasas), vecinos (índice, día y distancia) y, para "todos" y "mismo contexto", cuántos son, si son insuficientes, los percentiles del retorno por horizonte (fracciones), el % positivo y los toques de +R/−R. |

- **Solo informativo:** es la distribución de lo que pasó después de situaciones parecidas, **nunca un pronóstico ni una recomendación**. Con menos de `min_neighbors` vecinos, `insufficient = true` y se muestra "datos insuficientes".
- Lo calcula y lo escribe **solo Python** (`python -m cfds_intel.analogs --guardar`, migración 0029, solo agregar). Un informe igual al último del mismo día no se repite. Rust (gateway) y la UI lo leen sin recalcular.

### Evaluación de la Fase 5 (#298)

| Contrato | Tabla | Contenido |
|---|---|---|
| `walk_forward_report.v1` | `walk_forward_reports` | La evaluación de un instrumento: cada ventana walk-forward de `config/data_split.toml` (entrenamiento y prueba con los parámetros de config, cada tramo con sus métricas y comprar y mantener con costos y financiación), el resumen de las pruebas, la sensibilidad de los parámetros de `config/evaluation.toml` (solo con el entrenamiento) y su estabilidad, y el tramo **fuera de muestra aparte**. Fuente de las velas (`ig` o `futures`, con el instrumento derivado) y `config_sha256`. |

- **No elige nada:** la sensibilidad solo mide cómo cambian las métricas alrededor de los valores de config. Cambiar la estrategia es del dueño.
- El fuera de muestra se evalúa una sola vez, con los parámetros de config: la investigación (entrenamiento, prueba y sensibilidad) nunca lo toca (`research_period`).
- Decimales como texto y `null` cuando algo no está definido (un tramo sin velas no tiene métricas). Retornos y métricas en la moneda de la cuenta; los montos de comprar y mantener, en la del instrumento.
- **UI** (#316): el gateway lo sirve con `GET /api/v1/walk_forward?instrument=<id>` (el último, o `null`) y `GET /api/v1/walk_forward/{report_id}`. El bloque "Walk-forward / fuera de muestra" del detalle de una corrida (#304, `data-walk-forward`) muestra el último reporte del mismo `instrument_id` y `candle_source` (índice `(instrument_id, computed_at)`) tal cual llega: cada tramo trae sus métricas y comprar y mantener, sin nada que calcular en la UI.
- Lo escribe **solo Rust** (`PgJournal::record_walk_forward`, migración 0031, solo agregar con trigger). El mismo reporte (salvo `computed_at`) no guarda nada; con otro contenido es un error.

### Comparación paper vs backtest (fase 8, #315)

| Contrato | Contenido |
|---|---|
| `paper_comparison.v1` | Paper trading (registro del broker simulado) contra el backtest del mismo período, sobre las mismas velas cerradas y la misma config. Las operaciones se emparejan por la vela de la decisión. Trae las emparejadas (entrada, salida, tamaño, comisión, swap y P/L de cada lado, y su diferencia), las que faltan en paper, las que sobran, los rechazos de paper sin operación del backtest, y la diferencia total repartida por causa. |

- **Causas:** precio de ejecución, tamaño, comisión, swap, cierre de fin de semana (el broker simulado no lo hace, #278), rechazo del riesgo, y lo que falta o sobra en paper sin un rechazo que lo explique. **La suma de las causas es exactamente la diferencia**: montos redondeados a 8 decimales, en la moneda del instrumento.
- Es el criterio de aprobación de la fase 8 ("la diferencia está explicada"). Lo calcula solo Rust (`cfds_runner::compare`).
- **Guardado** (`infra/migrations/0032_paper_comparisons.sql`, solo agregar con trigger): `paper_comparisons`, con el documento entero y las columnas de búsqueda (instrumento y período). Lo escribe solo Rust (`PgJournal::record_paper_comparison`); la misma comparación otra vez no guarda nada y con otro contenido es un error. Se corre con el ejemplo `paper_vs_backtest` de `cfds-gateway` (`--guardar`).

### Aprendizaje continuo (fase 8b, #327)

| Contrato | Contenido |
|---|---|
| `learning_report.v1` | El informe periódico de la spec §15b, armado solo con el diario (`proposal.v1` + `outcome.v1`) de un período. Trae la **calibración** (tabla de confiabilidad por rango de probabilidad mostrada §13: casos, aciertos, promedio mostrado, frecuencia real e intervalo de Wilson, y "datos insuficientes" con menos de `min_cases`), una **muestra** por rango para verificarla a mano, el **aporte de cada señal** (R medio y % de TP1 con la señal y sin ella, con las muestras chicas marcadas) y las **propuestas de cambio** en texto, cada una con su evidencia. |
| `change_proposal.v1` | Una propuesta de cambio de un informe, guardada como nació (`pendiente_de_voto`), con su evidencia. |
| `change_vote.v1` | El voto de un miembro sobre una propuesta: a favor o en contra, y cuándo. Uno por miembro y propuesta. |

- **El sistema nunca se cambia solo:** cada propuesta nace `pendiente_de_voto` (spec §1.2: cambios de reglas o parámetros, 4 de 6 en 48 h). El informe solo lee el diario; nada aplica un cambio.
- **Votos** (spec §1.2, `config/votes.toml`): cambios de reglas o parámetros, 4 de 6 a favor dentro de 48 h; si no se llega en el plazo, o ya no se puede llegar, queda rechazada. El estado se calcula con los votos (`cfds_journal::votes::tally`); **contar votos no aplica nada**: el cambio se hace a mano en `config/` con el voto registrado.
- **Guardado** (`infra/migrations/0034_learning_votes.sql`, solo agregar con triggers): `learning_reports`, `change_proposals` (siempre `pendiente_de_voto`, lo impone un CHECK) y `change_votes` (único por miembro y propuesta). Lo escribe Rust (`PgJournal::record_learning_report` y `record_vote`). Se arma con el ejemplo `learning_report` de `cfds-journal`.
- La probabilidad mostrada es la guardada en la propuesta (`proposal.v1` `shown`, #333) cuando existe; si no (backtest), se recalcula con la tabla de entrenamiento (`cfds_journal::probability`). Umbrales en `config/learning.toml` y `config/probability.toml`.
- **UI (#331):** el gateway sirve la última `paper_comparison.v1` de cada instrumento (`GET /api/v1/paper_comparisons`), una por id (`/api/v1/paper_comparisons/{id}`), el último `learning_report.v1` guardado (`GET /api/v1/learning_report`, `PgJournal::latest_learning_report`; `null` si no hay ninguno) y el estado de cada una de sus propuestas (`GET /api/v1/learning_report/votes`, `change_vote_status.v1[]`). Performance los muestra tal cual. Votar no se hace desde la UI (cuentas y 2FA, decisión aparte).

| Contrato | Contenido |
|---|---|
| `change_vote_status.v1` | (respuesta del gateway) El estado de una propuesta de cambio en un momento: `change_id`, `status` (`pendiente_de_voto`, `aprobada` o `rechazada`) y los votos válidos `in_favor` y `against`. Lo cuenta el motor (`cfds_journal::votes::tally` con `config/votes.toml`, vía `PgJournal::change_statuses`). Solo lectura: contar votos nunca aplica un cambio. |

### Fondo del grupo (#179)

| Contrato | Tabla | Contenido |
|---|---|---|
| `fund_movement.v1` | `fund_movements` | Un aporte o retiro ya liquidado (spec §14): `id` (`fund:<fecha>:<orden>`), `settled_at`, `member_id`, `kind` (`contribution` o `withdrawal`), `amount` (en céntimos), `units`, `nav` y `currency`, con los decimales como string. |
| `fund_summary.v1` | (respuesta del gateway) | `GET /api/v1/fund` (#312): `currency`, `basis` (`last_settlement`: valuado al cerrar la última liquidación), `valued_at` (o `null`), `nav`, `total_value`, `total_units` y por miembro `units`, `value`, `net_contributions` y `pnl` (estos dos con signo). Lo calcula `cfds_fund::summary` desde el historial; la suma de `value` es `total_value`. |

- Tabla de **solo agregar**: un trigger impide `UPDATE` y `DELETE`. Guardar el mismo movimiento otra vez no duplica nada, y otro contenido con el mismo id es un error.
- El estado de cada miembro (unidades, aportes y retiros) **no se guarda aparte**: se reconstruye sumando el historial (`cfds_fund::Fund::replay`).
- Lo escribe el motor (`cfds-fund`, `PgFund`). Lo leen la UI (vista Fondo) y el aprendizaje.
