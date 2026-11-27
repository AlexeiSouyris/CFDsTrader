# Propuestas por mes de la estrategia en US500 (futuros ES, 2011–2026)

Issue #390 · 24-11-2026 · paso 4 de la spec de ML v1 (#170, `docs/spec/ml-v1.md` §10 y decisión abierta 4): **antes de entrenar, medir cuántas propuestas por mes da la estrategia en US500**. Es **solo un conteo**: no cambia ningún parámetro ni ninguna config de la estrategia, no calcula P/L y no escribe nada en la base.

## Respuesta corta

- **Sí alcanza.** Con la estrategia de `main` sin cambios, US500 da unas **20 propuestas por mes** (entre 18 y 24 de promedio anual, 10 a 40 en un mes suelto), con **27 % de `y = 1`** (TP1 antes que el stop).
- Las **51 ventanas walk-forward** de `config/data_split.toml` (24 meses de entrenamiento) llegan al mínimo de `config/ml.toml` (300 propuestas y 50 de cada clase): entre **462 y 542 propuestas** por ventana, con **111 a 160 de `y = 1`**. La clase escasa es `y = 1` y aun así duplica el mínimo.
- **Cuántos años hacen falta:** el mínimo se cumple en **todas** las ventanas desde **18 meses** de entrenamiento; con 12 meses no lo cumple ninguna (mediana 251 propuestas). Con los 24 meses de la config hay margen de ~1,7× en el total. Con 2011–2026 hay **15 años antes del fuera de muestra**, que dan 51 ventanas; con solo 2021–2026 (lo que había antes) habría 8.
- El fuera de muestra (desde 01-11-2025) tiene 234 propuestas: **solo se contaron**, no entran en ninguna ventana.

Las advertencias de abajo pueden bajar estos números (sobre todo el filtro de noticias y el kill switch), pero hay margen para absorberlas. Lo que sigue (programar `ml_features` para US500.FUT y entrenar) lo decide el dueño.

## Datos y método

- **Velas:** futuros ES (`ES.c.0`, Databento `GLBX.MDP3`, contrato más cercano sin ajuste) del 01-01-2011 al 23-11-2026: 370 610 velas de 15m y 90 442 de 1h, **armadas en memoria** desde las barras de 1 minuto con la misma función que guarda `futures_candles` (`cfds_market_data::futures_candles`). No se guardó ninguna vela: la sesión de PostgreSQL fue de solo lectura (`default_transaction_read_only`).
- **Preparación igual que `backtest_run --fuente futuros`:** validación de datos (§7.8 y §12: 222 datos sospechosos, 214 errores de datos que reinician el calentamiento), horario de entrada del CFD (`config/trading_window.toml`, con la sesión y los feriados de la NYSE), 63 vencimientos (se cierra antes y el calentamiento empieza de nuevo) y el filtro de noticias de alto impacto (§7.2), que **no tiene datos** para el período (ver advertencias).
- **Decisión y riesgo, los del backtest:** `choose` al cierre de la vela `t` (velas cerradas, sin look-ahead), entrada en la apertura de `t+1`, `cfds_risk::check` con la config de `main` (`config/strategy.toml`, `risk.toml`, `backtest.toml` sin cambios). **Una posición a la vez**: no se decide hasta la vela de salida de la propuesta anterior, como el backtest.
- **Etiqueta (spec de ML §1):** `y = 1` si toca TP1 antes que el stop; `y = 0` si sale por el stop (también con gap en la entrada), por tiempo o por un cierre forzado (decisión abierta 1). Se simula con `cfds_backtest::simulate`, el mismo de las salidas del backtest.
- **Ventanas:** las de `cfds_market_data::plan` con `config/data_split.toml` (24 meses de entrenamiento, 3 de validación, paso 3, 12 meses fuera de muestra). En cada ventana cuentan las propuestas cuya vela de señal empieza en el entrenamiento y cuyo resultado se conoce antes de que termine (la **purga** de la spec §5).
- **Herramienta:** `cfds_backtest::proposals` y el ejemplo `propuestas_por_mes`:

  ```text
  DATABASE_URL=... cargo run --release -p cfds-backtest --example propuestas_por_mes -- [US500] [desde] [hasta]
  ```

  Con los 16 años tarda varios minutos (en release). Hay un test que comprueba que, sin costos ni límites de la cuenta, sus propuestas son exactamente las operaciones del backtest, con la misma etiqueta.

## Advertencias

1. **Sin costos ni P/L.** TP1 y el stop se miden desde la apertura de `t+1` a precio medio. En el backtest, la entrada paga medio spread y slippage, así que la etiqueta puede cambiar en algún caso límite.
2. **Sin kill switch ni pérdida diaria.** La cuenta queda siempre en su capital inicial: las propuestas que el backtest no abriría por un kill switch (hasta 20 días hábiles cada vez, #277) o por la pérdida del día **sí** se cuentan acá. En la corrida de ML, el kill switch puede sacar algunas.
3. **Filtro de noticias de alto impacto (§7.2) sin datos:** no hay calendario económico guardado antes de que empezara la ingesta (ADR 0010, sin histórico). En vivo, ese filtro no deja abrir cerca de los eventos: con el calendario, habría menos propuestas.
4. **Sin ajuste de noticias del puntaje (§9)** (no hay interpretaciones del período) **ni filtro de spread (§7.3)** (no hay spread medido del CFD para las velas de futuros), igual que el backtest con futuros.
5. **Futuro, no CFD:** el precio del futuro tiene otra base que el CFD de IG y cotiza casi 23 h; el horario de entrada es el del CFD.
6. **Variables de ML faltantes:** la spec §3 saca del entrenamiento las propuestas sin todas sus variables (por ejemplo, el tercio de volatilidad necesita 3 años de historia diaria). Las primeras ventanas podrían perder propuestas por eso; este conteo no lo descuenta.
7. **El tramo fuera de muestra se contó y nada más.** No se miró su tasa ni se usó para ajustar nada.
8. **Feriados extendidos a 2011:** `config/holidays/` cubría desde 2021. Se regeneró con el mismo script y la misma versión de `exchange_calendars` desde 2011; de 2021 en adelante queda igual.

## Resultados

Tablas tal como las imprime el ejemplo (una posición a la vez, salvo donde dice "toda vela con señal": cada vela con señal, aunque la anterior siga abierta, solo como referencia). "Sin resolver" son las que terminan sin salida al final de los datos.

- Velas de 15m: 370610 (del 2011-01-02 23:00:00 UTC al 2026-11-23 00:00:00 UTC); de 1h: 90442. Datos sospechosos: 222; errores de datos: 214; vencimientos: 63.
- **Una posición a la vez (como el backtest):** 3942 propuestas; y = 1: 1075, y = 0: 2867, sin resolver: 0 (27,2 % de y = 1).
- **Toda vela con señal (se superponen):** 8536 propuestas; y = 1: 2203, y = 0: 6333.
- Fuera de muestra (desde 2025-11-01 00:00:00 UTC): 234 propuestas, solo contadas.

### Por año (una posición a la vez)

| Año | Propuestas | Largos | Cortos | y = 1 | y = 0 | Sin resolver | % y = 1 | Por mes | Toda vela con señal |
|---|---|---|---|---|---|---|---|---|---|
| 2011 | 216 | 112 | 104 | 77 | 139 | 0 | 35,6 % | 18,0 | 375 |
| 2012 | 248 | 146 | 102 | 58 | 190 | 0 | 23,3 % | 20,7 | 546 |
| 2013 | 232 | 144 | 88 | 58 | 174 | 0 | 25,0 % | 19,3 | 466 |
| 2014 | 234 | 132 | 102 | 61 | 173 | 0 | 26,0 % | 19,5 | 524 |
| 2015 | 249 | 125 | 124 | 68 | 181 | 0 | 27,3 % | 20,8 | 547 |
| 2016 | 251 | 138 | 113 | 66 | 185 | 0 | 26,2 % | 20,9 | 518 |
| 2017 | 237 | 139 | 98 | 57 | 180 | 0 | 24,0 % | 19,8 | 558 |
| 2018 | 289 | 151 | 138 | 86 | 203 | 0 | 29,7 % | 24,1 | 605 |
| 2019 | 247 | 153 | 94 | 57 | 190 | 0 | 23,0 % | 20,6 | 552 |
| 2020 | 259 | 176 | 83 | 77 | 182 | 0 | 29,7 % | 21,6 | 529 |
| 2021 | 244 | 142 | 102 | 72 | 172 | 0 | 29,5 % | 20,3 | 562 |
| 2022 | 254 | 120 | 134 | 78 | 176 | 0 | 30,7 % | 21,2 | 514 |
| 2023 | 277 | 150 | 127 | 74 | 203 | 0 | 26,7 % | 23,1 | 587 |
| 2024 | 266 | 155 | 111 | 72 | 194 | 0 | 27,0 % | 22,2 | 678 |
| 2025 | 234 | 135 | 99 | 62 | 172 | 0 | 26,4 % | 19,5 | 530 |
| 2026 | 205 | 106 | 99 | 52 | 153 | 0 | 25,3 % | 18,6 | 445 |

### Por mes (una posición a la vez; propuestas / y = 1)

| Año | ene | feb | mar | abr | may | jun | jul | ago | sep | oct | nov | dic |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2011 | 13/2 | 15/7 | 12/4 | 20/7 | 20/6 | 20/8 | 21/7 | 25/9 | 19/7 | 19/7 | 20/9 | 12/4 |
| 2012 | 18/1 | 20/2 | 15/4 | 22/3 | 27/5 | 16/7 | 19/6 | 26/3 | 17/0 | 32/12 | 23/10 | 13/5 |
| 2013 | 21/8 | 14/2 | 15/2 | 22/9 | 18/4 | 16/7 | 24/5 | 25/5 | 22/3 | 22/5 | 20/3 | 13/5 |
| 2014 | 19/5 | 19/4 | 19/5 | 17/5 | 25/6 | 14/6 | 14/3 | 20/5 | 16/1 | 34/11 | 20/2 | 17/8 |
| 2015 | 17/5 | 19/3 | 17/2 | 27/6 | 16/5 | 22/5 | 21/3 | 24/8 | 19/7 | 24/9 | 23/9 | 20/6 |
| 2016 | 18/7 | 26/6 | 18/5 | 24/3 | 20/5 | 19/6 | 23/7 | 26/7 | 20/3 | 23/5 | 19/7 | 15/5 |
| 2017 | 23/3 | 24/8 | 20/8 | 20/3 | 23/1 | 25/5 | 12/3 | 26/6 | 15/5 | 16/8 | 21/4 | 12/3 |
| 2018 | 29/9 | 20/6 | 26/7 | 24/6 | 29/6 | 18/7 | 23/5 | 32/9 | 14/4 | 40/13 | 17/9 | 17/5 |
| 2019 | 24/8 | 26/5 | 21/3 | 23/4 | 22/5 | 19/5 | 18/7 | 13/3 | 22/4 | 28/6 | 21/5 | 10/2 |
| 2020 | 28/10 | 16/7 | 11/4 | 16/6 | 16/2 | 17/8 | 33/7 | 28/8 | 20/3 | 28/9 | 28/9 | 18/4 |
| 2021 | 21/4 | 17/3 | 26/6 | 15/8 | 12/2 | 21/6 | 18/7 | 22/9 | 20/9 | 27/7 | 26/5 | 19/6 |
| 2022 | 28/5 | 21/5 | 19/5 | 23/5 | 24/10 | 18/5 | 16/6 | 32/11 | 24/8 | 13/6 | 23/7 | 13/5 |
| 2023 | 21/6 | 30/6 | 24/6 | 21/7 | 24/3 | 22/10 | 23/8 | 25/9 | 20/4 | 21/6 | 25/6 | 21/3 |
| 2024 | 22/7 | 20/5 | 14/4 | 29/8 | 28/6 | 16/4 | 22/11 | 36/9 | 15/4 | 27/4 | 16/3 | 21/7 |
| 2025 | 17/3 | 25/8 | 16/4 | 16/3 | 20/5 | 20/6 | 17/2 | 26/6 | 20/6 | 28/7 | 14/8 | 15/4 |
| 2026 | 10/2 | 19/4 | 21/5 | 27/8 | 14/5 | 19/6 | 27/3 | 13/4 | 16/2 | 26/7 | 13/6 | — |

### Por dirección

| Dirección | Propuestas | y = 1 | y = 0 | Sin resolver | % y = 1 |
|---|---|---|---|---|---|
| largo | 2224 | 589 | 1635 | 0 | 26,4 % |
| corto | 1718 | 486 | 1232 | 0 | 28,2 % |

### Por gatillos

| Gatillos | Propuestas | y = 1 | y = 0 | Sin resolver | % y = 1 |
|---|---|---|---|---|---|
| patrón de velas | 976 | 284 | 692 | 0 | 29,0 % |
| rechazo en nivel | 975 | 258 | 717 | 0 | 26,4 % |
| patrón de velas + rechazo en nivel | 706 | 204 | 502 | 0 | 28,8 % |
| rechazo en nivel + sweep | 604 | 167 | 437 | 0 | 27,6 % |
| patrón de velas + rechazo en nivel + sweep | 224 | 50 | 174 | 0 | 22,3 % |
| doble techo/suelo + rechazo en nivel | 111 | 26 | 85 | 0 | 23,4 % |
| patrón de velas + sweep | 110 | 30 | 80 | 0 | 27,2 % |
| sweep | 83 | 22 | 61 | 0 | 26,5 % |
| doble techo/suelo | 74 | 19 | 55 | 0 | 25,6 % |
| patrón de velas + doble techo/suelo + rechazo en nivel | 41 | 6 | 35 | 0 | 14,6 % |
| patrón de velas + doble techo/suelo | 18 | 2 | 16 | 0 | 11,1 % |
| doble techo/suelo + rechazo en nivel + sweep | 15 | 5 | 10 | 0 | 33,3 % |
| patrón de velas + doble techo/suelo + rechazo en nivel + sweep | 4 | 1 | 3 | 0 | 25,0 % |
| doble techo/suelo + sweep | 1 | 1 | 0 | 0 | 100,0 % |

### Por puntaje

| Puntaje | Propuestas | y = 1 | y = 0 | Sin resolver | % y = 1 |
|---|---|---|---|---|---|
| 5 | 3129 | 885 | 2244 | 0 | 28,2 % |
| 6 | 635 | 154 | 481 | 0 | 24,2 % |
| 7 | 146 | 29 | 117 | 0 | 19,8 % |
| 8 | 31 | 7 | 24 | 0 | 22,5 % |
| 9 | 1 | 0 | 1 | 0 | 0,0 % |

### Por motivo

| Motivo | Propuestas | y = 1 | y = 0 | Sin resolver | % y = 1 |
|---|---|---|---|---|---|
| stop (y = 0) | 2304 | 0 | 2304 | 0 | 0,0 % |
| TP1 (y = 1) | 1075 | 1075 | 0 | 0 | 100,0 % |
| salida por tiempo (y = 0) | 303 | 0 | 303 | 0 | 0,0 % |
| cierre forzado (y = 0) | 260 | 0 | 260 | 0 | 0,0 % |

### Rechazos del riesgo (señales candidatas que no pasan)

| Motivo | Una posición a la vez | Toda vela con señal |
|---|---|---|
| BelowMinDealSize | 1 | 1 |
| StopTooFar | 2622 | 4746 |

### Ventanas walk-forward (24 meses de entrenamiento; mínimo 300 y 50 por clase)

| Entrenamiento | Propuestas | y = 1 | y = 0 | ¿Alcanza? |
|---|---|---|---|---|
| 2011-02-01 a 2013-02-01 | 471 | 141 | 330 | sí |
| 2011-05-01 a 2013-05-01 | 475 | 136 | 339 | sí |
| 2011-08-01 a 2013-08-01 | 473 | 131 | 342 | sí |
| 2011-11-01 a 2013-11-01 | 479 | 121 | 358 | sí |
| 2012-02-01 a 2014-02-01 | 481 | 120 | 361 | sí |
| 2012-05-01 a 2014-05-01 | 478 | 124 | 354 | sí |
| 2012-08-01 a 2014-08-01 | 470 | 122 | 348 | sí |
| 2012-11-01 a 2014-11-01 | 465 | 124 | 341 | sí |
| 2013-02-01 a 2015-02-01 | 462 | 116 | 346 | sí |
| 2013-05-01 a 2015-05-01 | 474 | 114 | 360 | sí |
| 2013-08-01 a 2015-08-01 | 475 | 111 | 364 | sí |
| 2013-11-01 a 2015-11-01 | 473 | 122 | 351 | sí |
| 2014-02-01 a 2016-02-01 | 482 | 131 | 351 | sí |
| 2014-05-01 a 2016-05-01 | 495 | 131 | 364 | sí |
| 2014-08-01 a 2016-08-01 | 504 | 134 | 370 | sí |
| 2014-11-01 a 2016-11-01 | 502 | 131 | 371 | sí |
| 2015-02-01 a 2017-02-01 | 506 | 132 | 374 | sí |
| 2015-05-01 a 2017-05-01 | 507 | 140 | 367 | sí |
| 2015-08-01 a 2017-08-01 | 508 | 136 | 372 | sí |
| 2015-11-01 a 2017-11-01 | 498 | 131 | 367 | sí |
| 2016-02-01 a 2018-02-01 | 499 | 125 | 374 | sí |
| 2016-05-01 a 2018-05-01 | 501 | 130 | 371 | sí |
| 2016-08-01 a 2018-08-01 | 509 | 130 | 379 | sí |
| 2016-11-01 a 2018-11-01 | 526 | 141 | 385 | sí |
| 2017-02-01 a 2019-02-01 | 527 | 148 | 379 | sí |
| 2017-05-01 a 2019-05-01 | 532 | 140 | 392 | sí |
| 2017-08-01 a 2019-08-01 | 532 | 149 | 383 | sí |
| 2017-11-01 a 2019-11-01 | 538 | 143 | 395 | sí |
| 2018-02-01 a 2020-02-01 | 535 | 144 | 391 | sí |
| 2018-05-01 a 2020-05-01 | 508 | 142 | 366 | sí |
| 2018-08-01 a 2020-08-01 | 504 | 141 | 363 | sí |
| 2018-11-01 a 2020-11-01 | 494 | 135 | 359 | sí |
| 2019-02-01 a 2021-02-01 | 503 | 130 | 373 | sí |
| 2019-05-01 a 2021-05-01 | 491 | 135 | 356 | sí |
| 2019-08-01 a 2021-08-01 | 483 | 133 | 350 | sí |
| 2019-11-01 a 2021-11-01 | 489 | 145 | 344 | sí |
| 2020-02-01 a 2022-02-01 | 503 | 144 | 359 | sí |
| 2020-05-01 a 2022-05-01 | 523 | 142 | 381 | sí |
| 2020-08-01 a 2022-08-01 | 515 | 146 | 369 | sí |
| 2020-11-01 a 2022-11-01 | 507 | 150 | 357 | sí |
| 2021-02-01 a 2023-02-01 | 498 | 152 | 346 | sí |
| 2021-05-01 a 2023-05-01 | 515 | 154 | 361 | sí |
| 2021-08-01 a 2023-08-01 | 533 | 160 | 373 | sí |
| 2021-11-01 a 2023-11-01 | 529 | 153 | 376 | sí |
| 2022-02-01 a 2024-02-01 | 525 | 154 | 371 | sí |
| 2022-05-01 a 2024-05-01 | 524 | 155 | 369 | sí |
| 2022-08-01 a 2024-08-01 | 533 | 156 | 377 | sí |
| 2022-11-01 a 2024-11-01 | 542 | 148 | 394 | sí |
| 2023-02-01 a 2025-02-01 | 539 | 143 | 396 | sí |
| 2023-05-01 a 2025-05-01 | 521 | 139 | 382 | sí |
| 2023-08-01 a 2025-08-01 | 509 | 131 | 378 | sí |

51 de 51 ventanas alcanzan el mínimo.

### Meses de entrenamiento necesarios (antes del fuera de muestra)

| Meses | Ventanas | Alcanzan | Mediana de propuestas | Mínimo de y = 1 |
|---|---|---|---|---|
| 6 | 172 | 0 | 126 | 19 |
| 12 | 166 | 0 | 251 | 51 |
| 18 | 160 | 160 | 380 | 84 |
| 24 | 154 | 154 | 506 | 111 |
| 36 | 142 | 142 | 759 | 173 |
| 48 | 130 | 130 | 1019 | 241 |
| 60 | 118 | 118 | 1276 | 310 |
| 72 | 106 | 106 | 1527 | 368 |
| 96 | 82 | 82 | 2030 | 511 |
| 120 | 58 | 58 | 2528 | 658 |
