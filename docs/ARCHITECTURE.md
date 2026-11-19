# Arquitectura de CFDsTrader

Decisión base: [ADR 0001 – Arquitectura híbrida](decisions/0001-arquitectura-hibrida.md).

```
            ┌──────────────────────────────┐
            │  ui/  (TypeScript)  — fase 2 │
            └──────────────┬───────────────┘
                           │ WebSocket / REST
┌──────────────────────────▼───────────────────────────┐
│  engine/  (Rust) — MOTOR PRINCIPAL                   │
│  datos de mercado · indicadores · patrones de velas  │
│  liquidez · estrategias · riesgo/apalancamiento      │
│  backtesting · ejecución simulada · P/L · gateway    │
└───────▲──────────────────────────────────┬───────────┘
        │ señales ML / noticias            │ velas, features
        │ (Redis Streams)                  │ (Redis + PostgreSQL)
┌───────┴──────────────────────────────────▼───────────┐
│  intelligence/  (Python / PyTorch)                   │
│  ingesta y análisis de noticias · análogos           │
│  históricos · modelos de ML · predicciones           │
└──────────────────────────────────────────────────────┘
        PostgreSQL (histórico)  +  Redis (tiempo real, caché, eventos)
```

Todos los mensajes entre capas están definidos en `contracts/`.

## Registro de módulos

> **Antes de crear algo, buscalo acá.** Si tu PR crea un módulo o cambia su responsabilidad, actualizá esta tabla en el mismo PR.
> Estados: `planeado` → `en curso (#issue)` → `hecho`.

| Módulo | Ruta | Lenguaje | Responsabilidad (única) | Estado |
|---|---|---|---|---|
| core | `engine/crates/core` | Rust | Tipos compartidos: Instrument, Candle, Tick, Order, Position, Money (decimal) | planeado |
| market-data | `engine/crates/market-data` | Rust | Conectores a proveedores/brokers (trait `MarketDataProvider`), normalización a velas y ticks | planeado |
| indicators | `engine/crates/indicators` | Rust | Indicadores técnicos (SMA, EMA, RSI, ATR, VWAP, volumen...) | planeado |
| patterns | `engine/crates/patterns` | Rust | Patrones de velas (martillo, envolvente...) y chartistas (doble techo/suelo, soportes/resistencias) | planeado |
| liquidity | `engine/crates/liquidity` | Rust | Zonas de liquidez, barridos internos/externos, order flow | planeado |
| strategy | `engine/crates/strategy` | Rust | Combina features + señales de ML/noticias → señal con probabilidad | planeado |
| risk | `engine/crates/risk` | Rust | Apalancamiento por instrumento, tamaño de posición, margen, stop-loss, pérdida máxima | planeado |
| fund | `engine/crates/fund` | Rust | Fondo del grupo: aportes y retiros de cada miembro, unidades, valor de la unidad (NAV) y P&L por miembro | planeado |
| backtest | `engine/crates/backtest` | Rust | Simulación histórica y paper trading en tiempo real, P/L | planeado |
| gateway | `engine/crates/gateway` | Rust | API REST + WebSocket para la UI | planeado |
| news | `intelligence/cfds_intel/news` | Python | Ingesta de noticias en tiempo real, resumen e interpretación (positivo/negativo, impacto) | planeado |
| analogs | `intelligence/cfds_intel/analogs` | Python | Búsqueda de situaciones históricas similares en otras empresas e índices | planeado |
| models | `intelligence/cfds_intel/models` | Python | Modelos PyTorch de predicción y su entrenamiento | planeado |
| storage | `infra/` | SQL | Esquema de PostgreSQL, migraciones, docker-compose con Postgres + Redis | planeado |
| contracts | `contracts/` | JSON Schema | Mensajes entre capas (velas, señales, noticias, órdenes) | planeado |
| ui | `ui/` | TypeScript | Interfaz: gráficos, panel de noticias, estrategias, P/L | planeado (fase 2) |
| glosario | `docs/GLOSARIO.md` | Markdown | Única fuente de las definiciones de términos (la UI las reutiliza como tooltips) | hecho |
