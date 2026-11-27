# Programa 4, H4.1 y H4.2: estrategia A de Gianluca Bruni en el CFD US100 (resultados)

Issue #561 · 25-11-2026 · regla congelada en `2026-11-programa-4-preregistro.md` (congelado el 25-11-2026 a las 03:59 UTC, SHA-256 `973f7fd2ddcaf4a333eca8e8d179f29cee0900a36e10a4c8be5f084cceb47460`, PR #569), **antes** de correr ningún resultado. Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real.

## Resumen en simple

- **Qué se probó:** la "estrategia A" que Bruni muestra en el video. En velas de 15 minutos del Nasdaq, cuando la estructura del mercado (las dos últimas rupturas de zona) va para un lado y aparecen **dos velas seguidas fuertes** a favor, se entra en la vela siguiente, con el stop en el otro extremo de la primera vela, objetivo de 1,5 veces el riesgo y cierre antes del final del día. Dos versiones:
  - **H4.1, literal:** cada vela con un cuerpo de al menos **15 puntos**, como dice él.
  - **H4.2, normalizada:** cada vela con un cuerpo de al menos **0,35 × el ATR** (el número que deja pasar, en el último año, la misma proporción de velas que los 15 puntos).
- **Veredicto: H4.1 NO PASA y H4.2 NO PASA**, en los dos escenarios de financiación. Las dos se cierran y no se prueban variantes.
- **H4.1 (literal):** 539 operaciones entre 2016 y 2025. Ganó un poco: +0,15 % por mes, unos +0,045 R por operación después de costos. Pero eso **no se distingue del azar** con la vara fijada (p = 0,17 contra 0,0125) y el Sharpe deflactado, que descuenta las 359 pruebas del proyecto, da **0,03** (hace falta 0,95). Además, casi todo lo ganado viene de 2016–2020 (111 operaciones, +0,19 R cada una); desde 2021 (428 operaciones) queda en **+0,007 R**, o sea en cero.
- **H4.2 (normalizada):** 1.359 operaciones, todos los años. **Pierde**: −0,39 % por mes, −0,044 R por operación. Antes de costos ganaba algo (+0,069 R), pero sus stops son más cortos (30 puntos de mediana) y el costo del CFD se come 0,11 R por operación.
- **Lo que se aprende:** con las reglas medibles de Bruni hay un pequeño efecto a favor antes de costos (las operaciones salen mejor que entradas al azar con los mismos stops: percentil 93 a 97 del test de suerte), pero es **chico, inestable en el tiempo y, en la versión con stops chicos, más chico que el costo**. Nada de esto alcanza para pasar la vara del proyecto.
- **H4.3** (estrategia C) sigue congelada, esperando el calendario del BLS (decisión 11). La reserva (1,25 %) sigue sin usar.

## 1. Cómo se corrió

- Ejemplo `programa_4_h41_h42` de `cfds-backtest` (`engine/crates/backtest/examples/programa_4_h41_h42.rs`), con las reglas puras de `research::program4` (PR #569) y la ejecución de siempre (`run_plans`: riesgo `cfds_risk::check` con 1 %, spread de IG 1,6 puntos, slippage, comisión 0, financiación menos la base del futuro). Comprar y mantener con `managed_returns` y una cuenta nueva por ventana, como el programa Nasdaq.
- Base real en **solo lectura**: velas de 15 m del NQ (`futures_candles`, armadas desde `futures_bars_1m`) hasta el 01-11-2025 (el fuera de muestra **no se cargó**), y `official_releases` del FOMC publicado antes de esa fecha.
- 51 ventanas de validación (02-2013 → 10-2025, 153 meses), una cuenta nueva de 100.000 € en cada una.
- Días sin señales: 59 días de roll y 118 días con comunicado programado del FOMC (decisiones 11 y 15, §13.1 del pre-registro). Los días de CPI y empleo **no** se excluyeron porque el BLS no está cargado (limitación declarada antes de correr).

### Registro de corridas

| # | Hora (UTC) | Comando (desde `engine/`, código del commit `b078b22` de la rama, antes del squash) | Nota |
|---|---|---|---|
| 1 | 25-11-2026 04:03:45 | `programa_4_h41_h42 US100 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv --salida … --operaciones …` | primera y única corrida EUR |
| 2 | 25-11-2026 04:04:10 | `programa_4_h41_h42 US100 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv --salida … --operaciones …` | primera y única corrida USD |

Antes, sin resultados: `programa_4_k` (03:58, k = 0,35) y `programa_4_conteo` (03:58, señales por año), en el PR del congelado. No hubo corridas repetidas ni errores de código encontrados después de ver los resultados.

Archivos: retornos mensuales `2026-11-programa-4-h41-h42-retornos-eur.csv` y `…-usd.csv`; operaciones `2026-11-programa-4-h41-h42-operaciones.csv` (las operaciones son idénticas en los dos escenarios: ninguna cruza la noche, así que la financiación es 0 en todas).

### Señales por año (impresas antes de los resultados)

H4.1: 1.008 señales (0 en 2013–2015, 2 en 2016, 4 en 2017, 63 en 2018, 23 en 2019, 126 en 2020, 115 en 2021, 220 en 2022, 139 en 2023, 177 en 2024, 139 en 2025). H4.2: 2.534 (entre 149 y 225 por año). Iguales al conteo del congelado. Una señal no siempre es una operación: hay una posición a la vez y el riesgo rechaza stops demasiado lejos.

## 2. Criterios (US100)

| Criterio (pre-registro §7) | H4.1 EUR | H4.1 USD | H4.2 EUR | H4.2 USD |
|---|---|---|---|---|
| 1. Alfa contra comprar y mantener, p < 0,0125 | α = +0,123 % por mes, t 0,95, **p 0,171** → no | α = +0,125 %, **p 0,165** → no | α = −0,397 %, t −1,69, **p 0,953** → no | α = −0,396 %, **p 0,954** → no |
| 2. Sharpe deflactado de la ventaja ≥ 0,95 (N = 359) | Sharpe mensual 0,089, **DSR 0,032** → no | 0,090, **DSR 0,033** → no | −0,144, **DSR 0,000** → no | −0,143, **DSR 0,000** → no |
| 3. Ventaja media > 0 en las dos mitades | +0,185 % (2013–2020) y +0,022 % (2021–2025) → sí | +0,186 % y +0,024 % → sí | −0,754 % y +0,187 % → no | −0,752 % y +0,189 % → no |
| 4. Alfa con costos × 1,5 > 0 | +0,067 % → sí | +0,070 % → sí | −0,881 % → no | −0,878 % → no |
| 5. Mezcla 50/50 con Sharpe mayor que comprar y mantener | 0,306 > 0,293 → sí | 0,287 > 0,273 → sí | 0,186 < 0,293 → no | 0,169 < 0,273 → no |
| Mínimo de 100 operaciones | 539 → sí | 539 → sí | 1.359 → sí | 1.359 → sí |

**H4.1 no pasa** (falla los criterios 1 y 2 en los dos escenarios). **H4.2 no pasa** (falla los cinco criterios). Alcanzaba con fallar uno.

Referencias sin voto: DSR con N = 100: 0,076 (H4.1, EUR) y 0,000 (H4.2); con N = 1000: 0,015 y 0,000. Costos × 2: α = +0,012 % por mes (H4.1, EUR) y −1,362 % (H4.2).

Comprar y mantener el US100 dio +1,456 % por mes con Sharpe mensual 0,293 (EUR) y +1,357 % con 0,272 (USD), igual que en el programa Nasdaq y en R018.

## 3. Descriptivo (sin voto)

Las operaciones y lo que sigue son iguales en los dos escenarios.

| | H4.1 (15 puntos) | H4.2 (0,35 × ATR) |
|---|---|---|
| Operaciones (largas / cortas) | 539 (258 / 281) | 1.359 (802 / 557) |
| Señales rechazadas por el riesgo | 228 (stop a más de 3 ATR) | 471 (stop a más de 3 ATR) |
| No operadas porque la entrada abrió del lado del stop | 0 | 0 |
| Salidas: objetivo / stop / cierre del día | 98 / 176 / 265 | 289 / 469 / 601 |
| Aciertos (P/L neto > 0) | 50,8 % | 46,6 % |
| R medio antes de costos | +0,077 | +0,069 |
| Costo medio por operación | 0,032 R | 0,113 R |
| R medio neto | +0,045 | −0,044 |
| Distancia del stop (mediana) | 69,5 puntos | 30,0 puntos |
| R neto 2013–2020 / 2021–2025 | +0,192 (111 op.) / +0,007 (428 op.) | −0,085 (843 op.) / +0,023 (516 op.) |
| R neto largos / cortos | +0,127 / −0,030 | −0,007 / −0,098 |
| No se habrían abierto con 10.000 € (mínimo de IG) | 5 (0,9 %) | 5 (0,4 %) |
| Retorno mensual medio (costos × 1,5) | +0,153 % (+0,098 %) | −0,393 % (−0,886 %) |

Operaciones por año: H4.1 tiene 2 en 2016, 0 en 2017, 33 en 2018, 8 en 2019, 68 en 2020 y entre 62 y 119 por año desde 2021. H4.2 tiene entre 84 y 120 por año, todos los años.

- **Velas que tocaron el stop y el objetivo a la vez:** ninguna en las dos hipótesis (el objetivo queda a 2,5 R del stop, y con stops de 0,5 a 3 ATR una vela de 15 m casi nunca cubre esa distancia). La resolución con barras de 1 minuto no cambia nada.
- **Autocorrelación de resultados:** después de una perdedora la tasa de aciertos **no baja**, sube un poco (H4.1: 53,6 % tras una perdedora y 48,0 % tras una ganadora, contra 50,8 % en total; H4.2: 48,2 % y 44,8 % contra 46,6 %). En ventanas de 100 operaciones, la tasa tras una perdedora fue menor que la general en solo el 7,7 % (H4.1) y el 33,2 % (H4.2) de las ventanas. O sea: en esta estrategia, "saltear la operación que sigue a una pérdida" (la candidata de la reserva, pensada para H4.3) habría ido en contra. Es descriptivo y no habilita nada.
- **Test de suerte** (1.000 simulaciones, SplitMix64, semilla 20261125; §13.2): entradas al azar en las mismas sesiones, con los mismos stops en puntos, 1,5 R y los mismos costos.

| | Estrategia: R neto medio (percentil) | Simulaciones: R neto medio | Estrategia: retorno total con 1 % por operación (percentil) |
|---|---|---|---|
| H4.1 | +0,045 (93,0) | −0,016 | +24,3 % (94,1) |
| H4.2 | −0,045 (96,6) | −0,097 | −49,1 % (95,8) |

  Cómo leerlo: las dos hacen algo mejor que entrar al azar con los mismos stops (están en el 5 % de arriba de las simulaciones), pero el azar con estos costos pierde, así que "mejor que el azar" no quiere decir "gana". Y la vara que vota no es esta, sino el alfa contra comprar y mantener con su Sharpe deflactado.

## 4. Qué sigue

- **H4.1 y H4.2 se cierran.** No se prueban variantes (otro umbral, otro objetivo, otro horario, solo largos, solo hasta 2020): todas serían pruebas nuevas con costo en el presupuesto (§10). Los cortes de esta sección (largos contra cortos, por mitades) **no** se pueden usar como regla nueva.
- **H4.3** sigue congelada con su 1,25 %, a la espera de la carga del calendario del BLS (CPI y empleo).
- La **reserva** (1,25 %) sigue sin asignar.
- El fuera de muestra (desde el 01-11-2025) sigue sin tocar.
- Registro de resultados: filas **R029** (H4.1) y **R030** (H4.2).

## Límites

- Velas del futuro NQ, no del CFD: los movimientos en puntos son casi iguales y la financiación se corrige con la base (#470). El volumen no se usa en H4.1 ni H4.2.
- Spread de IG de 1,6 puntos constante (todas las ejecuciones caen en la sesión de contado); no hay spread medido del CFD para esas fechas.
- Solo se excluyen los días del FOMC como datos fuertes; los de CPI y empleo no estaban cargados.
- El filtro de noticias de alto impacto de la spec (§7.2) no tiene histórico y no se aplicó, como en los programas anteriores.
- Las reglas son una lectura medible de lo que dice Bruni (zonas, rupturas, estructura); otra definición podría dar otro resultado, pero cambiarla ahora sería una prueba nueva.
