# Ronda 1 de investigación: ruptura del rango de apertura (C) y cambio de mes (D1)

Issue #472 · 24-11-2026 · propuesta y regla: `docs/analysis/2026-11-propuesta-investigacion.md` (#466) · pre-registro: `docs/analysis/2026-11-ronda1-preregistro.md` (commits del 24-11-2026 a las 06:41 y a las 06:50, hora de autor; los dos **antes** de la primera corrida, que fue a las 06:51. Se publicaron en la rama como `81c6d27` y `3d7bd02`; después del rebase sobre `main` son `4c11bb4` y `cafbf45`).

> **Es un estudio, no un resultado oficial.** El tipo de referencia de la financiación de US500, US100 y US30 sigue pendiente con IG: se usan los dos escenarios de #421, solo en memoria. **El fuera de muestra (desde el 01-11-2025) no se cargó ni se miró.** Las velas son de futuros, no del CFD. Los resultados pasados no prometen ningún resultado futuro, y nada de esto es una recomendación de operar.

## Veredicto

| Familia | Escenario EUR (EONIA→€STR) | Escenario USD (EFFR→SOFR) | Veredicto |
|---|---|---|---|
| **C. Ruptura del rango de apertura** | no pasa los criterios 1, 2, 3 ni 4 | igual (sin financiación: es intradía) | **NO PASA. Se cierra.** |
| **D1. Cambio de mes** | pasa solo el 4 (costos × 1,5) | pasa solo el 4 | **NO PASA. Se cierra.** |

- **C pierde** en la cartera y en los tres índices: −0,50 % por mes en promedio (t = −2,75). Antes de costos tampoco hay ventaja: +0,05 R por operación en US500, −0,01 R en US100 y 0,00 R en US30. Los costos se llevan 0,06–0,12 R por operación.
- **D1 da un resultado positivo pero chico y no significativo:** +0,029 % por mes (EUR) y +0,026 % (USD), con p = 0,29 y 0,31 (hacía falta < 0,0125). El Sharpe deflactado es 0,009 y 0,008 (hacía falta ≥ 0,95). La primera mitad de la validación (2013–2019) es **negativa**, y la segunda mitad positiva se apoya sobre todo en 2020.
- Según la regla de #466 §4.4, las dos familias se **cierran**: no se prueban variantes de C ni de D1 sobre estos datos. Si ninguna pasa, #466 §5 recomienda **parar y revisar el planteo con el dueño antes de gastar la ronda 2**.
- No hay candidato para el fuera de muestra: queda intacto.

## Método (resumen; el detalle está en el pre-registro)

- **Datos:** velas de 15m de los futuros ES, NQ y YM de Databento (`--fuente futuros`), del 01-01-2011 al **01-11-2025, sin cargar nada del fuera de muestra**. Para saber dónde empieza el fuera de muestra se leyó solo la hora de la última vela guardada.
- **Walk-forward** de `config/data_split.toml`: 51 ventanas de validación de 3 meses, 02-2013 → 10-2025 (153 meses). Las familias no tienen parámetros que ajustar: las señales se calculan una vez con funciones puras sobre la serie de investigación, y en cada ventana se ejecutan las **decididas** en ella, con una cuenta nueva.
- **Cuenta:** una por índice, de 100 000 € (ver pre-registro §2.2), con todo el riesgo de `config/risk.toml` sin cambios (1 % por operación, stop entre 0,5 y 3 ATR, pérdida diaria 3 %, kill switch a −15 % que en backtest se reactiva a los 20 días hábiles). **Cartera** = promedio de los retornos mensuales de los tres índices.
- **Costos:** spread típico y slippage de config (`fill_price`), comisión, financiación de IG noche por noche con el escenario y **menos la base del futuro** (#470, EFFR→SOFR), las mismas funciones que el backtest de la estrategia en uso. Estrés: todos los costos × 1,5 sobre las mismas operaciones.
- **Base real en solo lectura:** conexión con `default_transaction_read_only=on`; no se escribió nada.
- **Herramienta:** ejemplo `ronda1` (módulo `cfds_backtest::research`, este PR), aparte de la estrategia en uso: ni el runner ni el paper lo usan, y `config/strategy.toml` no cambia. Parámetros en `config/research.toml`.

## Resultados por familia

### C. Ruptura del rango de apertura

Los dos escenarios dan exactamente lo mismo: las operaciones cierran antes del corte de financiación (23:00 de Madrid).

| Índice | Operaciones | Largos / cortos | Salen en el stop | Rechazadas por el riesgo | R medio antes de costos | Costo medio | R neto medio | Aciertos | Retorno mensual medio | Con costos × 1,5 |
|---|---|---|---|---|---|---|---|---|---|---|
| US500 | 1358 | 770 / 588 | 613 (45 %) | 1675 stop > 3 ATR, 1 drawdown | +0,050 R | 0,124 R | −0,073 R | 40,6 % | −0,64 % | −1,18 % |
| US100 | 811 | 460 / 351 | 346 (43 %) | 2186 stop > 3 ATR | −0,012 R | 0,073 R | −0,085 R | 41,4 % | −0,45 % | −0,64 % |
| US30 | 999 | 537 / 462 | 419 (42 %) | 1974 stop > 3 ATR | −0,001 R | 0,063 R | −0,064 R | 41,1 % | −0,42 % | −0,63 % |

R mediano en puntos: 10,7 (US500), 50,2 (US100) y 91,0 (US30).

| Criterio (cartera, 153 meses) | Resultado | ¿Pasa? |
|---|---|---|
| 1. Prueba t unilateral, nivel 1,25 % | media −0,504 % por mes (desvío 2,26 %), t = −2,75, p = 0,997 | no |
| 2. Sharpe deflactado ≥ 0,95 (N = 350) | Sharpe mensual −0,22 (≈ −0,77 anual), SR0 = 0,239, DSR = 0,000 | no |
| 3. Consistencia | 2013–2019: −0,98 % por mes; 2020–2025: +0,06 %; índices con media > 0: 0 de 3 | no |
| 4. Costos × 1,5 | −0,82 % por mes | no |

Lectura:
- **La ruptura no tiene ventaja antes de costos** en US100 ni en US30, y en US500 la que tiene (+0,05 R) no paga los costos.
- **El riesgo rechazó la mayoría de los días** (55 % en US500, 73 % en US100 y 66 % en US30): el stop en el extremo opuesto del rango queda a más de 3 ATR de 15m. Era lo previsto en #466 §3.C ("puede superar los 3 ATR en días de apertura violenta"), pero pasa en más de la mitad de los días, no solo en los violentos. Lo pre-registrado fue que el riesgo los rechace; probar C sin ese tope (con un máximo propio) sería **otra familia**, con su costo en el presupuesto de error.
- El costo real (0,06–0,12 R) es **mayor** que el estimado en #466 (0,02–0,07 R): las operaciones que pasan el riesgo son las de rangos más chicos, y de 2013 a 2019 el índice estaba más bajo con el mismo spread en puntos.
- Por año (suma de los retornos mensuales de la cartera): 2013 −16,8 %, 2014 −3,4 %, 2015 −20,5 %, 2016 −4,4 %, 2017 −15,3 %, 2018 −1,1 %, 2019 −19,9 %, 2020 −4,6 %, 2021 −1,4 %, 2022 −1,3 %, 2023 +9,4 %, 2024 +7,2 %, 2025 (a octubre) −5,0 %.

### D1. Cambio de mes

| Índice | Escenario | Operaciones | Salen en el stop | R medio antes de costos | Costo medio | R neto medio | Aciertos | Retorno mensual medio | Con costos × 1,5 |
|---|---|---|---|---|---|---|---|---|---|
| US500 | EUR | 197 | 21 | +0,038 R | 0,024 R | +0,015 R | 53,3 % | +0,019 % | +0,004 % |
| US500 | USD | 197 | 21 | +0,038 R | 0,027 R | +0,012 R | 53,3 % | +0,015 % | −0,002 % |
| US100 | EUR | 196 | 25 | +0,031 R | 0,016 R | +0,015 R | 54,1 % | +0,019 % | +0,009 % |
| US100 | USD | 196 | 25 | +0,031 R | 0,018 R | +0,013 R | 54,1 % | +0,017 % | +0,005 % |
| US30 | EUR | 196 | 18 | +0,052 R | 0,014 R | +0,038 R | 54,1 % | +0,048 % | +0,039 % |
| US30 | USD | 196 | 18 | +0,052 R | 0,017 R | +0,035 R | 53,6 % | +0,045 % | +0,034 % |

- Son ~152 eventos por índice; hay más operaciones porque el cierre del viernes parte la ventana en dos (se reabre el lunes, pre-registro §2.4). El riesgo no rechazó ninguna.
- R mediano en puntos: 68,5 (US500), 231,6 (US100) y 596,4 (US30).

| Criterio (cartera, 153 meses) | EUR | USD | ¿Pasa? |
|---|---|---|---|
| 1. Prueba t unilateral, nivel 1,25 % | media +0,029 % por mes (desvío 0,63 %), t = 0,56, p = 0,287 | media +0,026 %, t = 0,50, p = 0,309 | no |
| 2. Sharpe deflactado ≥ 0,95 (N = 350) | Sharpe mensual 0,046 (≈ 0,16 anual), SR0 = 0,239, DSR = 0,009 | Sharpe 0,041 (≈ 0,14), DSR = 0,008 | no |
| 3. Consistencia | 2013–2019: −0,026 % por mes; 2020–2025: +0,094 %; índices > 0: 3 de 3 | −0,029 % / +0,091 %; 3 de 3 | no (primera mitad negativa) |
| 4. Costos × 1,5 | +0,017 % por mes | +0,012 % | sí |

Lectura:
- **Hay un efecto positivo antes de costos** (+0,03 a +0,05 R por operación, 53–54 % de aciertos), del lado que dice la literatura, pero es **chico**: con 1 % de riesgo por operación da unos 0,03 % por mes en la cartera, un Sharpe anual de ~0,15. La regla pedía ~0,63 o más para pasar el criterio 1 (#466 §4.3).
- **No es estable:** de 2013 a 2019 la cartera pierde, y la segunda mitad positiva se apoya sobre todo en 2020 (suma de los retornos mensuales de la cartera por año, EUR: 2013 +0,33 %, 2014 −0,69 %, 2015 −0,65 %, 2016 −1,12 %, 2017 +1,44 %, 2018 −1,82 %, 2019 +0,33 %, **2020 +5,49 %**, 2021 +0,17 %, 2022 +0,05 %, 2023 +0,11 %, 2024 +0,14 %, 2025 +0,61 %).
- Coincide con la advertencia de #466 (McLean y Pontiff, 2016): el efecto publicado con muestras anteriores a 2011 aparece mucho más chico después.
- **Con 10 000 € no se podría operar buena parte:** 31 % de las operaciones en US500, 54 % en US100 y 58 % en US30 quedarían debajo del tamaño mínimo de IG.

## Descriptivo, sin voto: US2000 con costos supuestos

Costos «proporcionales» de #421 (supuestos, no datos de IG). Validación 08-2019 → 10-2025: 25 ventanas, 75 meses. La "primera mitad" son solo 5 meses de 2019.

| Familia | Escenario | Operaciones | R antes de costos | Costo | R neto | Retorno mensual medio | t | p | DSR |
|---|---|---|---|---|---|---|---|---|---|
| C | EUR y USD (iguales) | 303 (1120 rechazadas por stop > 3 ATR) | +0,015 | 0,054 | −0,039 | −0,16 % | −0,73 | 0,77 | 0,000 |
| D1 | EUR | 96 | +0,022 | 0,012 | +0,010 | +0,013 % | 0,16 | 0,44 | 0,003 |
| D1 | USD | 96 | +0,022 | 0,013 | +0,008 | +0,010 % | 0,13 | 0,45 | 0,002 |

Va en la misma dirección que la cartera: C pierde y D1 queda cerca de cero.

## Registro de corridas (pre-registro §2.7)

Todas con el binario de este PR, contra la base en solo lectura, sin cargar velas desde el 01-11-2025:

| # | Hora (24-11-2026, todas después del commit de las 06:50) | Comando (`cargo run --release -p cfds-backtest --example ronda1 -- …`) | Nota |
|---|---|---|---|
| 1 | 06:51 | `US500 US100 US30 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv --salida …` | primera corrida |
| 2 | 06:51 | lo mismo, más `--operaciones …` | se repitió **solo** para exportar las operaciones y revisarlas; números idénticos a la 1 |
| 3 | 06:51 | `US500 US100 US30 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv --salida … --operaciones …` | |
| 4 | 06:51 | `US2000 --descriptivo --supuestos ../docs/analysis/2026-11-us2000-supuestos-proporcional.toml --escenario-financiacion EUR=…` | descriptivo |
| 5 | 06:51 | lo mismo con `USD=…` | descriptivo |

Para el Sharpe deflactado de las próximas rondas, esta ronda suma **2 pruebas** (C y D1). Las corridas 4 y 5 son descriptivas y la 2 es una repetición sin cambios.

Los retornos mensuales de cada índice (con y sin el estrés de costos) quedan en `docs/analysis/2026-11-ronda1-retornos-eur.csv` y `docs/analysis/2026-11-ronda1-retornos-usd.csv`.

## Verificación

- Se revisaron a mano operaciones de las dos familias en el registro exportado:
  - C, 01-02-2013: ruptura al cierre de 10:00–10:15 ET, entrada en la apertura siguiente, stop en el mínimo del rango y salida a las 16:00 ET. El tamaño da 1 % de riesgo y el costo es 1,1 puntos × tamaño.
  - D1, 31-01-2013 (jueves): se cierra el viernes 01-02 (fin de semana) y se reabre el lunes 04-02, con salida en el tercer día hábil. El 03-07-2013 sale a las 13:00 ET, por el cierre anticipado. La financiación de una noche coincide con (tasa + 3 % − EFFR) × precio × tamaño / 360.
- 22 tests nuevos (`engine/crates/backtest/tests/research.rs`):
  - definiciones de las dos familias y sus casos borde: ruptura después de las 15:00, cierre igual al extremo, falta una vela del rango, decisión bloqueada, mes que termina en viernes, fin de semana dentro de la ventana, evento sin vela de salida;
  - **no repintado**: las señales calculadas con una parte de la serie son las mismas con la serie entera;
  - costos, financiación, riesgo, gap y stop, y retornos mensuales;
  - estadística contra valores de tabla (t de Student, Φ, Φ⁻¹) y el Sharpe deflactado calculado a mano.

## Desvíos y límites

1. **Una frase del pre-registro (§3.1) era inexacta.** Decía que el primer mes de la validación (02-2013) no tendría operación de D1. La regla aplicada es la del §2.1, ejecutar lo **decidido** dentro de la ventana, y el código la cumple. El 31-01-2013 fue jueves: su primera pata se decidió en el entrenamiento y no se ejecutó, pero la reapertura del lunes 04-02 se decidió en la validación y sí se ejecutó. No cambia ninguna regla ni el veredicto (es una operación de 197).
2. **Futuros, no el CFD.** La base y el horario no son los del CFD de IG, y el spread es el típico de config, no uno medido (#267). El tipo de referencia de la financiación sigue pendiente; los dos escenarios dan el mismo veredicto.
3. **Diarias de contado para el ATR de D1.** El rango verdadero del día siguiente a un vencimiento incluye el salto de la base (pre-registro §2.4).
4. **C con el tope de stop del riesgo:** se operó solo en los días con el stop a 3 ATR de 15m o menos (ver arriba). Sin ese tope sería otra familia.
5. **Capital de 100 000 €** para que el tamaño mínimo no decida la prueba. Con 10 000 €, D1 no se podría operar en buena parte de los casos.
6. **Sin calendario de noticias** para el período (§7.2), igual que en los estudios anteriores.

## Para el dueño

- C y D1 **no pasan** la regla pre-registrada y quedan cerradas para estos datos. No se activa nada, no se toca la estrategia en uso y el fuera de muestra sigue intacto.
- La recomendación de #466 §5 para este caso es **parar y revisar el planteo antes de la ronda 2**. Hay dos datos para esa conversación:
  - el único efecto con el signo esperado (D1) es del orden de 0,03 R por operación antes de costos;
  - la familia con más muestra (C) no muestra ventaja antes de costos en dos de los tres índices.
