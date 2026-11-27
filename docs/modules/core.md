---
modulo: core
ruta: `engine/crates/core` (crate `cfds-core`)
lenguaje: Rust
estado: hecho
resumen: Tipos compartidos del motor (Instrument, Candle, Order, Position, Money en decimal) y los tipos de los contratos que lee.
---

# core

## Responsabilidad (única)

- Tipos compartidos: Instrument, Candle, Tick, Order, Position, Money (decimal)
- lectura de `news_interpretation.v1` (`NewsInterpretation`, #130)
- nivel del mapa de liquidez `liquidity_level.v1` (`LiquidityLevel`, #231)
- tipos de `fvg_event.v1`, `opening_gap.v1`, `liquidity_stats.v1` e `imbalance.v1` (#231)
- mensajes del broker `order.v1`, `order_report.v1`, `ticket_verdict.v1`, `position.v1` y `paper_account.v1` (`broker`, solo modo paper, #278)

## Estado

- Instrument, Candle, Timeframe, Money, Currency (#4)
- mensajes del broker (#278)
- lectura de `sentiment.v1` y `analog_report.v1` (`market_context`, #294)
