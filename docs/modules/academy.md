---
modulo: academy
ruta: `engine/crates/academy` (crate `cfds-academy`)
lenguaje: Rust
estado: en curso (#718, #740)
resumen: Academia: catálogo de lecciones y herramientas con candado, corrección de exámenes y progreso desde eventos de solo agregar.
---

# academy

## Responsabilidad (única)

- Catálogo de la Academia (`config/academy.toml`, validado al arrancar): niveles, lecciones, exámenes y la herramienta de la Terminal que desbloquea cada lección. Su vista pública es `academy_catalog.v1`, **sin las claves** de las respuestas; `catalog_version` es el SHA-256 de la config (`catalog.rs`).
- Corrección de exámenes de preguntas (`quiz_answer.v1` → `quiz_result.v1`, función pura `grade_quiz`): una opción o un número con su tolerancia; una pregunta sin responder cuenta como incorrecta; la nota se redondea hacia abajo a 2 decimales (`quiz.rs`).
- Eventos de progreso de **solo agregar** (`academy_event.v1`, tabla `academy_events`, migración 0052, `pg.rs`, `PgAcademy`): intento de examen con la corrección del motor, lección sin examen completada y desbloqueo o rebloqueo a mano (una herramienta o `*`), con quién y cuándo.
- Estado calculado por el motor (`academy_progress.v1`, función pura `progress`): lee los eventos en orden de `seq`; una lección se aprueba con un intento aprobado (o, sin examen, al completarla); una herramienta queda abierta si se aprobó su lección o si el último desbloqueo a mano que la alcanza es `unlock` (`relock` vuelve a lo que digan las lecciones). El pedido de la UI (`$defs/request`) se valida contra el catálogo (`EventRequest::body`).
- Umbrales educativos (no de trading) en `[thresholds]`, **provisorios** hasta que el dueño los apruebe (`provisional = true`).

No calcula nada de trading ni toca el riesgo, el paper o las órdenes: el desbloqueo solo cambia qué se ve en la UI. La UI no corrige ni decide si algo se aprobó (CLAUDE.md §1).

## Estado

- Diseño: `docs/design/2026-11-academia.md` (#709). Contratos: #717. Base del motor (catálogo y corrección): #718. Eventos y progreso: #740.
- Sigue: rutas del gateway, replay de exámenes y su calificación (plan del diseño, §8).

## Interfaces

- Config: `config/academy.toml`.
- Contratos: `academy_catalog.v1`, `quiz_answer.v1`, `quiz_result.v1`, `academy_event.v1`, `academy_progress.v1`.
- PostgreSQL: `academy_events` (`infra/migrations/0052_academy_events.sql`).
