---
modulo: ingest
ruta: `engine/crates/ingest` (crate `cfds-ingest`)
lenguaje: Rust
estado: hecho
resumen: Único camino de entrada de velas (proveedor → cerradas → validación → PostgreSQL → Redis) y cálculos guardados de las diarias.
---

# ingest

## Responsabilidad (única)

- Único camino de entrada de velas: proveedor → solo cerradas → validación → PostgreSQL → Redis
- huecos de la base (`gaps`: qué velas cerradas esperadas faltan, para pedir solo eso)
- indicadores de las diarias guardadas (`daily_indicators`, #148: solo días con sesión, a `daily_indicators`; y la caída desde el máximo, `daily_drawdowns`, #213; y el tercio de la volatilidad, `daily_vol_terciles`, #218)
- mapa de liquidez desde las velas de 15m guardadas (ejemplo `liquidity_levels`, #231: calcula con `cfds-liquidity` y guarda en `liquidity_levels`; desde #504 también los números redondos de `[round_numbers]`)
- verificación del histórico de futuros de Databento contra las diarias de IG (ejemplo `databento_verify`, #248)
- velas de 15m y 1h de futuros desde lo bajado, a `futures_candles` (ejemplo `futures_candles`, #259)
- diarias de los futuros a `daily_candles` bajo el instrumento derivado (ejemplo `futures_daily`, #282), y sus indicadores con `daily_indicators --fuente futuros`
- estadísticas de liquidez desde las velas de 15m, 1h y diarias guardadas (ejemplo `liquidity_stats`, #231: eventos de FVG, gaps de apertura, desequilibrio y agregados)

## Estado

- hecho (#28), huecos (#137), caída desde el máximo (#213), tercio de la volatilidad (#218), mapa de liquidez (#231)
