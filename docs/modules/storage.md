---
modulo: storage
ruta: `infra/` (SQL) + `engine/crates/storage` (crate `cfds-storage`)
lenguaje: SQL + Rust
estado: hecho
resumen: Esquema y migraciones de PostgreSQL, escritura idempotente y publicación de velas cerradas en Redis.
---

# storage

## Responsabilidad (única)

- Esquema y migraciones de PostgreSQL
- escritura idempotente de velas
- publicación de velas cerradas en Redis Streams
- lectura de los cierres publicados del VIX (`vix_closes`, #491)
- lectura de las interpretaciones de noticias del stream `news_interpretations.v1` sin perder ni repetir entre reinicios (`NewsInterpretationConsumer`, #197)
- docker-compose
- `backtest_run_curves`, curva de cada corrida de solo agregar que escribe `cfds-journal` (migración 0030, #299)
- guardia de los tests de integración (`test_db::test_database_url`, #376: solo corren contra una base `cfds_test…`, nunca la real)
- cargas de diarias e indicadores diarios que no pisan ni frenan ante una fila ya guardada con otro valor y la reportan (`insert_daily_keeping` y sus pares de indicadores, #409, para extender la historia de los futuros hacia atrás)
- en esas cargas de indicadores, un valor guardado en NULL por falta de historia se completa una sola vez (NULL → valor) y queda en `daily_null_fills`, de solo agregar
- todo otro UPDATE o DELETE sigue fallando (migración 0040, #417)

## Estado

- docker-compose (#3)
- esquema, PgStore y CandlePublisher (#6)
- `sentiment_observations` de solo agregar (#27)
- `candle_spreads`, spread de cierre por vela (#95)
- `daily_candles`, velas diarias (#103)
- `news_items`, noticias de solo agregar que escribe Python (#111)
- `ig_allowance_log`, registro del cupo de IG de solo agregar (#140)
- `fund_movements`, historial del fondo de solo agregar (#179)
- `tiingo_usage_log` y `tiingo_verifications`, de solo agregar y sin datos de Tiingo (#206)
- `backtest_runs`, corridas de backtest de solo agregar que escribe `cfds-journal` (#200)
- `news_interpretations`, interpretaciones de noticias de solo agregar que escribe Python (#186) y lee el motor por intérprete y rango de `interpreted_at`, con el corte del ADR 0008 §5 para el backtest (`PgStore::news_interpretations`, #194)
- consumidor del stream de interpretaciones (#197)
- `liquidity_levels` y `liquidity_level_events`, mapa de liquidez de solo agregar, con un evento por nivel (barrido, roto o vencido) y lectura tal como se conocía en un instante (`PgStore::insert_liquidity_levels` y `PgStore::liquidity_levels`, #231)
- `fvg_zones`, `fvg_zone_events`, `opening_gaps`, `opening_gap_events`, `imbalances` y `liquidity_stats` (migración 0021), estadísticas de liquidez de solo agregar: cada evento una sola vez, agregados por corrida (`computed_at`) y lecturas tal como se conocían en un instante (`PgStore::insert_fvg_events`, `insert_opening_gaps`, `insert_imbalances`, `insert_liquidity_stats` y sus lecturas, #231)
- `economic_events`, calendario económico de solo agregar que escribe Python (`cfds_intel.calendar`, #239) y lee el motor para el filtro §7.2 (`PgStore::economic_events` y `economic_event_sightings`, #243)
- `official_releases`, calendario histórico oficial de solo agregar que escribe Python (`cfds_intel.calendar.official`, migración 0046, #520) y lee el motor publicado hasta un instante (`PgStore::official_releases`, #526)
- lectura de `futures_bars_1m` (`PgStore::futures_bars`, #248; filtra por fuente, dataset, símbolo e instrumento, #376)
- `ibkr_bars`, velas cerradas de IBKR como fuente de datos (`ibkr_bar.v1`, de solo agregar, por contrato: `insert_ibkr_bars` y `last_ibkr_bar_start`, migración 0048, #536)
- `futures_candles`, velas de 15m y 1h de futuros de solo agregar, aparte de `candles` (`PgStore::insert_futures_candles`, `futures_candles` y `futures_loaded_range`, #259), y la lectura por fuente (`CandleFeed`, `candles_from`, #259)
- `emotional_states` (`emotional_state.v1`, de solo agregar: `insert_emotional_states`, `latest_emotional_states` y `emotional_states`, #220)
- `ops_events` (`ops_event.v1` ya redactados, de solo agregar salvo la retención: `insert_ops_events`, `latest_ops_events` y `purge_ops_events_before`, #337)
- lecturas de `ibkr_bars` para el gateway (`latest_closed_ibkr_bar`, `ibkr_bars_closed_by`, `last_ibkr_bar_starting_before`: solo velas cerradas, de una sola serie, #551; `ibkr_bars_between` e `ibkr_series` para revisar 15m y 1h contra sus 1m, #563)
- `ibkr_bar_corrections`, correcciones de velas de `ibkr_bars` de solo agregar (clave de la vela, OHLC corregido, motivo, fuente y `corrected_at`; `insert_ibkr_bar_corrections`; todas las lecturas de `ibkr_bars` devuelven la corrección más reciente; migración 0049 idempotente, #575)
- `fomc_statement_novelty`, novedad de cada comunicado del FOMC respecto del anterior, de solo agregar, que escribe Python (`cfds_intel.fomc_novelty`, `fomc_statement_novelty.v1`, migración 0050, #668)
- lectura de noticias para la UI (`news_page`, `news_interpretations_of`, #600) y de `official_releases` por rango de hora (`official_releases_between`, con `source_url`, #600)
