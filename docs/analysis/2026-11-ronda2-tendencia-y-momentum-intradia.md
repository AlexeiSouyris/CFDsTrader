# Ronda 2 de investigación: tendencia mensual (A) y momentum intradía (B)

Issue #486 · 24-11-2026 · propuesta y regla: `docs/analysis/2026-11-propuesta-investigacion.md` (#466) · pre-registro: `docs/analysis/2026-11-ronda2-preregistro.md` (commits del 24-11-2026 a las 15:08 y a las 15:19, hora de autor de Madrid; los dos **antes** de la primera corrida, que fue a las 15:20. Se publicaron en la rama como `f9eadb7` y `1b599bf`; después del rebase sobre `main` son `4b2a116` y `d6083c3`, con el mismo contenido y la misma hora de autor).

> **Es un estudio, no un resultado oficial.** El tipo de referencia de la financiación de US500, US100 y US30 sigue pendiente con IG: se usan los dos escenarios de #421, solo en memoria. **El fuera de muestra (desde el 01-11-2025) no se cargó ni se miró.** Las velas son de futuros, no del CFD. Los resultados pasados no prometen ningún resultado futuro, y nada de esto es una recomendación de operar.

## Veredicto

| Familia | Escenario EUR (EONIA→€STR) | Escenario USD (EFFR→SOFR) | Veredicto |
|---|---|---|---|
| **A. Tendencia mensual** | pasa el 3 y el 4; no pasa el 1, el 2 ni el 5 | igual | **NO PASA. Se cierra.** |
| **B. Momentum intradía** | no pasa ninguno (1 a 4) | igual (sin financiación: es intradía) | **NO PASA. Se cierra.** |

- **A da un resultado positivo pero chico y no significativo:** +0,080 % por mes (EUR) y +0,070 % (USD), con p = 0,14 y 0,18 (hacía falta < 0,0125). El Sharpe deflactado es 0,030 y 0,022 (hacía falta ≥ 0,95). Las dos mitades y los tres índices son positivos y aguanta los costos × 1,5, pero su Sharpe (≈ 0,30 anual) es **un tercio** del de comprar y mantener el CFD en los mismos meses (≈ 0,90 anual): no pasa el criterio 5.
- **B pierde** en la cartera y en los tres índices: −2,0 % por mes en promedio (t = −11,0). Antes de costos tampoco hay ventaja: −0,01 R por operación en US500, −0,01 R en US100 y −0,03 R en US30. Los costos se llevan 0,08–0,15 R por operación.
- Según la regla de #466 §4.4, las dos familias se **cierran**: no se prueban variantes de A ni de B sobre estos datos.
- No hay candidato para el fuera de muestra: queda intacto.
- Con esta ronda **se agotó el presupuesto de error del programa** (5 %: 2,5 % en la ronda 1 y 2,5 % en esta). Cualquier prueba nueva necesita una decisión nueva del dueño (pre-registro §2.1).

## Método (resumen; el detalle está en el pre-registro)

- **Datos:** velas de 15m de los futuros ES, NQ e YM de Databento (`--fuente futuros`), del 01-01-2011 al **01-11-2025, sin cargar nada del fuera de muestra**. Para saber dónde empieza se leyó solo la hora de la última vela guardada.
- **Walk-forward** de `config/data_split.toml`: 51 ventanas de validación de 3 meses, 02-2013 → 10-2025 (153 meses). Las familias no tienen parámetros que ajustar: las señales se calculan una vez con funciones puras sobre la serie de investigación, y en cada ventana se ejecutan las **decididas** en ella, con una cuenta nueva.
- **A:** revisión al cierre de la anteúltima vela de 15m del último día hábil del mes (o la primera decisión permitida después). Largo si el retorno de 12 meses (encadenado en los vencimientos) es > 0, corto si es < 0. Cierra cada viernes y reabre el lunes; en cada revisión cierra y vuelve a abrir. Stop a 3 × ATR(20) **diario**, que el riesgo mide con ese mismo ATR.
- **B:** señal = signo del retorno desde el cierre del contado del día anterior hasta las 10:00 ET (encadenado en los vencimientos); decisión a las 15:30 ET, entrada en la apertura de la vela de 15:30 y salida a las 16:00 ET. Stop a 2 × ATR(14) de 15m. Sin el último día hábil de la semana.
- **Cuenta:** una por índice, de 100 000 € (pre-registro §1), con todo el riesgo de `config/risk.toml` sin cambios (1 % por operación, stop entre 0,5 y 3 ATR, pérdida diaria 3 %, kill switch a −15 % que en backtest se reactiva a los 20 días hábiles). **Cartera** = promedio de los retornos mensuales de los tres índices.
- **Costos:** spread típico y slippage de config (`fill_price`), comisión, financiación de IG noche por noche con el escenario y **menos la base del futuro** (#470, EFFR→SOFR), las mismas funciones que el backtest de la estrategia en uso. Estrés: todos los costos × 1,5 sobre las mismas operaciones.
- **Comprar y mantener (criterio 5 de A):** largo de 1 × el capital en cada ventana, mantenido los fines de semana y encadenado sin costo en los vencimientos, con spread y slippage en la entrada y la salida de cada ventana y la financiación de cada noche menos la base.
- **Base real en solo lectura:** conexión con `default_transaction_read_only=on`; no se escribió nada.
- **Herramienta:** ejemplo `ronda2` (módulo `cfds_backtest::research`, este PR), aparte de la estrategia en uso: ni el runner ni el paper lo usan, y `config/strategy.toml` y `config/risk.toml` no cambian. Parámetros en `config/research.toml` (secciones `[trend]`, `[intraday_momentum]` y `[rule_round2]`).

## Resultados por familia

### A. Tendencia mensual

| Índice | Escenario | Operaciones | Largos / cortos | Salen en el stop | R medio antes de costos | Costo medio | R neto medio | Aciertos | Retorno mensual medio | Con costos × 1,5 |
|---|---|---|---|---|---|---|---|---|---|---|
| US500 | EUR | 660 | 565 / 95 | 44 | +0,028 R | 0,020 R | +0,009 R | 53,3 % | +0,037 % | −0,006 % |
| US500 | USD | 660 | 565 / 95 | 44 | +0,028 R | 0,023 R | +0,006 R | 53,2 % | +0,026 % | −0,023 % |
| US100 | EUR | 669 | 603 / 66 | 38 | +0,048 R | 0,013 R | +0,035 R | 56,4 % | +0,152 % | +0,123 % |
| US100 | USD | 669 | 603 / 66 | 38 | +0,048 R | 0,016 R | +0,033 R | 56,2 % | +0,143 % | +0,109 % |
| US30 | EUR | 661 | 550 / 111 | 40 | +0,026 R | 0,014 R | +0,012 R | 52,5 % | +0,052 % | +0,022 % |
| US30 | USD | 661 | 550 / 111 | 40 | +0,026 R | 0,016 R | +0,009 R | 52,3 % | +0,041 % | +0,006 % |

- Son ~4,3 operaciones por mes y por índice, porque el cierre del viernes parte cada mes en semanas (pre-registro §2.2). El riesgo no rechazó ninguna.
- La señal estuvo larga la mayor parte del tiempo: 133, 138 y 129 de los 153 meses (US500, US100, US30); corta en 20, 15 y 24.
- R mediano en puntos: 106,2 (US500), 347,4 (US100) y 889,2 (US30).
- **Con 10 000 € no se podría operar la mayoría:** 52 % de las operaciones en US500, 64 % en US100 y 72 % en US30 quedarían debajo del tamaño mínimo de IG.

| Criterio (cartera, 153 meses) | EUR | USD | ¿Pasa? |
|---|---|---|---|
| 1. Prueba t unilateral, nivel 1,25 % | media +0,080 % por mes (desvío 0,93 %), t = 1,07, p = 0,142 | media +0,070 %, t = 0,94, p = 0,175 | no |
| 2. Sharpe deflactado ≥ 0,95 (N = 352) | Sharpe mensual 0,087 (≈ 0,30 anual), SR0 = 0,239, DSR = 0,030 | Sharpe 0,076 (≈ 0,26), DSR = 0,022 | no |
| 3. Consistencia | 2013–2019: +0,032 % por mes; 2020–2025: +0,138 %; índices > 0: 3 de 3 | +0,020 % / +0,129 %; 3 de 3 | sí |
| 4. Costos × 1,5 | +0,046 % por mes | +0,031 % | sí |
| 5. Sharpe ≥ comprar y mantener | 0,087 frente a 0,261 (≈ 0,90 anual; +1,10 % por mes) | 0,076 frente a 0,237 (≈ 0,82; +1,00 %) | no |

Comprar y mantener por índice (Sharpe mensual, EUR / USD): US500 0,245 / 0,220, US100 0,293 / 0,273, US30 0,203 / 0,178.

Lectura:
- **Hay un efecto positivo antes de costos** (+0,03 a +0,05 R por operación, 52–56 % de aciertos), del lado que dice la literatura, pero es **chico**: con 1 % de riesgo por operación da unos 0,08 % por mes en la cartera, un Sharpe anual de ~0,3. La regla pedía ~0,63 o más para pasar el criterio 1 (#466 §4.3).
- **Comprar y mantener fue mucho mejor en este período.** El Sharpe no depende del tamaño de la posición, así que la diferencia no es por el 1 % de riesgo: en 2013–2025 la señal estuvo larga casi siempre (como comprar y mantener), y los meses en corto, los stops y los cierres de cada viernes no agregaron nada que pague lo que se perdió.
- Por año (suma de los retornos mensuales de la cartera, EUR): 2013 +3,0 %, 2014 +0,9 %, 2015 −3,6 %, 2016 −4,8 %, 2017 +5,6 %, 2018 +0,1 %, 2019 +1,3 %, 2020 +2,2 %, 2021 +3,3 %, 2022 −1,6 %, 2023 −0,8 %, 2024 +3,8 %, 2025 (a octubre) +2,7 %. Los años sin tendencia clara (2015–2016, 2022–2023) son los negativos.
- Coincide con la advertencia de #466 §3.A: la evidencia fuerte del seguimiento de tendencia es en carteras de decenas de futuros; acá son tres índices de EE. UU. muy correlacionados.

### B. Momentum intradía

Los dos escenarios dan exactamente lo mismo: las operaciones cierran a las 16:00 ET, antes del corte de financiación (23:00 de Madrid).

| Índice | Operaciones | Largos / cortos | Salen en el stop | Rechazadas por el riesgo | R medio antes de costos | Costo medio | R neto medio | Aciertos | Retorno mensual medio | Con costos × 1,5 |
|---|---|---|---|---|---|---|---|---|---|---|
| US500 | 2441 | 1327 / 1114 | 243 (10 %) | 3 drawdown | −0,010 R | 0,149 R | −0,159 R | 38,2 % | −2,50 % | −3,64 % |
| US100 | 2478 | 1382 / 1096 | 174 (7 %) | 1 drawdown | −0,013 R | 0,098 R | −0,111 R | 41,8 % | −1,77 % | −2,54 % |
| US30 | 2466 | 1301 / 1165 | 265 (11 %) | — | −0,031 R | 0,079 R | −0,110 R | 42,4 % | −1,75 % | −2,37 % |

R mediano en puntos: 9,1 (US500), 33,2 (US100) y 74,0 (US30).

| Criterio (cartera, 153 meses) | Resultado (EUR y USD) | ¿Pasa? |
|---|---|---|
| 1. Prueba t unilateral, nivel 1,25 % | media −2,007 % por mes (desvío 2,25 %), t = −11,03, p = 1,000 | no |
| 2. Sharpe deflactado ≥ 0,95 (N = 352) | Sharpe mensual −0,89 (≈ −3,1 anual), SR0 = 0,239, DSR = 0,000 | no |
| 3. Consistencia | 2013–2019: −2,89 % por mes; 2020–2025: −0,96 %; índices con media > 0: 0 de 3 | no |
| 4. Costos × 1,5 | −2,85 % por mes | no |

Lectura:
- **La última media hora no siguió a la primera** en estos futuros de 2013 a 2025: el R antes de costos es negativo en los tres índices.
- Aunque hubiera tenido el signo esperado, la vara era alta: con un stop de 2 ATR de 15m, el costo es 0,08–0,15 R por operación, y #466 §3.B ya estimaba que hacía falta acertar la dirección más del 55–59 % de las veces.
- Por año (suma de los retornos mensuales de la cartera): de −34 % a −45 % en 2013–2017, −22 % y −27 % en 2018–2019, y de −4 % a −28 % en 2020–2025. Pierde todos los años.

## Descriptivo, sin voto: US2000 con costos supuestos

Costos «proporcionales» de #421 (supuestos, no datos de IG). Validación 08-2019 → 10-2025: 25 ventanas, 75 meses.

| Familia | Escenario | Operaciones | R antes de costos | Costo | R neto | Retorno mensual medio | t | p | Comprar y mantener (Sharpe mensual) |
|---|---|---|---|---|---|---|---|---|---|
| A | EUR | 339 (201 largos, 138 cortos) | −0,017 | 0,011 | −0,028 | −0,13 % | −1,23 | 0,89 | 0,097 (A: −0,142) |
| A | USD | 339 | −0,017 | 0,012 | −0,029 | −0,13 % | −1,25 | 0,89 | 0,079 (A: −0,145) |
| B | EUR y USD (iguales) | 1216 | −0,022 | 0,079 | −0,101 | −1,62 % | −6,53 | 1,00 | — |

Va en la misma dirección que la cartera para B; para A, en US2000 ni siquiera hay efecto antes de costos.

## Registro de corridas (pre-registro §2.7)

Todas con el binario de este PR (compilado del commit de las 15:19), contra la base en solo lectura, sin cargar velas desde el 01-11-2025:

| # | Hora (24-11-2026, Madrid; todas después del commit de las 15:19) | Comando (`target/release/examples/ronda2 …`, desde `engine/`) | Nota |
|---|---|---|---|
| 1 | 15:20 | `US500 US100 US30 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv --salida … --operaciones …` | primera corrida |
| 2 | 15:21 | `US500 US100 US30 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv --salida … --operaciones …` | |
| 3 | 15:21 | `US2000 --descriptivo --supuestos ../docs/analysis/2026-11-us2000-supuestos-proporcional.toml --escenario-financiacion EUR=…` | descriptivo |
| 4 | 15:21 | lo mismo con `USD=…` | descriptivo |

Ninguna corrida se repitió. Para el Sharpe deflactado de pruebas futuras, esta ronda suma **2 pruebas** (A y B): el total registrado queda en **352**. Las corridas 3 y 4 son descriptivas.

Los retornos mensuales de cada índice (con y sin el estrés de costos) y los de comprar y mantener quedan en `docs/analysis/2026-11-ronda2-retornos-eur.csv` y `docs/analysis/2026-11-ronda2-retornos-usd.csv`.

## Verificación

- Se revisaron a mano operaciones de las dos familias en el registro exportado:
  - A, US500, lunes 04-02-2013: reapertura a las 09:45 ET (la revisión de enero se decidió el 31-01, en el entrenamiento, y no se ejecutó), salida el viernes 08-02 a las 16:00 ET. Stop = 1499 − 3 × 12,52 (ATR diario); tamaño 26,63 = 1000 € / 37,56 puntos (1 % de riesgo); 4 noches de financiación.
  - A, jueves 28-02-2013: revisión al cierre de 15:30–15:45 ET y entrada a las 15:45, salida el viernes 01-03 a las 16:00 ET. Lunes 11-03-2013, después del cambio de horario de EE. UU.: 09:45 EDT = 13:45 UTC. Viernes 15-03-2013 (vencimiento): la pata se cierra a las 09:30 ET, antes del cambio de contrato.
  - B, lunes 04-02-2013: decisión a las 15:30 EST, salida a las 16:00, costo = 1,1 puntos × tamaño.
- 19 tests nuevos (`engine/crates/backtest/tests/research_round2.rs`) y los 22 de la ronda 1 sin cambios:
  - A: revisión de fin de mes (día normal, fin de mes en viernes, feriado), primer año sin 12 meses de historia, señal corta con el stop a 3 ATR diarios, cierre de cada viernes y reapertura (con el lunes feriado), el salto del vencimiento no da vuelta la señal, revisión base faltante, el stop en el tope de 3 ATR pasa el riesgo, **no repintado**;
  - B: largo y corto, cambio de horario (EST y EDT), último día de la semana (viernes y jueves antes de un viernes feriado), cierre anticipado, vela faltante o retorno cero, vencimiento dentro de la ventana de la señal, **no repintado**;
  - retorno encadenado en los vencimientos y comprar y mantener (costos exactos de un mes, vencimiento encadenado, sigue al precio).

## Desvíos y límites

1. **Redondeo del ATR de A** (pre-registro §3.1, fijado antes de correr): el ATR diario se redondea a 8 decimales para que el stop quede exactamente a 3 ATR y el riesgo no lo rechace por un redondeo. No cambia ninguna regla.
2. **Viernes de vencimiento en A:** la pata se cierra antes del cambio de contrato (09:30 ET del tercer viernes) y las marcas de la preparación común no dejan decidir el resto de ese día: se reabre el lunes. Son 4 días por año; la ronda 1 tenía la misma preparación.
3. **Futuros, no el CFD.** La base y el horario no son los del CFD de IG, y el spread es el típico de config, no uno medido (#267). El tipo de referencia de la financiación sigue pendiente; los dos escenarios dan el mismo veredicto.
4. **Comprar y mantener** se valúa en la moneda del instrumento (sin el cambio EUR/USD) y paga el spread de entrada y salida en cada ventana de 3 meses (más caro que el CFD real, que se compra una vez). Las dos cosas lo perjudican o son neutras; no cambian el criterio 5, que A no pasa por un factor de tres.
5. **Capital de 100 000 €** para que el tamaño mínimo no decida la prueba. Con 10 000 €, A no se podría operar en la mayoría de los casos.
6. **Sin calendario de noticias** para el período (§7.2), igual que en los estudios anteriores.

## Para el dueño

- A y B **no pasan** la regla pre-registrada y quedan cerradas para estos datos. No se activa nada, no se toca la estrategia en uso y el fuera de muestra sigue intacto.
- Con las rondas 1 y 2, las cuatro familias elegidas (C, D1, A y B) quedaron cerradas y **el presupuesto de error de 5 % del programa está agotado**. Seguir probando reglas sobre estos mismos datos ya no tiene un nivel de significancia pre-registrado.
- Dos datos para decidir cómo seguir:
  - los dos efectos con el signo esperado (D1 y A) son del orden de 0,03–0,05 R por operación antes de costos, lejos de lo que pide la regla;
  - en el período 2013–2025, comprar y mantener los índices tuvo un Sharpe tres veces mayor que la mejor de las cuatro familias. Esto describe el pasado: no promete nada para el futuro y no es una recomendación de operar.
