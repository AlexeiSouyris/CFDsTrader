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

**Circuito de aprendizaje** ([spec §15b](spec/fase-1-especificacion.md#15b-aprendizaje-continuo)): cada propuesta y su resultado quedan en el **diario** (`journal`). Con el diario se mide la calibración de las probabilidades y el aporte de cada señal (Rust), y se reentrenan los modelos (Python). El resultado es un **informe con propuestas de cambio** que el grupo vota. **El sistema nunca cambia sus propios parámetros.**

```
propuesta ──► resultado ──► diario ──► calibración + aporte por señal (Rust)
                                   └─► reentrenamiento walk-forward (Python)
                                              │
                         informe con propuestas de cambio ──► votación 4 de 6 ──► config/
```

Todos los mensajes entre capas están definidos en `contracts/`.

## Registro de módulos

> **Antes de crear algo, buscalo acá.** Si tu PR crea un módulo o cambia su responsabilidad, actualizá esta tabla en el mismo PR.
> Estados: `planeado` → `en curso (#issue)` → `hecho`.

| Módulo | Ruta | Lenguaje | Responsabilidad (única) | Estado |
|---|---|---|---|---|
| core | `engine/crates/core` (crate `cfds-core`) | Rust | Tipos compartidos: Instrument, Candle, Tick, Order, Position, Money (decimal) | Instrument, Candle, Timeframe, Money, Currency (#4) |
| market-data | `engine/crates/market-data` | Rust | Validación de velas (#5); calendario de sesiones y feriados por bolsa (#9); separación entrenamiento/validación/fuera de muestra (#16); catálogo de instrumentos y costos del broker con su fuente (#7); trait `MarketDataProvider` y proveedor en memoria (#28); proveedor de IG (#8) | validación (#5), calendario (#9), separación (#16), catálogo (#7), trait de proveedores (#28) |
| ingest | `engine/crates/ingest` (crate `cfds-ingest`) | Rust | Único camino de entrada de velas: proveedor → solo cerradas → validación → PostgreSQL → Redis | hecho (#28) |
| indicators | `engine/crates/indicators` | Rust | Indicadores técnicos (SMA, EMA, RSI, ATR, VWAP, volumen...) y tendencia de 1h | ATR de Wilder (#5), SMA, EMA y tendencia (#37) |
| patterns | `engine/crates/patterns` | Rust | Patrones de velas (martillo, envolvente...) y chartistas (doble techo/suelo, soportes/resistencias) | planeado |
| liquidity | `engine/crates/liquidity` | Rust | Zonas de liquidez, barridos internos/externos, order flow | planeado |
| regime | `engine/crates/regime` | Rust | Clasificar el régimen de mercado por instrumento (volatilidad × tendencia) con velas cerradas; elegir el juego de parámetros votado para ese régimen | propuesto ([ADR 0003](decisions/0003-modelo-adaptativo-y-contexto.md), #19) |
| strategy | `engine/crates/strategy` | Rust | Combina features + señales de ML/noticias → señal con probabilidad | planeado |
| risk | `engine/crates/risk` | Rust | Apalancamiento por instrumento, tamaño de posición, margen, stop-loss, pérdida máxima | planeado |
| fund | `engine/crates/fund` | Rust | Fondo del grupo: aportes y retiros de cada miembro, unidades, valor de la unidad (NAV) y P&L por miembro | planeado |
| journal | `engine/crates/journal` | Rust | Diario de propuestas y resultados (simulado y real); calibración de probabilidades y aporte de cada señal | planeado (#13) |
| backtest | `engine/crates/backtest` | Rust | Simulación histórica y paper trading en tiempo real, P/L | planeado |
| gateway | `engine/crates/gateway` | Rust | API REST + WebSocket para la UI y la app del celular | planeado |
| auth | `engine/crates/auth` | Rust | Cuentas de los miembros, 2FA (TOTP), roles (responsable / miembro), sesiones, registro de acciones | planeado |
| notifier | `engine/crates/notifier` | Rust | Notificaciones push al celular (propuestas, alertas, votaciones) | planeado |
| news | `intelligence/cfds_intel/news` | Python | Ingesta de noticias en tiempo real, resumen e interpretación (positivo/negativo, impacto) | planeado |
| analogs | `intelligence/cfds_intel/analogs` | Python | Búsqueda de situaciones históricas similares en otras empresas e índices | planeado |
| universe | `intelligence/cfds_intel/universe.py` | Python | Componentes del NASDAQ 100 en cada fecha, sin sesgo de supervivencia (fuente: nasdaq-100-ticker-history, MIT, ADR 0005) | hecho (#10) |
| models | `intelligence/cfds_intel/models` | Python | Modelos PyTorch de predicción y su entrenamiento | planeado |
| learning | `intelligence/cfds_intel/learning` | Python | Reentrenamiento walk-forward de los modelos con el diario, y el informe de propuestas de cambio para votar | planeado (#13) |
| storage | `infra/` (SQL) + `engine/crates/storage` (crate `cfds-storage`) | SQL + Rust | Esquema y migraciones de PostgreSQL; escritura idempotente de velas; publicación de velas cerradas en Redis Streams; docker-compose | docker-compose (#3); esquema, PgStore y CandlePublisher (#6) |
| intelligence | `intelligence/` (paquete `cfds_intel`) | Python | Proyecto base de la capa de inteligencia | esqueleto (#3) |
| ci | `.github/workflows/ci.yml` | YAML | Checks de Rust y Python en cada PR | hecho (#3) |
| contracts | `contracts/` | JSON Schema | Mensajes entre capas (velas, señales, noticias, órdenes) | candle.v1, instrument.v1 (#4) |
| ui | `ui/` | TypeScript (React + Tailwind) | Interfaz: gráficos, panel de noticias, estrategias, P/L. **Hoy: solo prototipo visual con datos de ejemplo, sin lógica (#30)** | prototipo (#30); app real en fase 10 |
| glosario | `docs/GLOSARIO.md` | Markdown | Única fuente de las definiciones de términos (la UI las reutiliza como tooltips) | hecho |
