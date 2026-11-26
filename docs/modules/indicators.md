---
modulo: indicators
ruta: `engine/crates/indicators`
lenguaje: Rust
estado: hecho
resumen: Indicadores técnicos (SMA, EMA, ATR, cruces, volatilidad realizada, pivotes diarios, caída), tendencia de 1h y medidas de sentimiento desde precios diarios.
---

# indicators

## Responsabilidad (única)

- Indicadores técnicos (SMA, EMA, ATR, cruces, volatilidad realizada y terciles, pivotes diarios, caída, intervalo de Wilson) y tendencia de 1h. **No hay RSI ni VWAP** en el motor (lección 6.3 de la Academia)
- medidas de sentimiento desde precios diarios (spec §9.1): volatilidad realizada, su tercio frente a su propia historia (`vol_terciles`, el único cálculo: lo leen los análogos y ML) y caída desde el máximo (`drawdown_from_high`)
- intervalo de Wilson de una proporción (`wilson`, el único: lo usan `journal` y `liquidity`)
- SMA y EMA de cualquier serie de valores (`sma_values`, `ema_values`), por ejemplo los cierres diarios (EMA 365 diaria del gráfico, #491)
- cruce dorado y de la muerte entre dos medias (`crosses`) y puntos pivote diarios clásicos (`floor_pivots`), herramientas de análisis (#491)

## Estado

- ATR de Wilder (#5), SMA, EMA y tendencia (#37)
- ATR sobre velas diarias y volatilidad realizada (#148)
- caída desde el máximo (#213)
- tercio de la volatilidad (#218)
- Wilson movido desde `journal` (#231)
- EMA de valores, cruces y puntos pivote diarios (#491)
