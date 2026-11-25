---
modulo: gateway
ruta: `engine/crates/gateway` (crate `cfds-gateway`)
lenguaje: Rust
estado: hecho
resumen: API REST + WebSocket de solo lectura para la UI y la app del celular.
---

# gateway

## Responsabilidad (única)

- API REST + WebSocket para la UI y la app del celular. Hoy: servidor HTTP **de solo lectura** (hyper) con `GET /api/v1/liquidity?instrument=<id>&timeframe=<tf>[&source=futures|ibkr|auto][&before=<UTC>]` (15m, 1h o 1d; con `source=futures`, lo calculado con las velas de los futuros, #259; con `source=ibkr`, lo calculado con las barras de IBKR de los CFD con mapeo en `[ibkr.instruments]` de `config/candle_sources.toml` (US100 → MNQ, US500 → MES; `liquidity_levels` y `liquidity_stats` con `--fuente ibkr`, guardado bajo `<id>.IBKR`, contratos empalmados sin repintar por `PgStore::ibkr_candles`), #630; con `source=auto`, IG y, si en la ventana hay menos de `[view.auto] min_candles` velas, la primera fuente de reemplazo con velas (IBKR antes que Databento desde #630), diciendo cuál y por qué en `fallback`, #628; con `before`, la vista tal como se conocía en ese momento, para pedir velas más viejas al desplazar el gráfico, #401) (`contracts/liquidity_view.v1`), que junta lo guardado en PostgreSQL tal como se conocía al cierre de la última vela de ese timeframe, y `GET /api/v1/instruments` (`contracts/gateway_instruments.v1`)
- ventanas, dirección y lo que abre primero en `config/gateway.toml`. Rutas del broker simulado (`cfds_gateway::paper`, #278, solo paper): `POST /api/v1/paper/preview` (`ticket_verdict.v1`) y `POST /api/v1/paper/orders` (`order_report.v1`, idempotente y guardado), con token local anti-CSRF (`[paper] token_env`), `application/json`, límite de body y `created_at` de ahora
- `GET /api/v1/paper/positions`, `GET /api/v1/paper/account` y `GET /api/v1/backtest_runs` (`backtest_run.v1`). Runner de paper trading (`cfds_gateway::paper_runner`, fase 8, #305): con `[runner] enabled` corre `cfds-runner` cada `poll_secs` sobre el mismo broker simulado, con los filtros de entrada del backtest (horario, spread §7.3, noticias de alto impacto §7.2) y el ajuste de noticias §9, y manda cada propuesta con `submit_and_record`
- cada propuesta ejecutada va al diario (`proposal.v1` con la probabilidad mostrada de `[runner] probability_run_id`) y su `outcome.v1` cuando terminó, sin reescribir tras un reinicio los que ya están en el diario (#333, #438; única escritura además del registro del broker, en su propia transacción de escritura)
- nunca llama a IG
- apagado por defecto. Operación segura (`cfds_gateway::ops`, fase 9, #297): logs JSON sin secretos de cada evento del broker y alertas de `cfds-obs`
- las órdenes responden 503 hasta que la reconciliación al arrancar pasa limpia y desde que llega SIGTERM, que guarda y verifica el registro antes de salir. Sesión de PostgreSQL de solo lectura, salvo la transacción del registro del broker (`insert_paper_order`)
- `infra/scripts/ui_liquidez.sh` lo levanta junto con la UI. No calcula nada propio
- `GET /api/v1/backtest_runs/{run_id}` (`backtest_run_detail.v1`, de solo lectura, #299)

## Estado

- vista de liquidez (#231)
- rutas del broker simulado (#278)
- contexto de mercado del Panel (#294: `GET /api/v1/sentiment` con `sentiment.v1` y `GET /api/v1/analogs` con `analog_report.v1`, solo lectura, `[context]`)
- fondo del grupo (#312: `GET /api/v1/fund` con `fund_summary.v1` y `GET /api/v1/fund/movements` con `fund_movement.v1[]`, solo lectura, módulo `fund`)
- walk-forward de la Fase 5 (`GET /api/v1/walk_forward?instrument=<id>`, el último `walk_forward_report.v1` del instrumento o `null`, y `GET /api/v1/walk_forward/{report_id}`, solo lectura del diario, módulo `walk_forward`, #316)
- estado de la operación (#314: `GET /api/v1/ops/status` con `ops_status.v1` y `GET /api/v1/ops/events` con `ops_event.v1[]`, solo lectura, módulo `status`; los eventos son las líneas ya redactadas del `Logger`, en memoria, `[ops] feed_size`, y desde #337 también en la tabla `ops_events`, guardados en segundo plano sin frenar al broker (`ops_store`), con retención en un job aparte, `examples/ops_events_retention.rs`; desde #576 los eventos iguales se devuelven agrupados con `count` y `last_seen`, `[ops] events_limit` grupos por defecto y `?limit=` hasta `feed_size`, y que un cliente cierre la conexión es `info` `client_closed`, fuera del feed)
- ensayo del runner sobre velas guardadas con un reloj simulado (`examples/runner_replay.rs`, #406: el mismo `run_once` del gateway, solo contra bases `cfds_test_*`)
- estados emocionales del Panel (#220: `GET /api/v1/emotions` con `emotional_state.v1[]`, solo lectura, módulo `emotions`)
- fases 8 y 8b (#331: `GET /api/v1/paper_comparisons` y `/api/v1/paper_comparisons/{id}` con `paper_comparison.v1`, y `GET /api/v1/learning_report` con el último `learning_report.v1` o `null`, solo lectura, módulo `learning`; el informe guardado con `PgLearning` y `GET /api/v1/learning_report/votes` con el estado de cada propuesta, `change_vote_status.v1[]`, contado por `cfds_journal::votes::tally`; sin rutas para votar)
- experimentos de ML de la fase 4b (`GET /api/v1/ml/experiments[?instrument=<id>]` con `ml_experiment_entry.v1[]`: el último de cada instrumento o la historia de uno, tal como los guardó Python, solo lectura, módulo `ml`, #426)
- herramientas de análisis en la vista de liquidez (#501: bloque opcional `analysis` de `liquidity_view.v1` en 15m con las velas de IG, armado por `cfds_backtest::analysis` con las diarias y el VIX guardados hasta `as_of`, `ViewAnalysis`; config `config/analysis.toml` y `[pivots]` de `config/patterns.toml`; desde #511 se calcula con `[warmup]` velas de 15m antes de la vista y se devuelve solo lo de la vista (`analysis_view`, sin repintado por el borde), también con `before`, y las diarias y el VIX salen de un caché en memoria por instrumento que se relee solo con un día o un VIX nuevo (`series_cache`))
- WebSocket planeado
- velas de IBKR para la terminal de la app (#551: `GET /api/v1/ibkr/bars?instrument=<id>&timeframe=1m|15m|1h[&limit=<n>][&before=<UTC>]` con `gateway_ibkr_bars.v1`, solo velas cerradas de una sola serie (con el criterio de la carga: la última de la serie solo si cerró hace al menos `[load].data_delay_secs` de `config/ibkr.toml` o ya hay una guardada después, `cfds_ibkr::load::complete_prefix_by`, #575; con las correcciones de `ibkr_bar_corrections`), y `GET /api/v1/ibkr/watchlist` con `gateway_ibkr_watchlist.v1`: último cierre, hora de la vela, atraso y cierre de una sesión anterior con la variación, día de sesión de `cfds_market_data::session_day`; solo lectura de `ibkr_bars`, módulo `ibkr`, `[ibkr]` de `config/gateway.toml`)
- vela en formación de IBKR (#558, solo visual: `GET /api/v1/ibkr/live?instrument=<id>&timeframe=1m|5m|15m|1h` por SSE con `ibkr_live_bar.v1`, como mucho un mensaje por segundo, la cerrada antes que la siguiente; la conexión de streaming de `cfds_ibkr::live_feed` se abre solo con `[ibkr] live = true` y si IB Gateway está cerrado informa "disconnected" sin frenar el gateway; módulo `ibkr_live`)
- noticias y calendario para la vista Noticias (#600): `GET /api/v1/news[?limit][&before][&feed]` (`gateway_news.v1`: `news_items` con sus `news_interpretations`, de la más nueva a la más vieja, sin cortar noticias de la misma hora entre páginas) y `GET /api/v1/calendar[?from][&to]` (`gateway_calendar.v1`: `economic_events` y `official_releases` por hora), solo lectura, nada antes de su `published_at` / `interpreted_at`, límites en `[news]` y `[calendar]` de `config/gateway.toml` (módulo `news`) 
- diario de paper para la vista Diario (#615): `GET /api/v1/journal[?limit]` (`gateway_journal.v1`: las propuestas con `source = paper` y su `outcome.v1` simulado o `null`, de la señal más nueva a la más vieja, tal como las guardó el runner; solo lectura, módulo `journal`)
