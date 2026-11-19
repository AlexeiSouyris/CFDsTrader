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

## Checklist de trading (si toca datos, indicadores, estrategia, riesgo o ejecución)

Respondé cada punto (✅ / ❌ + explicación / N/A):

- [ ] ¿Hay algún uso de datos del futuro (look-ahead)?
- [ ] ¿Algún indicador o patrón usa la vela en curso o un pivote todavía no confirmado?
- [ ] ¿El timeframe mayor usa solo velas cerradas?
- [ ] ¿Los indicadores coinciden con una plataforma de referencia (por ejemplo TradingView)?
- [ ] ¿Se respeta el período de calentamiento?
- [ ] ¿Están incluidos comisiones, spread, slippage y swap overnight?
- [ ] ¿Las noticias se usan solo desde su hora de publicación?
- [ ] ¿Los tests cubren casos borde y datos corruptos?
- [ ] ¿Algún parámetro quedó hardcodeado?
- [ ] ¿Qué pasa si se cae la conexión en medio de una orden?
- [ ] ¿Los límites de riesgo se pueden saltear desde la estrategia?
- [ ] ¿Hay alguna credencial expuesta (código o logs)?

## Supuestos y riesgos

<!-- Qué se supuso y qué podría salir mal. -->

## Cómo probarlo

<!-- Comandos o pasos para verificar. -->

## Hecho por

- [ ] Humano
- [ ] Agente de IA (indicar cuál):
