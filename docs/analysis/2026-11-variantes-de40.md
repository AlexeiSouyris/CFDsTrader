# Estudio: variantes de la estrategia del DE40 en backtest (futuros)

Issue #359 · 24-11-2026 · **solo estudio**: no cambia `config/strategy.toml` ni ningún parámetro en uso. Presenta resultados y una propuesta; **decide el dueño**, en otro Issue.

## Lo primero: qué se pudo medir y qué no

1. **El walk-forward de config no se puede correr en el DE40.** `config/data_split.toml` pide 24 meses de entrenamiento + 3 de prueba antes de los 12 del fuera de muestra: **39 meses**. Las velas de `DE40.FUT` van del 11-03-2025 al 30-10-2026 (**19 meses**). No se achicó la config (es del dueño).
2. **Lo que queda en muestra son 6 meses.** El fuera de muestra se calcula igual que en `plan`: los últimos 12 meses desde el mes de la última vela, o sea **desde el 01-10-2025**. Antes quedan 2 bloques de 3 meses (`validation_months`, con paso `step_months`): **abr–jun 2025** y **jul–sep 2025**. Cada bloque se evalúa aparte, desde el capital inicial y con 30 días de calentamiento (`config/evaluation.toml`). No hay entrenamiento: las variantes no ajustan nada, vienen fijadas de antes.
3. **El fuera de muestra no se usó acá, pero el diagnóstico de #324 ya lo había mirado.** Ese diagnóstico corrió del 01-03-2025 al 01-11-2026: 13 de sus 19 meses son el tramo que hoy es fuera de muestra, y de ahí salen las hipótesis (horas, stops y gatillos). Por lo tanto:
   - los 6 meses en muestra **no son una prueba independiente**: también formaron parte de la muestra que sugirió las variantes;
   - el fuera de muestra del DE40 **ya no sirve para confirmar estas variantes concretas**: se eligieron mirándolo.
4. **El control en otros índices no se pudo hacer.** La idea era probar las variantes que no dependen del horario de Xetra (V1, V2, V4, V6 y V10) en US500, US100 y US30, que tienen 5 años de futuros y sí permiten el walk-forward de config. El backtest se niega a correr: el tipo de referencia de la financiación de esos CFD figura como **pendiente** en `config/instruments.toml` ("Confirmar con IG"). No se inventó.

## Método

- **Herramienta:** ejemplo nuevo `variantes` (`engine/crates/backtest/examples/variantes.rs`). Usa la misma preparación que `backtest_run` y `walk_forward` (`examples/common/prepare.rs`), corre **en memoria**, no guarda nada y se ejecutó con una sesión de PostgreSQL de solo lectura (`default_transaction_read_only=on`).
- **Variantes:** archivo de estudio `docs/analysis/2026-11-variantes-de40.toml`, **escrito antes de correr nada** y sin cambios después. Cada variante es la config de `main` más:
  - overrides de claves que ya existen (mismo mecanismo que la sensibilidad de #303, `override_toml`);
  - horas UTC de la vela de entrada (t+1) en las que no se abre;
  - conjuntos exactos de gatillos con los que no se abre.

  Las dos últimas solo **quitan** entradas: se calcula la señal de la vela t igual que el recorrido (misma `Analysis` del tramo y mismo ajuste de noticias) y, si cae en una hora o un conjunto excluido, en t no se abre. Con la variante base, el resultado es idéntico al de `evaluate_period` (hay test).
- **Comando:**

  ```text
  cargo run --release -p cfds-backtest --example variantes -- \
      DE40 2025-03-01T00:00:00Z 2026-11-01T00:00:00Z 10000 1 \
      docs/analysis/2026-11-variantes-de40.toml --fuente futuros
  ```
- **Costos completos:** spread de config (1,8 puntos, no hay spread medido del CFD para futuros, #267), slippage, comisión (cero en DE40) y financiación noche por noche con el €STR diario guardado.
- **Otros supuestos:**
  - kill switch de backtest que se reactiva a los 20 días hábiles (`config/backtest.toml`, #277);
  - sin noticias (no hay interpretaciones posteriores al corte del modelo en estos meses);
  - el filtro de spread (§7.3) y el de noticias de alto impacto (§7.2) no se aplican, porque no hay datos.
- **Columnas:**
  - "Gana": operaciones con neto > 0, con su IC 95 % de Wilson (z de `config/probability.toml`).
  - "Payoff": ganancia neta media de las ganadoras / pérdida neta media de las perdedoras.
  - "P/L sin costos": a precio medio. "Costos" = P/L sin costos − neto (spread, slippage y financiación).
  - Drawdown: el máximo del tramo sobre la curva diaria. Montos en EUR (la moneda de la cuenta), redondeados.
- **Variantes:** 12 en total: la base y 11 alternativas. Todas corren sobre los bloques de abr–jun y jul–sep de 2025, que caen en horario de verano (Xetra abre a las 07:00 UTC).

| Variante | Qué cambia |
|---|---|
| V0 | nada (config de `main`) |
| V1 | `risk.toml` `stop_distance.min_atr` = 2,5 (stop entre 2,5 y 3 ATR) |
| V2 | `min_atr` = 3 y `max_atr` = 3,5 (stop entre 3 y 3,5 ATR, hoy rechazado por StopTooFar) |
| V3 | no abrir con la vela de entrada a las 07, 08 o 10 UTC |
| V4 | no abrir con el conjunto exacto de gatillos «patrón de velas + rechazo en nivel» |
| V5 a V11 | combinaciones: V1+V3, V1+V4, V3+V4, V1+V3+V4, V2+V3, V2+V4, V2+V3+V4 |

## Resultados (DE40.FUT, abr–sep 2025, en muestra)

### Los dos bloques sumados

| Variante | Op. | Gana | IC 95 % | Payoff | P/L sin costos | Costos | Neto | Peor drawdown | Tramos con neto > 0 |
|---|---|---|---|---|---|---|---|---|---|
| V0 base (config) | 145 | 38,6 % | 31,0–46,7 % | 1,56 | 589 | 737 | −148 | 13,8 % | 1 de 2 |
| V1 stop 2,5–3 ATR | 30 | 43,3 % | 27,3–60,8 % | 1,76 | 693 | 110 | 583 | 6,4 % | 2 de 2 |
| V2 stop 3–3,5 ATR | 26 | 46,1 % | 28,7–64,5 % | 1,38 | 277 | 68 | 209 | 3,7 % | 1 de 2 |
| V3 sin 07, 08 y 10 UTC | 103 | 44,6 % | 35,4–54,2 % | 1,59 | 2328 | 596 | 1731 | 10,0 % | 2 de 2 |
| V4 sin patrón+rechazo | 122 | 36,0 % | 28,0–44,8 % | 1,66 | 171 | 633 | −461 | 11,5 % | 0 de 2 |
| V5 = V1 + V3 | 14 | 57,1 % | 32,5–78,6 % | 2,31 | 844 | 42 | 801 | 2,0 % | 2 de 2 |
| V6 = V1 + V4 | 27 | 40,7 % | 24,5–59,2 % | 1,76 | 458 | 100 | 357 | 6,4 % | 2 de 2 |
| V7 = V3 + V4 | 86 | 43,0 % | 33,0–53,5 % | 1,74 | 2141 | 515 | 1625 | 8,2 % | 2 de 2 |
| V8 = V1 + V3 + V4 | 11 | 54,5 % | 28,0–78,7 % | 2,13 | 604 | 34 | 569 | 2,0 % | 2 de 2 |
| V9 = V2 + V3 | 13 | 53,8 % | 29,1–76,7 % | 1,33 | 314 | 33 | 281 | 2,0 % | 2 de 2 |
| V10 = V2 + V4 | 26 | 46,1 % | 28,7–64,5 % | 1,38 | 277 | 68 | 209 | 3,7 % | 1 de 2 |
| V11 = V2 + V3 + V4 | 12 | 50,0 % | 25,3–74,6 % | 1,20 | 132 | 28 | 104 | 2,0 % | 1 de 2 |

### Por tramo

| Variante | Tramo | Op. | Gana | Payoff | P/L sin costos | Costos | Neto | Drawdown | Retorno | Comprar y mantener |
|---|---|---|---|---|---|---|---|---|---|---|
| V0 base (config) | 2025-04-01 → 2025-07-01 | 66 | 42,4 % | 1,43 | 511 | 293 | 218 | 12,7 % | 2,1 % | 13,4 % |
| V0 base (config) | 2025-07-01 → 2025-10-01 | 79 | 35,4 % | 1,69 | 77 | 444 | −366 | 13,8 % | −3,6 % | −2,1 % |
| V1 stop 2,5–3 ATR | 2025-04-01 → 2025-07-01 | 12 | 50,0 % | 1,59 | 313 | 37 | 275 | 1,9 % | 2,7 % | 13,4 % |
| V1 stop 2,5–3 ATR | 2025-07-01 → 2025-10-01 | 18 | 38,8 % | 1,97 | 379 | 72 | 307 | 6,4 % | 3,0 % | −2,1 % |
| V2 stop 3–3,5 ATR | 2025-04-01 → 2025-07-01 | 14 | 42,8 % | 1,29 | 18 | 35 | −17 | 3,7 % | −0,1 % | 13,4 % |
| V2 stop 3–3,5 ATR | 2025-07-01 → 2025-10-01 | 12 | 50,0 % | 1,49 | 259 | 32 | 226 | 2,3 % | 2,2 % | −2,1 % |
| V3 sin 07, 08 y 10 UTC | 2025-04-01 → 2025-07-01 | 43 | 51,1 % | 1,40 | 1210 | 204 | 1005 | 5,1 % | 10,0 % | 13,4 % |
| V3 sin 07, 08 y 10 UTC | 2025-07-01 → 2025-10-01 | 60 | 40,0 % | 1,77 | 1117 | 391 | 726 | 10,0 % | 7,2 % | −2,1 % |
| V4 sin patrón+rechazo | 2025-04-01 → 2025-07-01 | 50 | 38,0 % | 1,49 | −32 | 212 | −244 | 10,4 % | −2,4 % | 13,4 % |
| V4 sin patrón+rechazo | 2025-07-01 → 2025-10-01 | 72 | 34,7 % | 1,79 | 203 | 421 | −217 | 11,5 % | −2,1 % | −2,1 % |
| V5 = V1 + V3 | 2025-04-01 → 2025-07-01 | 8 | 75,0 % | 4,41 | 725 | 25 | 700 | 0,4 % | 7,0 % | 13,4 % |
| V5 = V1 + V3 | 2025-07-01 → 2025-10-01 | 6 | 33,3 % | 2,62 | 119 | 17 | 101 | 2,0 % | 1,0 % | −2,1 % |
| V6 = V1 + V4 | 2025-04-01 → 2025-07-01 | 9 | 44,4 % | 1,38 | 78 | 28 | 49 | 1,9 % | 0,4 % | 13,4 % |
| V6 = V1 + V4 | 2025-07-01 → 2025-10-01 | 18 | 38,8 % | 1,97 | 379 | 72 | 307 | 6,4 % | 3,0 % | −2,1 % |
| V7 = V3 + V4 | 2025-04-01 → 2025-07-01 | 32 | 50,0 % | 1,58 | 1090 | 157 | 932 | 4,7 % | 9,3 % | 13,4 % |
| V7 = V3 + V4 | 2025-07-01 → 2025-10-01 | 54 | 38,8 % | 1,88 | 1051 | 357 | 693 | 8,2 % | 6,9 % | −2,1 % |
| V8 = V1 + V3 + V4 | 2025-04-01 → 2025-07-01 | 5 | 80,0 % | 3,24 | 485 | 17 | 468 | 0,4 % | 4,6 % | 13,4 % |
| V8 = V1 + V3 + V4 | 2025-07-01 → 2025-10-01 | 6 | 33,3 % | 2,62 | 119 | 17 | 101 | 2,0 % | 1,0 % | −2,1 % |
| V9 = V2 + V3 | 2025-04-01 → 2025-07-01 | 10 | 50,0 % | 1,34 | 163 | 25 | 138 | 2,0 % | 1,3 % | 13,4 % |
| V9 = V2 + V3 | 2025-07-01 → 2025-10-01 | 3 | 66,6 % | 1,18 | 151 | 8 | 142 | 1,3 % | 1,4 % | −2,1 % |
| V10 = V2 + V4 | 2025-04-01 → 2025-07-01 | 14 | 42,8 % | 1,29 | 18 | 35 | −17 | 3,7 % | −0,1 % | 13,4 % |
| V10 = V2 + V4 | 2025-07-01 → 2025-10-01 | 12 | 50,0 % | 1,49 | 259 | 32 | 226 | 2,3 % | 2,2 % | −2,1 % |
| V11 = V2 + V3 + V4 | 2025-04-01 → 2025-07-01 | 9 | 44,4 % | 1,13 | −18 | 20 | −38 | 2,0 % | −0,3 % | 13,4 % |
| V11 = V2 + V3 + V4 | 2025-07-01 → 2025-10-01 | 3 | 66,6 % | 1,18 | 151 | 8 | 142 | 1,3 % | 1,4 % | −2,1 % |

## Lectura

1. **La base, en estos 6 meses, casi empata:** −148 neto con +589 a precio medio. Es mucho mejor que en el diagnóstico completo (−3566). La mayor parte de la pérdida del diagnóstico cae en el tramo que hoy es fuera de muestra.
2. **El horario (V3) es lo único que cambia mucho con muestra razonable.** Saca 42 de 145 operaciones; esas 42 perdían unos −1880 en conjunto. Con V3 los dos bloques son positivos (+1005 y +726) y el win rate pasa de 38,6 % a 44,6 %. **Pero los IC se superponen** (31–47 % contra 35–54 %).
3. **Los stops de 2,5–3 ATR (V1) dan positivo en los dos bloques, con solo 30 operaciones** (12 y 18). El IC del win rate va de 27 % a 61 %: no distingue nada. El drawdown baja porque se opera mucho menos.
4. **Stops de 3–3,5 ATR (V2): 26 operaciones, resultado mixto** (−17 y +226). No hay señal clara.
5. **Sacar «patrón de velas + rechazo» (V4) no mejora: empeora** (−461 contra −148; los dos bloques negativos). En estos meses ese disparador no era el problema, lo que contradice la lectura del diagnóstico sobre la muestra completa. V10 da igual que V2: ninguna señal de 3–3,5 ATR tenía ese disparador.
6. **Las combinaciones con V1 o V2 quedan en 11 a 27 operaciones** en 6 meses, 3 a 18 por bloque. Los porcentajes de V5 y V8 (57 % y 55 %, payoff > 2) salen de 14 y 11 operaciones: son ruido hasta que se demuestre lo contrario.

## Advertencia de sobreajuste (comparaciones múltiples)

- Se compararon **11 variantes contra la base** en 2 bloques. Con 11 intentos, la mejor variante va a verse bien aunque ninguna tenga ventaja real.
  - Con un nivel de 5 % por comparación, la probabilidad de al menos un falso positivo es 1 − 0,95¹¹ ≈ **43 %**.
  - Con la corrección de Bonferroni, cada comparación tendría que pasar al 0,45 %. Ningún IC de esta tabla se separa ni siquiera al 5 %.
- Las variantes **no son independientes**: V5 a V11 se arman con V1 a V4. Sus resultados se repiten.
- Las hipótesis salieron de una muestra (#324) que **incluye estos mismos 6 meses** y el fuera de muestra entero. Que "funcionen" acá es en buena parte circular. El caso más claro es V3: las horas 07, 08 y 10 se eligieron porque perdían, en parte en estos mismos meses.
- **2 bloques, un instrumento, horario de verano solamente, y precios de futuros, no del CFD** (base y horario distintos, ADR 0011).
- **El filtro de horas reduce la exposición.** Parte de la mejora es operar menos en una muestra en la que la base no tenía ventaja.

## Propuesta para el dueño (no es una decisión)

1. **No cambiar ningún parámetro con esta evidencia.** Ninguna variante es estadísticamente distinta de la base y no existe un tramo limpio donde confirmarla.
2. **Si se quiere seguir con una sola hipótesis, la candidata es el horario (V3)**: es la única con muestra razonable (103 operaciones) y positiva en los dos bloques. Para validarla hace falta, en un Issue aparte:
   - **más historia de FDAX anterior a marzo de 2025** (Databento la tiene), para que el walk-forward de config corra y quede un fuera de muestra que nadie haya mirado;
   - o una **decisión explícita del dueño** sobre otra separación para el DE40, registrada antes de mirar resultados.

   En ambos casos conviene fijar de antemano una sola variante (por ejemplo V3), no las 11.
3. **No seguir con V4 («sin patrón + rechazo»)**: en la parte en muestra va en contra de lo que sugería el diagnóstico.
4. **Los stops de 2,5–3 ATR (V1) solo se pueden juzgar con mucha más historia**: dejan unas 5 operaciones por mes.
5. **Para el control en US500, US100 y US30 hay que confirmar con IG el tipo de referencia de la financiación** de esos CFD (está pendiente en `config/instruments.toml`). Con eso, las variantes que no dependen del horario se pueden probar en 5 años con el walk-forward de config, en instrumentos que no sugirieron las hipótesis.
