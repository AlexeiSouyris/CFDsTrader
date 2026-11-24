# Chequeos descriptivos pedidos por la revisión externa

Issue #512 · 24-11-2026 · rondas que se revisan: `2026-11-ronda2-tendencia-y-momentum-intradia.md` (#486) y `2026-11-ronda3-volatilidad-y-numeros-redondos.md` (#493), con su pre-registro `2026-11-ronda3-preregistro.md`.

> **Todo esto es descriptivo.** No vota, no gasta presupuesto de error y **no cambia ningún veredicto registrado** (V, M1 y B siguen sin pasar y cerradas). Cambiar un criterio después de ver los resultados **no vale** para el programa 2: todo lo que se quiera probar a partir de acá va a un **pre-registro nuevo**. El fuera de muestra (desde el 01-11-2025) no se cargó ni se miró. Las velas son de futuros, no del CFD. Los resultados pasados no prometen ningún resultado futuro, y nada de esto es una recomendación de operar.

## Resumen

1. **Deriva nocturna (02:00 a 03:00 de Nueva York): la revisión tiene razón.** En 2013–2020 es clara en los tres índices (+1,4 a +1,6 puntos básicos por día, t de 4,1 a 4,4). **Desde 2021 desaparece** (−0,6 a +0,1 pb, ningún t distinto de cero). Año por año, ningún año desde 2021 es positivo con t > 1. El t = 3,15 de la ronda 3 viene entero de la primera mitad.
2. **Después de un día de caída fuerte la deriva es mayor** (+2,2 pb en US500, +2,1 en US100 y +0,8 en US30, frente a +0,5 a +0,8 el resto de los días), **pero solo en 2013–2020.** Desde 2021 tampoco hay nada después de las caídas (t de −1,0 a +0,4). Aun en el mejor caso, el costo del CFD a esa hora (1,5 a 4 pb) se lleva casi todo.
3. **50 % comprar y mantener + 50 % V escalada:** Sharpe mensual **0,278 frente a 0,261** de comprar y mantener (escenario EUR; 0,252 frente a 0,237 en USD). El retorno es casi igual (−0,01 % por mes); la mejora viene de **menos volatilidad** (desvío 3,90 % frente a 4,20 %). Es una diferencia chica frente al error de un Sharpe con 153 meses (≈ 0,08 mensual) y **no se probó**: no cambia el criterio 5 de V.
4. **Confirmado: comprar y mantener paga los mismos costos que las estrategias** (spread, slippage, comisión y financiación de IG menos la base del futuro), con las mismas funciones. La revisión supuso lo contrario; el código lo desmiente (citas abajo).
5. **B (momentum de la última media hora) antes de costos:** −0,010 R por operación en US500 (t = −0,8), −0,013 R en US100 (t = −1,2) y −0,031 R en US30 (t = −2,6). **El t = −11 no es un efecto de mercado:** es el costo (0,08 a 0,15 R por operación, casi fijo) repetido en unas 2 450 operaciones por índice. Sin el costo, B está en cero en US500 y US100 y apenas negativo en US30.

## Cómo se corrió

- Ejemplo nuevo `chequeos_revision` (`engine/crates/backtest/examples/chequeos_revision.rs`), que reutiliza la carga de las rondas (`examples/common/research_round.rs`) y `cfds_backtest::research`: `window_returns` (deriva), `managed_returns` y `vol_constant` (V y comprar y mantener), `intraday_momentum_plans` y `run_family` (B). Lo nuevo vive en `research::review_checks` (caída fuerte, deriva después de la caída y cartera mezclada), con 5 tests en `engine/crates/backtest/tests/review_checks.rs`. `config/strategy.toml`, `config/risk.toml`, el runner y el paper no cambian.
- Mismos datos que la ronda 3: futuros ES, NQ e YM de Databento, del 01-01-2011 al 01-11-2025, walk-forward de `config/data_split.toml` (51 ventanas de validación, 02-2013 → 10-2025, 153 meses), cuenta nueva por ventana y por índice, costos de config.
- Base en solo lectura (`default_transaction_read_only=on`). Dos corridas, una por escenario de financiación, el 24-11-2026 a las 18:09 y 18:10 (Madrid), con el binario del commit `feat(engine/backtest): descriptive checks from the external review` (hora de autor 18:09:34, antes de la primera corrida). Ninguna se repitió.
- **Comprobación:** comprar y mantener sale igual que en la ronda 3 (+1,0955 % por mes, Sharpe 0,2607 en EUR; 0,2367 en USD) y la deriva de todo el período y B neto coinciden con los informes de las rondas 3 y 2.

## 1. Deriva nocturna de 02:00 a 03:00 ET, por mitades y por año

Misma definición que la ronda 3 (pre-registro §5): retorno desde la apertura de la vela de 15m de las 02:00 ET hasta el cierre de la de las 03:00 ET, días hábiles con las cuatro velas y sin vencimiento adentro. No depende del escenario de financiación (los dos dan lo mismo).

> **Descriptivo.** Cambiar un criterio después de ver esto no vale para el programa 2; lo que se quiera probar va a un pre-registro nuevo.

| Índice | Período | Días | Media (pb por día) | t |
|---|---|---|---|---|
| US500 | todo (02-2013 → 10-2025) | 3276 | +0,83 | +3,15 |
| US500 | 2013–2020 | 2026 | +1,53 | +4,40 |
| US500 | 2021–10/2025 | 1250 | −0,31 | −0,79 |
| US100 | todo | 3270 | +1,03 | +3,42 |
| US100 | 2013–2020 | 2020 | +1,62 | +4,27 |
| US100 | 2021–10/2025 | 1250 | +0,08 | +0,17 |
| US30 | todo | 3276 | +0,63 | +2,49 |
| US30 | 2013–2020 | 2029 | +1,40 | +4,07 |
| US30 | 2021–10/2025 | 1247 | −0,63 | −1,78 |

Año por año (media en pb y t):

| Año | US500 | US100 | US30 |
|---|---|---|---|
| 2013 (desde febrero) | +1,44 (2,78) | +1,72 (3,16) | +1,45 (3,17) |
| 2014 | +1,06 (1,95) | +0,75 (1,30) | +0,89 (1,78) |
| 2015 | +2,65 (3,34) | +3,19 (3,75) | +2,39 (3,17) |
| 2016 | +1,77 (1,79) | +1,93 (1,78) | +1,75 (1,94) |
| 2017 | +0,43 (1,37) | +0,28 (0,73) | +0,54 (1,65) |
| 2018 | +1,39 (1,62) | +1,56 (1,43) | +1,04 (1,16) |
| 2019 | −0,55 (−0,79) | −0,76 (−0,84) | −0,86 (−1,25) |
| 2020 | +3,99 (2,00) | +4,27 (2,08) | +4,01 (1,97) |
| 2021 | +0,08 (0,10) | +0,64 (0,57) | −0,23 (−0,31) |
| 2022 | −0,22 (−0,19) | +0,24 (0,16) | −0,28 (−0,28) |
| 2023 | −1,23 (−1,78) | −1,23 (−1,49) | −0,97 (−1,52) |
| 2024 | −0,32 (−0,59) | +0,20 (0,25) | −1,27 (−2,54) |
| 2025 (hasta octubre) | +0,23 (0,21) | +0,66 (0,53) | −0,35 (−0,35) |

Lectura:
- **El efecto es de 2013–2020.** Desde 2021 la media está en cero o es negativa en los tres índices. Coincide con lo que dice la revisión: el efecto publicado se apagó.
- 2019 ya fue negativo, y 2020 (el año de la pandemia, muy volátil) aporta mucho a la primera mitad. Con un año de unos 250 días, el error de la media anual es de 0,5 a 2 pb: los años sueltos son ruidosos.
- Con el costo de ida y vuelta del CFD a esa hora (≈ 1,5 a 4 pb, ronda 3), ni la primera mitad alcanzaba con holgura.

## 2. La deriva solo después de un cierre de contado con caída fuerte

**Regla, fijada antes de mirar** (en `config/research.toml`, sección `[review_checks]`, en el commit de las 18:09:34, antes de la primera corrida de las 18:09:40):

- Retorno del día = del cierre de la sesión de contado anterior al cierre de la de hoy (última vela de 15m dentro de la sesión de XNYS), encadenado en los vencimientos.
- **Caída fuerte** = ese retorno queda **por debajo del percentil 20 de los 250 días anteriores** (el 50.º menor de esos 250, k = ⌈0,2 × 250⌉). Los primeros 250 días no tienen marca (calentamiento).
- La ventana de 02:00 a 03:00 ET del día D cuenta como "después de una caída fuerte" si el **último cierre de contado terminado antes de las 02:00** fue una caída fuerte (el lunes mira el viernes).
- Sin ningún parámetro ajustado: 250 días y el percentil 20 son los del ejemplo del Issue.

> **Descriptivo.** Cambiar un criterio después de ver esto no vale para el programa 2; lo que se quiera probar va a un pre-registro nuevo.

| Índice | Período | Tras caída fuerte: días, media (pb), t | Resto de los días: días, media (pb), t |
|---|---|---|---|
| US500 | todo | 632, +2,25, +2,79 | 2644, +0,49, +1,87 |
| US500 | 2013–2020 | 397, +3,30, +3,18 | 1629, +1,10, +3,13 |
| US500 | 2021–10/2025 | 235, +0,47, +0,37 | 1015, −0,49, −1,28 |
| US100 | todo | 646, +2,05, +2,29 | 2624, +0,78, +2,57 |
| US100 | 2013–2020 | 405, +2,96, +2,71 | 1615, +1,28, +3,32 |
| US100 | 2021–10/2025 | 241, +0,53, +0,34 | 1009, −0,02, −0,05 |
| US30 | todo | 638, +0,84, +1,08 | 2638, +0,58, +2,30 |
| US30 | 2013–2020 | 395, +2,08, +2,02 | 1634, +1,24, +3,56 |
| US30 | 2021–10/2025 | 243, −1,18, −1,04 | 1004, −0,49, −1,44 |

Lectura:
- Después de una caída fuerte la deriva es **más grande** (en US500 y US100, unas 2 a 3 veces la del resto de los días), y la marca cae en ≈ 20 % de los días, como se esperaba.
- **Pero tampoco sobrevive a 2021:** en la segunda mitad, después de las caídas, la media es +0,5 pb en US500 y US100 (t ≈ 0,3) y −1,2 pb en US30. Es la misma historia del punto 1.
- Aun con +3,3 pb (US500, 2013–2020), el costo del CFD a esa hora (3–4 pb en US500) se lo lleva. En US100, con un costo de 1,5–2,2 pb, quedaría un margen chico solo en la primera mitad.
- Son 6 cortes (3 índices × 2 mitades) de una idea elegida después de leer la revisión: aunque alguno se vea bien, no cuenta como prueba.

## 3. Cartera 50 % comprar y mantener + 50 % V escalada a la volatilidad de comprar y mantener

Cómo se armó, sin mirar el futuro:
- **V** es exactamente la de la ronda 3 (peso `min(2, c / RV)`, `c` del entrenamiento de cada ventana, rebalanceo mensual en la apertura siguiente).
- **Escala** `s` de cada ventana = desvío de los retornos mensuales de comprar y mantener / desvío de los de V, **los dos calculados en el entrenamiento de esa ventana** (24 meses; desde el primer mes con una decisión de V, con la misma `c`). Se usa sin cambios en los 3 meses de validación.
- **Mes de la cartera** = 0,5 × comprar y mantener + 0,5 × s × V, por índice; después, el promedio de los tres índices, como en las rondas.
- **Mismos costos**: los dos lados son los retornos de `managed_returns`, con spread, slippage, comisión y financiación menos la base. Supuesto: la mezcla se rebalancea cada mes sin costo extra (los dos lados son el mismo índice, así que en la práctica es una sola posición con peso 0,5 + 0,5 × s × w). Escalar V por `s` se toma como lineal (los costos de V también se escalan).
- Escalar la cartera entera a la volatilidad de comprar y mantener no cambiaría el Sharpe (multiplica la media y el desvío por lo mismo), por eso se informa el Sharpe.

> **Descriptivo.** No cambia el criterio 5 de V (ronda 3: Sharpe de V ≥ el de comprar y mantener, que no pasó). Cambiar un criterio después de ver esto no vale para el programa 2; lo que se quiera probar va a un pre-registro nuevo.

| Índice | Escenario | Escala media (mín – máx) | Comprar y mantener: retorno por mes (Sharpe) | Mezcla: retorno por mes (Sharpe) | Sharpe con costos × 1,5 (CyM / mezcla) |
|---|---|---|---|---|---|
| US500 | EUR | 1,13 (0,91 – 1,37) | +0,99 % (0,245) | +0,98 % (0,259) | 0,225 / 0,236 |
| US500 | USD | 1,14 (0,93 – 1,40) | +0,90 % (0,220) | +0,88 % (0,232) | 0,188 / 0,195 |
| US100 | EUR | 1,15 (0,96 – 1,44) | +1,46 % (0,293) | +1,47 % (0,312) | 0,277 / 0,294 |
| US100 | USD | 1,17 (0,98 – 1,46) | +1,36 % (0,273) | +1,37 % (0,290) | 0,246 / 0,261 |
| US30 | EUR | 1,09 (0,87 – 1,34) | +0,84 % (0,203) | +0,80 % (0,203) | 0,184 / 0,181 |
| US30 | USD | 1,10 (0,88 – 1,36) | +0,74 % (0,178) | +0,70 % (0,177) | 0,147 / 0,143 |

| Cartera (153 meses) | EUR | USD |
|---|---|---|
| Comprar y mantener: retorno por mes, desvío, Sharpe mensual (anual ≈) | +1,096 %, 4,20 %, 0,261 (0,90) | +0,996 %, 4,21 %, 0,237 (0,82) |
| Mezcla: retorno por mes, desvío, Sharpe mensual (anual ≈) | +1,082 %, 3,90 %, 0,278 (0,96) | +0,985 %, 3,92 %, 0,252 (0,87) |
| Diferencia de retorno por mes (t) | −0,013 % (−0,11) | −0,011 % (−0,09) |
| Sharpe con costos × 1,5 (CyM / mezcla) | 0,242 / 0,256 | 0,206 / 0,216 |

Lectura:
- La mezcla tiene un Sharpe **un poco mayor** (+0,017 mensual, ≈ +0,06 anual) en los dos escenarios y con costos × 1,5, en US500 y US100; en US30 da lo mismo.
- **No gana más:** el retorno es el mismo. La mejora es de **menos riesgo** (V se achica en los meses agitados), que es lo que la literatura le atribuye a la volatilidad manejada.
- El tamaño es chico: con 153 meses el error de un Sharpe mensual es ≈ 0,08, cinco veces la diferencia. No se hizo ninguna prueba y **no se puede afirmar que sea mejor**.
- La escala media fue ≈ 1,1: en el entrenamiento V salía algo menos volátil que comprar y mantener, así que su mitad se agrandó un 10 % en promedio (hasta 1,46 en alguna ventana). Con la escala, la exposición de la parte de V puede pasar el tope de 2 de la ronda 3 (hasta ≈ 2,9); la cartera entera nunca pasa de ≈ 1,96 veces el capital.

## 4. Comprar y mantener paga los mismos costos que las estrategias

La revisión externa supuso que comprar y mantener se calculaba sin costos. **No es así.** Los dos caminos que lo calculan usan las mismas funciones de costos que las estrategias de estudio y que el backtest de la estrategia en uso (`engine/crates/backtest/src/costs.rs`):

- **Spread y slippage:** `fill_price` (`costs.rs`, línea 127): precio = medio ± (spread / 2 + slippage), siempre en contra, con slippage = 0,1 × spread + 1 tick (líneas 134–135).
- **Comisión:** `commission` (`costs.rs`, línea 145). En los índices de IG es 0 (`costs.rs`, encabezado; test `indices_pay_no_commission`).
- **Financiación:** `funding_over` (`costs.rs`, línea 206), noche por noche: largo paga la tasa de referencia + la tasa administrativa de IG sobre el cierre. **Menos la base del futuro:** `basis_carry` (ADR 0015, #467), porque el precio del futuro ya descuenta ese costo.

Dónde lo paga comprar y mantener:

- `engine/crates/backtest/src/research/buy_and_hold.rs` (`buy_and_hold_returns`, ronda 2):
  - líneas 62–65: `entry_price` y `exit_price` con `fill_price`, y `entry_cost` / `exit_cost` = diferencia con el precio medio + `commission`;
  - líneas 95–106: cada corte de financiación paga `funding_over` y se le resta `basis_carry`;
  - líneas 118–121: el valor de la cuenta descuenta `entry_cost` desde la entrada y `exit_cost` en la salida.
- `engine/crates/backtest/src/research/vol_managed.rs` (`managed_returns`, ronda 3, comprar y mantener = peso 1 sin rebalanceos):
  - líneas 166–179 (`trade_cost`): `fill_price` y `commission` en cada compra o venta (entrada, rebalanceos y salida);
  - líneas 206–220: `funding_over` menos `basis_carry` cada noche, sobre las unidades que se tienen.
  - El test `weight_one_without_rebalances_is_exactly_buy_and_hold` (`tests/research_round3.rs`, línea 300) verifica que con peso 1 da **exactamente** lo mismo que `buy_and_hold_returns`, y la cartera real da lo mismo que la ronda 2 (+1,0955 % por mes, Sharpe 0,2607).
- Las estrategias de estudio usan las mismas funciones: `engine/crates/backtest/src/research/run.rs`, líneas 146–176 (`fill_price`, `commission`, `funding_over` y `basis_carry`).

Lo único que comprar y mantener paga **menos** es lo que corresponde: entra y sale una sola vez por ventana de validación (cuenta nueva cada 3 meses, igual que las estrategias), así que paga el spread pocas veces; la financiación la paga todas las noches, igual que cualquier posición que se mantiene. Por eso es la vara justa: la financiación (≈ 0,25 % por mes por la tasa administrativa) es su costo más grande.

## 5. Momentum de la última media hora (B, ronda 2) antes de costos

**El informe de la ronda 2 ya lo traía:** "R medio antes de costos" −0,010 (US500), −0,013 (US100) y −0,031 (US30), con costo medio de 0,149, 0,098 y 0,079 R (`2026-11-ronda2-tendencia-y-momentum-intradia.md`, tabla de B). Lo que no traía era el t antes de costos, que se agrega acá (por operación, en R; igual en los dos escenarios):

> **Descriptivo.** No cambia el veredicto de B (no pasa, cerrada). Lo que se quiera probar va a un pre-registro nuevo.

| Índice | Operaciones | R antes de costos (desvío) | t antes de costos | Costo medio | R neto (desvío) | t neto por operación | Retorno por mes (t, 153 meses) |
|---|---|---|---|---|---|---|---|
| US500 | 2441 | −0,010 (0,60) | −0,83 | 0,149 R | −0,159 (0,61) | −12,8 | −2,50 % (−11,3) |
| US100 | 2478 | −0,013 (0,54) | −1,19 | 0,098 R | −0,111 (0,55) | −10,0 | −1,77 % (−8,7) |
| US30 | 2466 | −0,031 (0,58) | −2,62 | 0,079 R | −0,110 (0,58) | −9,4 | −1,75 % (−8,6) |

Lectura:
- **El t = −11 de la cartera es el costo, no el mercado.** Antes de costos B está en cero en US500 y US100 (t de −0,8 y −1,2). El costo es casi el mismo en cada operación (spread y slippage sobre un stop de 2 ATR de 15m), y con ≈ 2 450 operaciones por índice, restar 0,08 a 0,15 R fijos a cada una da un t muy grande: −0,159 / 0,613 × √2441 ≈ −12,8.
- Por eso el resultado es tan "consistente" (pierde todos los años): la parte de mercado es ruido alrededor de cero y la parte de costo es fija.
- US30 es la excepción: −0,031 R antes de costos con t = −2,6 (al revés de lo que dice la literatura). Es un solo índice entre tres y no se buscó; no se le da peso.

## Para el dueño

- **Ningún veredicto cambia.** V, M1 y B siguen cerradas; la deriva nocturna sigue sin servir con el CFD, y ahora además se ve que **desde 2021 no existe** en nuestros datos. No se activa nada y el fuera de muestra sigue intacto.
- La revisión acertó en el punto 1 (la deriva se apagó) y se equivocó en el 4 (comprar y mantener sí paga los costos).
- Lo único que se ve un poco mejor que comprar y mantener es la **mezcla 50/50 con V** (Sharpe ≈ 0,96 anual frente a 0,90), por menos volatilidad y no por más retorno. Si se quiere probar, va a un **pre-registro nuevo** con su costo en el presupuesto, y con una vara clara: un Sharpe más alto que el de comprar y mantener **por más que el error** de 153 meses, que hoy no se cumple.
- Filas nuevas en `docs/analysis/registro-de-resultados.csv`: R022 a R026, marcadas "descriptivo (revisión externa)".
