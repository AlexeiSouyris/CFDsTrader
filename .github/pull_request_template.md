## Issue

Closes #

## Qué cambia

<!-- 2-5 líneas: qué hace este PR y por qué. -->

## Área

- [ ] engine (Rust)
- [ ] intelligence (Python)
- [ ] infra (PostgreSQL / Redis)
- [ ] contracts
- [ ] ui (TypeScript)
- [ ] docs

## Anti-duplicados (obligatorio)

- [ ] Busqué en `main`, Issues y PRs abiertos: no existe otra implementación de esto.
- [ ] El cálculo vive en un solo lenguaje (no dupliqué lógica entre Rust, Python y UI).
- [ ] Si creé o cambié un módulo, actualicé el *Registro de módulos* en `docs/ARCHITECTURE.md`.
- [ ] Si agregué o cambié mensajes entre lenguajes, actualicé `contracts/`.

## Calidad y riesgo

- [ ] Tests agregados o actualizados y en verde.
- [ ] Linters y formato en verde.
- [ ] Dinero con decimales exactos y timestamps en UTC.
- [ ] No hay secretos en el código y `.env.example` está actualizado si hace falta.
- [ ] No habilita órdenes reales (o hay un ADR aprobado).

## Cómo probarlo

<!-- Comandos o pasos para verificar. -->

## Hecho por

- [ ] Humano
- [ ] Agente de IA (indicar cuál):
