# Fase 3b: validación de los indicadores contra una referencia, con datos reales

Criterio de aprobación de la fase 3b (`docs/ROADMAP.md`): *coincide con la referencia y ningún valor histórico cambia al agregar datos*. Sale de #81.

## Datos

- DE40 (`IX.D.DAX.IFE.IP`, demo de IG), 948 velas de 15m a precio medio, del 09-11-2026 al 20-11-2026. Se cargaron con `ig_backfill` por el circuito de ingesta.
- Las velas **no están en el repo**: es público y redistribuir precios de IG puede violar sus términos. Para reproducir, se cargan y exportan localmente (ver `tools/verify/README.md`).

## Referencia elegida

- **TA-Lib 0.8.3**: implementación independiente y de uso estándar.
- **TradingView** no se puede automatizar, y además muestra otro feed: no son los precios medios de la cuenta demo, así que sus valores no serían comparables vela por vela.
- **Convención del motor:** la spec §4 pide la de TradingView: `ta.atr = ta.rma(ta.tr(true), 14)`, con primer TR = H − L y primer ATR = promedio de los primeros 14 TR. TA-Lib arranca un paso después: su primer ATR es el promedio de TR₁..TR₁₄. Con el suavizado de Wilder esa diferencia inicial se multiplica por 13/14 en cada vela y desaparece.

## Resultado (`tools/verify/compare_reference.py`, 20-11-2026)

| Indicador | Desde la vela | Diferencia relativa máxima | Lectura |
|---|---|---|---|
| SMA(50) | 49 (primer valor, igual que TA-Lib) | 3,1 × 10⁻¹⁵ | Idéntico (redondeo de `float` de TA-Lib) |
| ATR(14), arranque | 13 | 1,2 × 10⁻² | Diferencia de arranque esperada: convención de TradingView vs. TA-Lib |
| ATR(14), convergido | 300 | 1,3 × 10⁻¹¹ | Idéntico |

## Sin repintado, con los mismos datos reales

Se exportó la misma serie cortada en dos puntos y se comparó vela por vela con la serie completa: OHLC, SMA, ATR, tendencia de 1h, puntajes largo y corto, y pivotes confirmados.

| Corte | Velas | Velas idénticas | Pivotes idénticos |
|---|---|---|---|
| 13-11 00:00 UTC | 384 | sí | sí |
| 17-11 12:00 UTC | 620 | sí | sí |

Queda fijado en CI con datos sintéticos: `no_exported_value_changes_when_more_candles_arrive` (`engine/crates/backtest/tests/export.rs`).

## Pendiente para la aprobación

- **Revisión humana en el gráfico:** las dos personas revisan una muestra de operaciones en el visor (#82). Pueden cotejar algunos valores de SMA y ATR con el gráfico del DE40 en IG o TradingView, sabiendo que el feed puede diferir unos puntos.
