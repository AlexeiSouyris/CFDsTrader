---
modulo: liquidity
ruta: `engine/crates/liquidity`
lenguaje: Rust
estado: hecho
resumen: Marco ICT (sweep, FVG, BOS/CHoCH, order blocks) y mapa de liquidez informativo de la spec §5.4.1.
---

# liquidity

## Responsabilidad (única)

- Marco ICT (spec §5.4): sweep de pivotes de 1h, FVG, rupturas de estructura (BOS/CHoCH) y order blocks
- mapa de liquidez informativo de la spec §5.4.1 (`levels`: EQH/EQL con los pivotes de `patterns`, PDH/PDL y PWH/PWL con el calendario de `market-data`, máximo y mínimo de las ventanas UTC de sesión, números redondos más cercanos por encima y por debajo del cierre con pasos por instrumento en `[round_numbers]` (`round_number_levels`, `round_high`/`round_low`, #497), y su estado intacto/barrido/roto o vencido, contrato `liquidity_level.v1`)
- volumen relativo de la vela de un sweep contra la mediana de las velas anteriores (`sweeps_with_volume_at`, #497; vacío si no hay volumen)
- estadísticas informativas de §5.4.1 (`stats`: eventos de FVG en 15m, 1h y 1d con toque, mitad, llenado, vencimiento y reacción, `fvg_event.v1`; gaps de apertura diarios, `opening_gap.v1`; agregados por instrumento, timeframe, dirección y tercil de tamaño con IC de Wilson, `liquidity_stats.v1`)
- desequilibrio aproximado desde el precio (`imbalance`: velas de desplazamiento e índice de −1 a +1, `imbalance.v1`; no es flujo de órdenes)
- rechazo de la spec §9.1 (`rejection_state_at`: martillo o estrella fugaz que toca un nivel, un FVG o un order block, o un sweep que cierra adentro, reuniendo los detectores existentes)
- config en `config/liquidity.toml`

## Estado

- hecho (#43)
- mapa de liquidez (#231)
- estadísticas de FVG y gaps y desequilibrio (#231, guardadas con el ejemplo `liquidity_stats` de `cfds-ingest`)
- rechazo (#220)
- números redondos y volumen de la barrida (#497, informativos)
- niveles redondos guardados con el ejemplo `liquidity_levels` (#504, migración 0045)
