# 0001 – Arquitectura híbrida: Rust + Python/PyTorch + PostgreSQL/Redis + UI en TypeScript

- **Estado:** aceptada
- **Fecha:** 2026-11-19
- **Aprobada por:** pendiente de confirmar por ambos colaboradores

## Contexto

Queremos un sistema que analice gráficos y noticias en tiempo real, detecte patrones (velas, doble techo, liquidez), compare con situaciones históricas de otras empresas e índices, y sugiera y simule operaciones con CFDs, con distinto apalancamiento por instrumento y su P/L.

## Decisión

| Capa | Tecnología | Para qué |
|---|---|---|
| Motor principal | **Rust** | Estrategias, cálculos (indicadores, patrones, liquidez), riesgo, backtesting, ejecución simulada, API para la UI |
| Inteligencia | **Python / PyTorch** | Machine learning, deep learning, predicciones, análisis de noticias, análogos históricos |
| Infraestructura | **PostgreSQL + Redis** | Datos históricos (Postgres); tiempo real, caché y eventos (Redis Streams) |
| Interfaz | **TypeScript** | UI web (fase 2) |

Reglas derivadas (ver `CLAUDE.md`):
- Cada cálculo vive en un único lenguaje.
- La comunicación entre capas va por Redis o PostgreSQL usando contratos versionados en `contracts/`.
- Modo por defecto: simulación/paper, sin órdenes reales.

## Alternativas consideradas

- Todo en Python: más rápido de arrancar, pero menos rendimiento y menos garantías en el motor de riesgo y ejecución.
- Todo en TypeScript: un solo lenguaje, pero un ecosistema de ML y cálculo cuantitativo más pobre.

## Consecuencias

- Hay que mantener dos toolchains (cargo y uv), además de Node para la UI.
- Los contratos entre capas son obligatorios. Hacen más lento el arranque, pero evitan duplicados e incompatibilidades.
- Proveedor de datos y broker: **pendiente** (ADR futuro). El motor usa un trait `MarketDataProvider` para poder cambiarlo.
