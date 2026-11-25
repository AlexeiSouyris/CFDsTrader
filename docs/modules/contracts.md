---
modulo: contracts
ruta: `contracts/`
lenguaje: JSON Schema
estado: hecho
resumen: Mensajes entre capas en JSON Schema (índice en `contracts/README.md`).
---

# contracts

## Responsabilidad (única)

- Mensajes entre capas (velas, señales, noticias, órdenes)

## Estado

- candle.v1, instrument.v1 (#4)
- proposal.v1, outcome.v1 (#60)
- sentiment_observation.v1 (#27)
- chart_export.v1 (#81; bloque `analysis` #491)
- candle_spread.v1 (#95)
- economic_event.v1 (#91)
- daily_candle.v1 (#103)
- news_item.v1 (#109)
- news_interpretation.v1 (#122)
- fund_movement.v1 (#179)
- liquidity_level.v1 (#231)
- liquidity_view.v1, gateway_instruments.v1 (#231; bloque opcional `analysis` #501)
- economic_event.v1 guardado en `economic_events` (#239)
- futures_bar.v1 (#247)
- treasury_yield.v1, vix_daily.v1 (#260)
- order.v1, order_report.v1, ticket_verdict.v1, position.v1, paper_account.v1 (#278)
- analog_report.v1 (#294)
- official_release.v1 (#520)
- gateway_ibkr_bars.v1, gateway_ibkr_watchlist.v1 (#551)
- ibkr_live_bar.v1 (#558)
