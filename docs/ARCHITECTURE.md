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

> **Antes de crear algo, buscalo acá.** Cada módulo tiene su ficha en [`docs/modules/`](modules/README.md), con la responsabilidad y el estado en detalle.
> Si tu PR crea un módulo o cambia su responsabilidad, creá o actualizá su ficha en el mismo PR y regenerá esta tabla con `python3 tools/docs_index.py` (el CI falla si no está al día).
> Estados: `planeado` → `en curso (#issue)` → `hecho`.

<!-- registro-de-modulos:inicio (generado por `python3 tools/docs_index.py`; no editar a mano) -->
| Módulo | Ruta | Lenguaje | Responsabilidad (única) | Estado |
|---|---|---|---|---|
| [ci](modules/ci.md) | `.github/workflows/ci.yml` | YAML | Checks de Rust, Python y TypeScript en cada PR. | hecho |
| [contracts](modules/contracts.md) | `contracts/` | JSON Schema | Mensajes entre capas en JSON Schema (índice en `contracts/README.md`). | hecho |
| [glosario](modules/glosario.md) | `docs/GLOSARIO.md` | Markdown | Única fuente de las definiciones de términos (la UI las reutiliza como tooltips). | hecho |
| [auth](modules/auth.md) | `engine/crates/auth` | Rust | Cuentas de los miembros, 2FA (TOTP), roles, sesiones y registro de acciones. | planeado |
| [backtest](modules/backtest.md) | `engine/crates/backtest` (crate `cfds-backtest`) | Rust | Simulación histórica y paper trading en tiempo real, P/L, costos, métricas y walk-forward. | hecho |
| [broker](modules/broker.md) | `engine/crates/broker` (crate `cfds-broker`) | Rust | Interfaz de broker con envío idempotente; hoy el broker simulado (`PaperBroker`) con reconciliación y swap. | hecho |
| [core](modules/core.md) | `engine/crates/core` (crate `cfds-core`) | Rust | Tipos compartidos del motor (Instrument, Candle, Order, Position, Money en decimal) y los tipos de los contratos que lee. | hecho |
| [fund](modules/fund.md) | `engine/crates/fund` (crate `cfds-fund`) | Rust | Fondo del grupo: aportes y retiros, unidades, valor de la unidad (NAV) y P&L por miembro. | hecho |
| [gateway](modules/gateway.md) | `engine/crates/gateway` (crate `cfds-gateway`) | Rust | API REST + WebSocket de solo lectura para la UI y la app del celular. | hecho |
| [ibkr](modules/ibkr.md) | `engine/crates/ibkr` (crate `cfds-ibkr`) | Rust | IBKR como segundo broker, solo cuenta paper y solo lectura (ADR 0017): conexión, cuenta, velas e histórico. | en curso (#534) |
| [ig](modules/ig.md) | `engine/crates/ig` (crate `cfds-ig`) | Rust | Proveedor de IG (cuenta demo, solo lectura): histórico y velas en vivo por REST, condiciones, spread y cupo. | hecho |
| [indicators](modules/indicators.md) | `engine/crates/indicators` | Rust | Indicadores técnicos (SMA, EMA, RSI, ATR, VWAP...), tendencia de 1h y medidas de sentimiento desde precios diarios. | hecho |
| [ingest](modules/ingest.md) | `engine/crates/ingest` (crate `cfds-ingest`) | Rust | Único camino de entrada de velas (proveedor → cerradas → validación → PostgreSQL → Redis) y cálculos guardados de las diarias. | hecho |
| [journal](modules/journal.md) | `engine/crates/journal` (crate `cfds-journal`) | Rust | Diario de propuestas y resultados, calibración de probabilidades y aporte de cada señal. | hecho |
| [liquidity](modules/liquidity.md) | `engine/crates/liquidity` | Rust | Marco ICT (sweep, FVG, BOS/CHoCH, order blocks) y mapa de liquidez informativo de la spec §5.4.1. | hecho |
| [market-data](modules/market-data.md) | `engine/crates/market-data` | Rust | Validación de velas, errores de datos, filtros de spread y de noticias, calendario de mercado y catálogo de instrumentos. | hecho |
| [notifier](modules/notifier.md) | `engine/crates/notifier` | Rust | Notificaciones push al celular (propuestas, alertas, votaciones). | planeado |
| [obs](modules/obs.md) | `engine/crates/obs` (crate `cfds-obs`) | Rust | Observabilidad: logs estructurados en JSON sin secretos y alertas (fase 9). | hecho |
| [patterns](modules/patterns.md) | `engine/crates/patterns` | Rust | Patrones de velas y chartistas, y herramientas de análisis que no entran al puntaje (Fibonacci y otras). | hecho |
| [regime](modules/regime.md) | `engine/crates/regime` (crate `cfds-regime`) | Rust | Régimen de mercado por instrumento y estados emocionales que salen del precio (spec §9.1). | en curso |
| [risk](modules/risk.md) | `engine/crates/risk` (crate `cfds-risk`) | Rust | Apalancamiento por instrumento, tamaño de posición, margen, stop-loss y pérdida máxima. | hecho |
| [runner](modules/runner.md) | `engine/crates/runner` (crate `cfds-runner`) | Rust | Runner de paper trading en tiempo real sobre velas cerradas ya guardadas (fase 8). | hecho |
| [strategy](modules/strategy.md) | `engine/crates/strategy` (crate `cfds-strategy`) | Rust | Combina features y señales de ML/noticias en una señal con probabilidad (puntaje de confluencia y stop estructural). | hecho |
| [terminal](modules/terminal.md) | `engine/crates/terminal` (crate `cfds-terminal`, binario `bolsa`) | Rust | Ver la bolsa desde la terminal (binario `bolsa`), solo lectura y solo presentación. | en curso (#541) |
| [tiingo](modules/tiingo.md) | `engine/crates/tiingo` (crate `cfds-tiingo`) | Rust | Verificación transitoria de las velas de US500, US100 y US30 contra Tiingo (ADR 0009). | hecho |
| [storage](modules/storage.md) | `infra/` (SQL) + `engine/crates/storage` (crate `cfds-storage`) | SQL + Rust | Esquema y migraciones de PostgreSQL, escritura idempotente y publicación de velas cerradas en Redis. | hecho |
| [intelligence](modules/intelligence.md) | `intelligence/` (paquete `cfds_intel`) | Python | Proyecto base de la capa de inteligencia y única salida HTTPS (`cfds_intel/net.py`). | hecho |
| [analogs](modules/analogs.md) | `intelligence/cfds_intel/analogs` | Python | Búsqueda de situaciones históricas similares (análogos v1 sobre velas diarias). | hecho |
| [calendar](modules/calendar.md) | `intelligence/cfds_intel/calendar` | Python | Calendario económico de Forex Factory, solo por su feed JSON oficial (ADR 0010), con su forecast y previous (ADR 0018). | hecho |
| [calendar.actuals](modules/calendar.actuals.md) | `intelligence/cfds_intel/calendar/actuals` | Python | Dato real oficial del CPI y del empleo por la API pública v1 del BLS (ADR 0018 §2). | hecho |
| [calendar.official](modules/calendar.official.md) | `intelligence/cfds_intel/calendar/official` | Python | Calendario histórico oficial (ADR 0016): comunicados del FOMC y publicaciones del BLS. | hecho |
| [databento](modules/databento.md) | `intelligence/cfds_intel/databento` | Python | Importador del histórico de 1 minuto de futuros desde Databento (ADR 0011). | hecho |
| [fomc_novelty](modules/fomc_novelty.md) | `intelligence/cfds_intel/fomc_novelty` | Python | Novedad de cada comunicado del FOMC respecto del anterior (diff de palabras y embeddings locales), descriptiva. | en curso (#668) |
| [learning](modules/learning.md) | `intelligence/cfds_intel/learning` | Python | Reentrenamiento walk-forward de los modelos con el diario e informe de propuestas de cambio. | planeado (#13) |
| [models](modules/models.md) | `intelligence/cfds_intel/models` | Python | Modelos PyTorch de predicción y su entrenamiento (ML v1, solo informativo). | en curso (#399) |
| [news](modules/news.md) | `intelligence/cfds_intel/news` | Python | Ingesta de noticias en tiempo real, resumen e interpretación (positivo/negativo, impacto). | hecho |
| [phase4b](modules/phase4b.md) | `intelligence/cfds_intel/phase4b` | Python | Reporte de aprobación de la fase 4b: cada capa contra no usarla, sin fuga entre períodos. | en curso (#221) |
| [rates](modules/rates.md) | `intelligence/cfds_intel/rates` | Python | Tipos de referencia diarios de la financiación nocturna (€STR, EFFR, SOFR) y tipo de cambio EUR/USD. | hecho |
| [sentiment](modules/sentiment.md) | `intelligence/cfds_intel/sentiment` | Python | Fuentes de sentimiento con su hora de publicación real (ADR 0004): COT e índice de sentimiento. | hecho |
| [treasury](modules/treasury.md) | `intelligence/cfds_intel/treasury` | Python | Curva par diaria del Tesoro de EE. UU. (ADR 0012) con su hora de publicación sin mirar el futuro. | hecho |
| [universe](modules/universe.md) | `intelligence/cfds_intel/universe.py` | Python | Componentes del NASDAQ 100 en cada fecha, sin sesgo de supervivencia (ADR 0005). | hecho |
| [vix](modules/vix.md) | `intelligence/cfds_intel/vix` | Python | VIX diario desde el CSV oficial de Cboe (ADR 0012 §2). | hecho |
| [research-checks](modules/research-checks.md) | `tools/research/` | Python (solo biblioteca estándar) | Chequeos de CI de las reglas de investigación: el último `dsr_trials` cubre las pruebas del registro de resultados. | en curso (#660) |
| [verify](modules/verify.md) | `tools/verify/` | Python, HTML | Herramientas de verificación (ROADMAP 3b) que leen `chart_export.v1` y comparan con TA-Lib. | hecho |
| [ui](modules/ui.md) | `ui/` | TypeScript (React + Tailwind) | Interfaz: gráficos, noticias, estrategias, P/L; hoy prototipo integrado, la app real es la fase 10. | en curso |
<!-- registro-de-modulos:fin -->
