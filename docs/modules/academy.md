---
modulo: academy
ruta: `engine/crates/academy` (crate `cfds-academy`)
lenguaje: Rust
estado: en curso (#718)
resumen: Academia: catálogo de lecciones y herramientas con candado, corrección de exámenes y progreso desde eventos de solo agregar.
---

# academy

## Responsabilidad (única)

- Catálogo de la Academia (`config/academy.toml`, validado al arrancar): niveles, lecciones, exámenes y la herramienta de la Terminal que desbloquea cada lección. Su vista pública es `academy_catalog.v1`, **sin las claves** de las respuestas; `catalog_version` es el SHA-256 de la config (`catalog.rs`).
- Corrección de exámenes de preguntas (`quiz_answer.v1` → `quiz_result.v1`, función pura `grade_quiz`): una opción o un número con su tolerancia; una pregunta sin responder cuenta como incorrecta; la nota se redondea hacia abajo a 2 decimales (`quiz.rs`).
- Umbrales educativos (no de trading) en `[thresholds]`, **provisorios** hasta que el dueño los apruebe (`provisional = true`).

No calcula nada de trading ni toca el riesgo, el paper o las órdenes: el desbloqueo solo cambia qué se ve en la UI. La UI no corrige ni decide si algo se aprobó (CLAUDE.md §1).

## Estado

- Diseño: `docs/design/2026-11-academia.md` (#709). Contratos: #717. Base del motor (catálogo y corrección): #718.
- Sigue: eventos de solo agregar y progreso, rutas del gateway, replay de exámenes y su calificación (plan del diseño, §8).

## Interfaces

- Config: `config/academy.toml`.
- Contratos: `academy_catalog.v1`, `quiz_answer.v1`, `quiz_result.v1`.
