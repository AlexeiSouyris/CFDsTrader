---
modulo: academy
ruta: `engine/crates/academy` (crate `cfds-academy`)
lenguaje: Rust
estado: en curso (#718, #740, #721, #722)
resumen: Academia: catálogo de lecciones y herramientas con candado, corrección de exámenes y progreso desde eventos de solo agregar.
---

# academy

## Responsabilidad (única)

- Catálogo de la Academia (`config/academy.toml`, validado al arrancar): niveles, lecciones, exámenes y la herramienta de la Terminal que desbloquea cada lección. Su vista pública es `academy_catalog.v1`, **sin las claves** de las respuestas; `catalog_version` es el SHA-256 de la config (`catalog.rs`).
- Corrección de exámenes de preguntas (`quiz_answer.v1` → `quiz_result.v1`, función pura `grade_quiz`): una opción o un número con su tolerancia; una pregunta sin responder cuenta como incorrecta; la nota se redondea hacia abajo a 2 decimales (`quiz.rs`).
- Eventos de progreso de **solo agregar** (`academy_event.v1`, tabla `academy_events`, migración 0052, `pg.rs`, `PgAcademy`): intento de examen con la corrección del motor, lección sin examen completada y desbloqueo o rebloqueo a mano (una herramienta o `*`), con quién y cuándo.
- Estado calculado por el motor (`academy_progress.v1`, función pura `progress`): lee los eventos en orden de `seq`; una lección se aprueba con un intento aprobado (o, sin examen, al completarla); una herramienta queda abierta si se aprobó su lección o si el último desbloqueo a mano que la alcanza es `unlock` (`relock` vuelve a lo que digan las lecciones). El pedido de la UI (`$defs/request`) se valida contra el catálogo (`EventRequest::body`).
- Sesiones de replay de los exámenes (`replay.rs`, `[replay]` de `config/academy.toml`): instrumento, timeframe y fecha **al azar con una semilla** (`SplitMix64` de la investigación; misma semilla y mismos datos, misma sesión), **solo dentro del período de investigación** (el fuera de muestra de `config/data_split.toml` se calcula con `cfds_market_data::plan` y cada ventana pasa por la guardia `research_period` de `cfds-backtest`); velas sin fecha real (número, día de la sesión, hora UTC: `replay_session.v1`, `replay_step.v1`); el **cursor vive en el motor** y `bars_until` rechaza cualquier vela posterior; marcas y entradas con el cursor del momento (`replay_mark.v1`, `replay_order.v1`); todas las velas solo con la sesión terminada. Las velas salen de un `BarStore` (en producción, el gateway).
- Calificación de una sesión terminada (`grade.rs`, `exam_result.v1`, función pura): **proceso** por entrada con lo que se sabía al decidir en `t` (stop presente, del lado correcto y entre `min_atr` y `max_atr` × ATR(t) de `config/risk.toml`, riesgo ≤ el del ticket, plan escrito, ninguna otra abierta) y por sesión (operaciones mínimas, precisión de niveles ≥ el umbral de preguntas); **niveles** marcados contra los soportes y resistencias del motor (`cfds_patterns`, pivotes confirmados) con las velas **hasta el cursor final** y tolerancia `level_tolerance_atr` × ATR; **resultado en R** que se muestra y no aprueba: entrada en la apertura de `t+1`, salida con `simulate_leg` del backtest (stop, gap, objetivo o fin), medio spread y slippage con `fill_price` y financiación por noche = tasa administrativa del CFD (con velas del futuro la referencia ya la descuenta la base, ADR 0015). Solo instrumentos sin comisión por ejecución.
- Umbrales educativos (no de trading) en `[thresholds]`, **provisorios** hasta que el dueño los apruebe (`provisional = true`).

No calcula nada de trading ni toca el riesgo, el paper o las órdenes: el desbloqueo solo cambia qué se ve en la UI. La UI no corrige ni decide si algo se aprobó (CLAUDE.md §1).

## Estado

- Diseño: `docs/design/2026-11-academia.md` (#709). Contratos: #717. Base del motor (catálogo y corrección): #718. Eventos y progreso: #740. Replay: contratos #720, motor #721, calificación #722.
- Rutas del gateway: #719. Sigue: las rutas del replay (#723).

## Interfaces

- Config: `config/academy.toml`.
- Contratos: `academy_catalog.v1`, `quiz_answer.v1`, `quiz_result.v1`, `academy_event.v1`, `academy_progress.v1`, `replay_session.v1`, `replay_step.v1`, `replay_mark.v1`, `replay_order.v1`, `exam_result.v1`.
- Usa: `cfds-backtest` (`SplitMix64`, `research_period`, `simulate_leg`, `fill_price`, `funding`, `funding_nights`), `cfds-patterns` (pivotes y niveles), `cfds-indicators` (`wilder_atr`), `cfds-market-data` (`plan`, `SplitConfig`), `cfds-core` (`Candle`).
- PostgreSQL: `academy_events` (`infra/migrations/0052_academy_events.sql`).
