# Diagnóstico: por qué pierde la estrategia en DE40 (futuros)

Issue #323 · 23-11-2026 · **solo lectura**: no cambia ningún parámetro ni ninguna config. Presenta hallazgos y opciones; **decide el dueño**.

## Datos y método

- **Corrida:** DE40 con las velas de 15m y 1h de los futuros (FDAX, Databento), guardada bajo `DE40.FUT`, del 01-03-2025 al 01-11-2026 (hay velas desde el 11-03-2025). Usa la config de `main` y el kill switch que se reactiva a los 20 días hábiles (solo backtest, #277).
  - Sin noticias: no hay interpretaciones en el período.
  - Sin filtro de spread, porque con futuros no hay spread medido del CFD; los costos usan el spread de config, 1,8 puntos.
- **Resultado:** 384 operaciones y capital de 10000 a 6433 (−35,7 %), con 9 eventos del kill switch (5 activaciones y 4 reactivaciones). Comprar y mantener en el mismo período: +1552 (+15,5 %).
- **Herramienta:** el ejemplo `diagnostico` (`engine/crates/backtest/examples/diagnostico.rs`) corre el mismo backtest que `backtest_run`, con la misma preparación, **en memoria y sin guardar nada**. Se corrió contra la base real en una sesión de solo lectura.
- **Montos:** en la moneda de la cuenta (EUR, la misma que la de DE40). Solo agregados, sin precios.
- **R** = tamaño × distancia del stop desde la entrada ejecutada. El riesgo por operación es el 1 % del capital, así que 1 R ≈ 100 al principio.
- **Intervalos:** cada tasa lleva su IC 95 % de Wilson (z de `config/probability.toml`). Los grupos con menos de 30 operaciones se marcan como **muestra chica**.
- **Payoff:** ganancia media de las ganadoras / pérdida media de las perdedoras.

> **Advertencias.** Es **una** muestra en sample, de 19 meses y un solo instrumento, sin fuera de muestra. Con decenas de grupos, alguno va a parecer bueno o malo por azar: casi todos los IC se superponen. Ninguna diferencia de esta tabla justifica sola cambiar un parámetro. Cualquier opción tiene que pasar por el walk-forward de la Fase 5 (#298) y el fuera de muestra, y para DE40 hoy no alcanzan los datos (ver "Límites").

## Resumen

1. **La pérdida tiene dos mitades.** A precio medio, sin costos, la estrategia ya pierde −1685. Los costos suman otros −1881, y el neto queda en −3566. No es solo un problema de costos: **no hay ventaja antes de costos** en esta muestra.
2. **El punto de equilibrio no se alcanza.** Gana el 34,1 % de las operaciones (IC 29,5–38,9 %) con payoff 1,65. Para quedar en cero con ese payoff hace falta acertar el 37,7 %; el déficit es de unos 3,6 puntos, dentro del IC.
3. **El stop manda.** El 63 % de las operaciones (242) termina en el stop a −1,03 R. Llegan a TP1 109 (28 %), que dejan en promedio +1,9 R.
   - La mitad de las perdedoras llegó a +0,49 R a favor antes de perder, y una de cada cuatro llegó a ~1 R.
   - La MAE mediana es 1,07 R: el precio suele ir hasta el stop.
4. **Los stops cortos son los que más pierden.** Con stop a menos de 1 ATR: −0,29 R por operación. Con stop de 2,5 a 3 ATR: +0,16 R y 44,8 % de acierto. Es el mismo patrón que los 295 rechazos por StopTooFar, con stops que la estructura pide más lejos (mediana 4,51 ATR, #277).
5. **La apertura de Xetra pesa.** Las entradas de las 07:00 y 08:00 UTC pierden −2547 juntas, y ahí cae el 43 % de los StopTooFar (07:00–08:59 UTC; 58 % si se suma la hora de las 09:00, como en #277). Las 10:00 UTC también pierden fuerte (−1829). Las 09:00 y 12:00 UTC ganan. Los IC se superponen.
6. **Volatilidad alta, peor resultado.** En el tercio alto del ATR de 15m (descriptivo, dentro de la muestra): 27 % de acierto y −3201; en el medio, +1182. El tercio oficial (`daily_vol_terciles`) **no tiene dato**, porque necesita 3 años de historia de `DE40.FUT`.
7. **Costos:** el spread de config equivale al 77 % del P/L a precio medio (en valor absoluto), el slippage al 24 % y el swap al 10 %. No hay comisión. Por operación suman unos 0,04 R.

## Cortes del P/L neto

Columnas: operaciones, % que gana (IC 95 %), neto total, neto medio, R medio y payoff.

### Todas

| Grupo | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff |
|---|---|---|---|---|---|---|---|
| todas | 384 | 34,1 % | 29,5–38,9 % | −3566 | −9,2 | −0,09 | 1,65 |

### Hora de entrada (UTC)

Xetra abre a las 07:00 UTC en verano y a las 08:00 UTC en invierno.

| Hora | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff | |
|---|---|---|---|---|---|---|---|---|
| 07:00 | 55 | 25,4 % | 15,8–38,3 % | −1684 | −30,6 | −0,31 | 1,74 | |
| 08:00 | 59 | 30,5 % | 20,2–43,1 % | −863 | −14,6 | −0,16 | 1,79 | |
| 09:00 | 56 | 41,0 % | 29,1–54,1 % | 695 | 12,4 | +0,16 | 1,71 | |
| 10:00 | 37 | 18,9 % | 9,4–34,2 % | −1829 | −49,4 | −0,46 | 1,61 | |
| 11:00 | 31 | 35,4 % | 21,1–53,0 % | −300 | −9,6 | −0,14 | 1,53 | |
| 12:00 | 44 | 45,4 % | 31,7–59,9 % | 1087 | 24,7 | +0,23 | 1,72 | |
| 13:00 | 41 | 39,0 % | 25,6–54,2 % | 212 | 5,1 | +0,00 | 1,70 | |
| 14:00 | 41 | 39,0 % | 25,6–54,2 % | −270 | −6,6 | −0,06 | 1,37 | |
| 15:00 | 17 | 29,4 % | 13,2–53,1 % | −381 | −22,4 | −0,22 | 1,63 | muestra chica |
| 16:00 | 3 | 33,3 % | 6,1–79,2 % | −231 | −77,2 | −0,67 | 0,88 | muestra chica |

### Gatillos de la señal (tipo de setup)

| Gatillos | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff | |
|---|---|---|---|---|---|---|---|---|
| rechazo en nivel | 102 | 35,2 % | 26,7–44,9 % | −547 | −5,3 | −0,05 | 1,68 | |
| patrón de velas | 94 | 32,9 % | 24,3–42,9 % | −343 | −3,6 | −0,02 | 1,91 | |
| patrón de velas + rechazo en nivel | 78 | 32,0 % | 22,7–43,0 % | −2146 | −27,5 | −0,27 | 1,27 | |
| rechazo en nivel + sweep | 54 | 33,3 % | 22,2–46,6 % | −458 | −8,4 | −0,10 | 1,73 | |
| patrón de velas + rechazo en nivel + sweep | 23 | 34,7 % | 18,8–55,1 % | −83 | −3,6 | −0,08 | 1,77 | muestra chica |
| doble techo/suelo + rechazo en nivel | 13 | 38,4 % | 17,7–64,4 % | 112 | 8,6 | +0,02 | 1,82 | muestra chica |
| doble techo/suelo | 5 | 80,0 % | 37,5–96,3 % | 514 | 102,9 | +0,94 | 1,45 | muestra chica |
| patrón de velas + sweep | 5 | 20,0 % | 3,6–62,4 % | −328 | −65,7 | −0,65 | 0,61 | muestra chica |
| sweep | 5 | 20,0 % | 3,6–62,4 % | −292 | −58,4 | −0,47 | 1,49 | muestra chica |
| doble techo/suelo + patrón de velas + rechazo en nivel | 3 | 33,3 % | 6,1–79,2 % | −100 | −33,6 | −0,35 | 0,89 | muestra chica |
| doble techo/suelo + patrón de velas | 2 | 50,0 % | 9,4–90,5 % | 107 | 53,7 | +0,36 | 2,08 | muestra chica |

"Patrón de velas + rechazo en nivel" explica el 60 % de la pérdida neta, con un payoff bajo (1,27). El doble techo/suelo sale positivo, pero en muestras chicas.

### Dirección

| Dirección | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff |
|---|---|---|---|---|---|---|---|
| largo | 198 | 32,8 % | 26,6–39,6 % | −2074 | −10,4 | −0,10 | 1,72 |
| corto | 186 | 35,4 % | 28,9–42,5 % | −1492 | −8,0 | −0,08 | 1,59 |

Las dos direcciones pierden parecido. En un período en que el índice subió, los largos no rinden mejor.

### Día de la semana (UTC)

| Día | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff |
|---|---|---|---|---|---|---|---|
| lunes | 81 | 35,8 % | 26,2–46,6 % | −642 | −7,9 | −0,10 | 1,56 |
| martes | 72 | 33,3 % | 23,5–44,8 % | −810 | −11,2 | −0,10 | 1,66 |
| miércoles | 87 | 33,3 % | 24,3–43,7 % | −690 | −7,9 | −0,08 | 1,77 |
| jueves | 77 | 35,0 % | 25,3–46,2 % | 256 | 3,3 | +0,02 | 1,94 |
| viernes | 67 | 32,8 % | 22,7–44,7 % | −1680 | −25,0 | −0,23 | 1,24 |

El viernes concentra la peor relación (payoff 1,24). Una hipótesis, sin verificar: el cierre forzado del fin de semana corta las ganadoras.

### Tercio de la volatilidad

- **Tercio oficial** (`daily_vol_terciles`, 3 años de historia): **0 de 384 operaciones con dato**. `DE40.FUT` tiene diarias desde 2025-03 y hacen falta 3 años antes de cada día.
- **Sustituto descriptivo:** tercios del ATR de 15m al decidir, con los cortes calculados sobre **toda** la muestra. Usa información de la muestra entera, así que **no sirve para decidir en vivo**; solo describe.

| Tercio del ATR | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff |
|---|---|---|---|---|---|---|---|
| 1 bajo | 128 | 32,0 % | 24,5–40,5 % | −1547 | −12,0 | −0,11 | 1,76 |
| 2 medio | 128 | 42,9 % | 34,7–51,6 % | 1182 | 9,2 | +0,11 | 1,53 |
| 3 alto | 128 | 27,3 % | 20,3–35,6 % | −3201 | −25,0 | −0,29 | 1,67 |

### Motivo de salida (secuencia de salidas)

| Salidas | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | |
|---|---|---|---|---|---|---|---|
| stop | 242 | 0,0 % | 0,0–1,5 % | −25034 | −103,4 | −1,03 | |
| TP1 → trailing | 96 | 100 % | 96,1–99,9 % | 18887 | 196,7 | +1,94 | |
| cierre forzado | 18 | 72,2 % | 49,1–87,5 % | 592 | 32,8 | +0,31 | muestra chica |
| por tiempo | 15 | 60,0 % | 35,7–80,1 % | 267 | 17,8 | +0,18 | muestra chica |
| TP1 → break-even | 9 | 100 % | 70,0–99,9 % | 809 | 89,8 | +0,95 | muestra chica |
| TP1 → cierre forzado | 4 | 100 % | 51,0–100 % | 911 | 227,9 | +2,32 | muestra chica |

Pasada TP1 (109 operaciones), la gestión funciona: ninguna de esas termina en pérdida. La pérdida viene del 63 % que va directo al stop.

### Distancia del stop (en ATR de 15m al decidir; el riesgo rechaza más de 3)

| Distancia | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff |
|---|---|---|---|---|---|---|---|
| menos de 1 | 41 | 29,2 % | 17,6–44,4 % | −974 | −23,7 | −0,29 | 1,59 |
| 1 a 1,5 | 112 | 33,9 % | 25,8–43,1 % | −1037 | −9,2 | −0,05 | 1,67 |
| 1,5 a 2 | 118 | 29,6 % | 22,1–38,4 % | −1924 | −16,3 | −0,15 | 1,84 |
| 2 a 2,5 | 64 | 37,5 % | 26,6–49,7 % | −597 | −9,3 | −0,13 | 1,38 |
| 2,5 a 3 | 49 | 44,8 % | 31,8–58,6 % | 966 | 19,7 | +0,16 | 1,66 |

## MAE y MFE en R

MAE es lo máximo que fue en contra; MFE, lo máximo que fue a favor. Se miden entre la vela de entrada y la de la última salida, ambas incluidas. Es una cota conservadora: dentro de una vela de 15m no se sabe el orden de máximo y mínimo.

| Medida | p10 | p25 | Mediana | p75 | p90 | n |
|---|---|---|---|---|---|---|
| MAE (todas) | 0,19 | 0,55 | 1,07 | 1,30 | 1,69 | 384 |
| MFE (todas) | 0,09 | 0,32 | 0,86 | 2,15 | 3,03 | 384 |
| MFE de las perdedoras | 0,03 | 0,18 | 0,49 | 0,96 | 1,44 | 253 |

TP1 está a 2 R. Una de cada cuatro perdedoras llegó a ~1 R a favor y una de cada diez a 1,4 R, pero no a TP1 (el stop pasa a break-even solo después de TP1). En cambio, la mitad de las perdedoras casi no fue a favor: menos de 0,5 R.

## Costos

| Concepto | Total | Sobre el P/L a precio medio (valor absoluto) |
|---|---|---|
| P/L a precio medio (sin costos) | −1685 | |
| Spread (1,8 puntos de config, por ejecución la mitad) | 1302 | 77,3 % |
| Slippage (0,1 × spread + 1 tick por ejecución) | 405 | 24,0 % |
| Comisión | 0 | 0 % |
| Financiación (swap) | 173 | 10,3 % |
| **Costos totales** | **1881** | 111,6 % |
| **P/L neto** | **−3566** | |

- Como el P/L bruto es negativo, el "% del bruto" no tiene sentido. Por eso los costos se comparan con el P/L a precio medio en valor absoluto, y se dan además en R.
- Control: spread + slippage coincide exacto con P/L medio − P/L bruto.
- Por operación, los costos son unos **0,04 R**, casi la mitad del −0,09 R neto medio.
- El spread es el de config porque con futuros no hay spread medido del CFD (#267). Si el spread real de IG en DE40 fuera menor, los costos bajarían, pero el P/L a precio medio seguiría negativo.

## Rechazos del riesgo

| Motivo | Señales |
|---|---|
| StopTooFar (stop a más de 3 ATR) | 295 |
| BelowMinDealSize | 8 |
| DailyLossLimit | 5 |
| DrawdownLimit (activa el kill switch) | 5 |

StopTooFar por hora de la decisión (UTC):

| Hora | 07 | 08 | 09 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|
| Señales | 65 | 61 | 45 | 28 | 21 | 16 | 16 | 20 | 18 | 5 |
| Parte | 22,0 % | 20,6 % | 15,2 % | 9,4 % | 7,1 % | 5,4 % | 5,4 % | 6,7 % | 6,1 % | 1,6 % |

Del análisis de #277: el stop rechazado estaba a una mediana de 4,51 ATR (p25 3,79, p75 5,10, p90 6,05). Hubo 93 entre 3 y 4 ATR, 117 entre 4 y 5, 79 entre 5 y 7 y 6 entre 7 y 10. Eran señales que la estructura pedía con stop lejano. No hay manera de saber cómo habrían salido sin simularlas.

## Opciones para el dueño

**No son recomendaciones de cambio.** Cada una es algo para evaluar con el walk-forward y el fuera de muestra (#298) antes de tocar la config. Ninguna está probada fuera de esta muestra.

1. **Horario.** Evaluar no abrir en la primera hora y media después de la apertura de Xetra (07:00–08:59 UTC en verano), donde se juntan la pérdida y los StopTooFar. La config es `config/trading_window.toml`.
2. **Distancia del stop.** Los stops de menos de 1 ATR pierden más y los de 2,5 a 3 ATR ganan. Se puede evaluar subir el mínimo (`stop_distance.min_atr`, hoy 0,5) o el colchón del stop (`stop.atr_buffer`). También `stop_distance.max_atr` (hoy 3): la sensibilidad de #303 ya lo mide alrededor de 3. Ojo: un stop más lejos con el mismo riesgo implica un tamaño menor y el mismo R.
3. **Salidas.** Una de cada cuatro perdedoras llegó a ~1 R sin alcanzar TP1 (2 R). Se puede evaluar TP1 o el paso a break-even antes. En la prueba de humo de #310, `tp1_r` 1,5 dio peor que 2, así que no está claro.
4. **Volatilidad.** El tercio alto del ATR es el peor. Un filtro por tercio de volatilidad recién se podría evaluar cuando haya tercios oficiales; para `DE40.FUT` faltan años de historia.
5. **Tipo de setup.** "Patrón de velas + rechazo en nivel" concentra el 60 % de la pérdida, con payoff 1,27. Se puede evaluar el peso de esa combinación en el puntaje (`config/strategy.toml` `score`).
6. **Costos.** Habría que confirmar el spread real de IG en DE40 frente a los 1,8 puntos de config. No arregla la falta de ventaja antes de costos, pero cambia cuánto falta.
7. **Datos.** Para evaluar cualquiera de estas opciones con la separación actual hacen falta al menos 39 meses de DE40 y hoy hay 19 (#310). Las salidas son más histórico de FDAX o decidir otra separación para la evaluación.

## Límites

- Es una sola corrida, en sample y sobre un solo instrumento: no hay fuera de muestra.
- Con muchos cortes, los grupos extremos se explican en parte por azar, y los IC se superponen en casi todo.
- Las velas son de futuros, no del CFD: la base y el horario de cotización son distintos (ADR 0011).
- El tercio de volatilidad usado es un sustituto descriptivo.
- MAE y MFE salen de velas de 15m: son cotas, no el recorrido exacto.
