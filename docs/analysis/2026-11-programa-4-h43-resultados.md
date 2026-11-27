# Programa 4, H4.3: estrategia C ("de campeonato") de Gianluca Bruni en el CFD US100 (resultados)

Issue #561 · 25-11-2026 · regla congelada en `2026-11-programa-4-preregistro.md` (congelado el 25-11-2026 a las 03:59 UTC, SHA-256 `973f7fd2ddcaf4a333eca8e8d179f29cee0900a36e10a4c8be5f084cceb47460`, PR #569; verificado antes de correr: coincide con el `.sha256`). Esperaba el calendario del BLS (decisión 11), cargado en #610. Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real.

## Resumen en simple

- **Qué se probó:** la "estrategia C" que Bruni dice que usó en el campeonato. En velas de 15 minutos del Nasdaq: cuando el día viene "corriendo" su zona de más volumen hacia arriba, la estructura es alcista y el precio vuelve a la zona de valor del día anterior y la recupera con una vela fuerte de poco volumen, se compra en la vela siguiente, con el stop en el mínimo de esa vela y el objetivo en 2 veces el riesgo (o el máximo anterior, si está más cerca). Lo mismo al revés para vender. Se saltean los días con dato fuerte (FOMC, inflación y empleo) y los días de cambio de contrato.
- **Veredicto: H4.3 NO PASA**, en los dos escenarios de financiación. Falla los cinco criterios y además el mínimo de 100 operaciones. Se cierra y no se prueban variantes.
- **Lo que se vio:** la regla es muy exigente: en casi 13 años dio **128 señales** y **85 operaciones** (unas 7 por año). Y esas operaciones **pierden**: −0,31 veces el riesgo por operación después de costos (−0,17 antes de costos), 39 % de aciertos.
  - Antes de costos ya pierde: no es solo el costo.
  - Muchas veces el "máximo anterior" queda muy cerca de la entrada (en 48 de 85 operaciones recortó el objetivo), así que la ganancia posible es chica y el costo del CFD pesa mucho.
  - Contra entradas al azar con los mismos stops, la estrategia queda **peor** que el 81 % de las simulaciones.
- **Con esto, el programa 4 queda sin hipótesis vivas** (H4.1, H4.2 y H4.3 no pasan). La reserva (1,25 %) sigue sin usar y el fuera de muestra sigue sin tocar.

## 1. Cómo se corrió

- Ejemplo `programa_4_h43` de `cfds-backtest` (`engine/crates/backtest/examples/programa_4_h43.rs`), con las reglas puras nuevas de `research::program4_c` (perfil de volumen `ProfileTracker`, señales `championship_plans`) sobre la estructura, el ATR, los días de roll y la ventana de ejecución de `research::program4` (las mismas de H4.1 y H4.2), y la ejecución de siempre (`run_plans`: riesgo `cfds_risk::check` con 1 %, spread de IG 1,6 puntos, slippage, comisión 0, financiación menos la base del futuro). Único agregado a la ejecución: un **tope del objetivo** (`Plan::target_cap`, el extremo de la estructura) y, si la apertura de la entrada ya llegó a ese tope, no se opera (`through_cap`). Comprar y mantener con `managed_returns` y una cuenta nueva por ventana, como H4.1 y H4.2.
- Tests con datos sintéticos en `engine/crates/backtest/tests/research_program4_c.rs` (15): config, ancho del casillero, reparto del volumen, área de valor y empates del POC, sesión del CME con horario de verano, perfil anterior incompleto, contratos distintos y vencimiento, perfil en desarrollo sin barras futuras, días de CPI a las 08:30, la señal larga y su espejo corto, cada condición que la bloquea, reentradas y fin de la secuencia, no repintado, tope del objetivo y régimen.
- Base real en **solo lectura**: velas de 15 m del NQ y **4.305.685 barras de 1 minuto** (para el perfil y nada más) hasta el 01-11-2025 (el fuera de muestra **no se cargó**), y `official_releases` del FOMC y del BLS publicado antes de esa fecha.
- 51 ventanas de validación (02-2013 → 10-2025, 153 meses), una cuenta nueva de 100.000 € en cada una.
- Días sin señales: 59 de roll y 456 con dato fuerte (118 fechas del FOMC, 176 del CPI y 177 del empleo en todo el histórico cargado).

### Registro de corridas

| # | Hora (UTC) | Comando (desde `engine/`, código del commit `c471bfb` de la rama, `ef354f1` después del rebase, antes del squash) | Nota |
|---|---|---|---|
| 1 | 25-11-2026 12:06:19 | `programa_4_h43 US100 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv --salida … --operaciones …` | primera y única corrida EUR |
| 2 | 25-11-2026 12:06:58 | `programa_4_h43 US100 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv --salida …` | primera y única corrida USD |

No hubo corridas repetidas ni errores de código encontrados después de ver los resultados.

Archivos: retornos mensuales `2026-11-programa-4-h43-retornos-eur.csv` y `…-usd.csv`; operaciones `2026-11-programa-4-h43-operaciones.csv` (idénticas en los dos escenarios: ninguna cruza la noche).

### Perfiles y señales (impresos antes de los resultados)

- Sesiones del CME cerradas desde 01-2013: 3.287; con al menos el 90 % de sus barras de 1 minuto (si no, el perfil no existe y al día siguiente no hay señales, §6.1): 2.754. Por año: 2013 101 de 239, 2014 131 de 250, 2015 159 de 259, 2016 218 de 258 y desde 2017 entre 232 y 247 de unas 258 (ver "Ambigüedades", punto 3).
- **Señales: 128** (77 primeras de su secuencia y 51 reentradas): 2 en 2013, 7 en 2014, 1 en 2015, 6 en 2016, 7 en 2017, 14 en 2018, 14 en 2019, 4 en 2020, 16 en 2021, 15 en 2022, 16 en 2023, 8 en 2024 y 18 en 2025.
- Cuántas velas (por lado, en toda la serie) quedan después de cada condición:

| Paso | Velas × lado |
|---|---|
| Decisión permitida, ejecución en horario y perfiles de las dos sesiones | 120.794 |
| 2. Estructura actual de ese lado | 60.397 |
| 1. Perfil en desarrollo desplazado (POC y VAL más altos; espejo en cortos) | 35.681 |
| 3. Vuelta al área de valor (o secuencia en curso) | 15.290 |
| 4. Extremo de la estructura sin perforar | 7.461 |
| 5a. Cierre más allá de la vela anterior + 1,5 puntos | 1.489 |
| 5b. Cierre más allá del VAH (VAL) de ayer + 1,5 puntos | 1.107 |
| 5c. Volumen eficiente | 740 |
| 6. Primera de la secuencia rompe el VAH (las reentradas no lo necesitan) | 184 |
| 7. Sin dato fuerte ni roll ese día | 177 |
| 8. Hay recorrido hasta el extremo de la estructura | **128** |

## 2. Criterios (US100)

| Criterio (pre-registro §7) | EUR | USD | ¿Pasa? |
|---|---|---|---|
| 1. Alfa contra comprar y mantener, p < 0,0125 | α = −0,162 % por mes (≈ −1,94 % por año), t −2,56, **p 0,994** | α = −0,162 %, **p 0,995** | **no** |
| 2. Sharpe deflactado de la ventaja ≥ 0,95 (N = 359) | Sharpe mensual −0,219, **DSR 0,000** | −0,220, **DSR 0,000** | **no** |
| 3. Ventaja media > 0 en las dos mitades | **−0,226 %** (2013–2020) y **−0,057 %** (2021–2025) | −0,226 % y −0,057 % | **no** |
| 4. Alfa con costos × 1,5 > 0 | **−0,198 %** | **−0,199 %** | **no** |
| 5. Mezcla 50/50 con Sharpe mayor que comprar y mantener | 0,257 < 0,293 | 0,237 < 0,273 | **no** |
| Mínimo de 100 operaciones | **85** | **85** | **no** |

**H4.3 no pasa**: falla todo, en los dos escenarios (alcanzaba con fallar uno).

Referencias sin voto: DSR con N = 100 y con N = 1000: 0,000. Costos × 2: α = −0,235 % por mes (EUR).

Comprar y mantener el US100 dio +1,456 % por mes con Sharpe mensual 0,293 (EUR) y +1,357 % con 0,273 (USD), igual que en H4.1, H4.2 y el programa Nasdaq.

## 3. Descriptivo (sin voto)

| | H4.3 |
|---|---|
| Señales / operaciones (largas / cortas) | 128 / 85 (40 / 45) |
| Señales sin operar | 8 rechazadas por el riesgo (6 stop a menos de 0,5 ATR, 2 a más de 3 ATR); el resto, por una posición a la vez |
| No operadas porque la entrada abrió del lado del stop / ya en el extremo de la estructura | 0 / 0 |
| Salidas: objetivo / stop / cierre del día | 29 / 39 / 17 |
| Objetivos recortados por el extremo de la estructura | 48 de 85 |
| Aciertos (P/L neto > 0) | 38,8 % |
| R medio antes de costos | −0,172 |
| Costo medio por operación | 0,141 R |
| R medio neto | −0,313 |
| Distancia del stop (mediana) | 19 puntos |
| Recorrido de las que llegaron al objetivo (mediana) | 12,25 puntos |
| Primeras / reentradas operadas | 73 (R neto −0,348) / 12 (R neto −0,098) |
| R neto 2013–2020 / 2021–2025 | −0,544 (41 op.) / −0,097 (44 op.) |
| R neto largos / cortos | −0,322 / −0,304 |
| No se habrían abierto con 10.000 € (mínimo de IG) | 0 |
| Retorno mensual medio (costos × 1,5) | −0,165 % (−0,203 %) |

Operaciones por año: 2 en 2013, 6 en 2014, 1 en 2015, 3 en 2016, 6 en 2017, 9 en 2018, 11 en 2019, 3 en 2020, 9 en 2021, 9 en 2022, 10 en 2023, 7 en 2024 y 9 en 2025.

- **Régimen** (percentil del ATR en 200 velas, §6.4): 52 operaciones en "extremo" (R neto −0,38), 32 en "alto" (−0,25), 1 en "medio" y ninguna en "bajo". La regla casi siempre dispara cuando la volatilidad ya está alta (la vela del disparador es grande).
- **Velas que tocaron el stop y el objetivo a la vez:** 1 (contada como stop); con barras de 1 minuto, el objetivo fue primero. Cambiarla no mueve nada.
- **Autocorrelación:** aciertos 38,8 % en total, 33,3 % después de una ganadora y 43,1 % después de una perdedora. Con 85 operaciones no hay ventanas de 100. Igual que en H4.1 y H4.2, "saltear la operación que sigue a una pérdida" (la candidata de la reserva) habría ido en contra. Es descriptivo y no habilita nada.
- **Test de suerte** (1.000 simulaciones, SplitMix64, semilla 20261125, objetivo 2 R, los mismos días excluidos): R neto medio de la estrategia −0,315 (**percentil 19**; media de las simulaciones −0,200); retorno total con 1 % por operación −23,8 % (percentil 16,1). La estrategia sale **peor** que entrar al azar con los mismos stops. Parte de eso es el tope del objetivo (las simulaciones usan 2 R sin tope).

## 4. Qué sigue

- **H4.3 se cierra.** No se prueban variantes (otro margen, otra media de volumen, sin el tope del objetivo, solo reentradas, otra sesión del perfil): todas serían pruebas nuevas con costo en el presupuesto (§10). Los cortes de la sección 3 **no** se pueden usar como regla nueva.
- Con H4.1, H4.2 y H4.3 cerradas, el programa 4 no tiene hipótesis vivas. La **reserva** (1,25 %) sigue sin asignar; usarla pide un pre-registro nuevo y fechado (§8).
- El fuera de muestra (desde el 01-11-2025) sigue sin tocar.
- Registro de resultados: fila **R033**.

## Ambigüedades y cómo se resolvieron

Ninguna de estas lecturas se eligió mirando resultados: están en el código del commit de la corrida, escrito antes de correr. Donde había que elegir, se tomó la lectura más conservadora o la única que deja la regla bien definida, y se dice.

1. **Días de CPI y empleo (decisión 11).** El texto dice "dato fuerte con hora programada dentro de la sesión de contado", pero el CPI y el empleo salen a las **08:30 ET**, antes de la apertura (09:30). Leído al pie de la letra, nunca excluiría esos días, y la decisión los nombra expresamente y pide esperar al BLS para H4.3. Se tomó la lectura **conservadora**: se excluye el día de la sesión de contado en cuya fecha está programado el dato, antes de su cierre (para el FOMC, 14:00 ET, es exactamente lo mismo que en H4.1 y H4.2). En el embudo, el paso de los datos fuertes (y del roll) saca solo 7 de 184 candidatas, así que esta lectura no cambia el veredicto.
2. **La "vuelta al área de valor" cuenta después de la ruptura.** El máximo de la estructura se define "entre la vela de la última ruptura alcista y la última vela anterior a la primera que tocó el área de valor". Si esa primera vuelta del día vino **antes** de la ruptura, el máximo no existe. Se tomó la única lectura que lo deja definido: la vuelta que arma una secuencia es la primera vela de la sesión **posterior** a la última ruptura de ese lado. Leído literalmente (la primera vuelta del día, aunque sea anterior), habría todavía menos señales.
3. **"Le falta más del 10 % de sus barras de 1 minuto" (§6.1).** Databento solo guarda una barra en los minutos con operaciones, así que no se puede distinguir un minuto sin operaciones de un dato perdido. Se aplicó **literal**: una sesión de 23 horas (1.380 minutos) necesita al menos 1.242 barras. En 2013 y 2014, con el NQ menos líquido de noche, cerca de la mitad de las sesiones no llegan (en 2015, 4 de cada 10), y esos días siguientes no tienen señales (ver la tabla de perfiles). Es lo conservador (menos operaciones, ningún número inventado) y es una de las razones de que la primera mitad tenga pocas operaciones.
4. **"Día hábil" de la sesión del CME.** Se usó lunes a viernes: la sesión `d` va de las 18:00 ET de la víspera a las 17:00 ET de `d` (la del lunes empieza el domingo a las 18:00). Un feriado de lunes a viernes tiene su propia sesión, corta, que no llega al 90 %: el día siguiente no tiene señales (conservador).
5. **Primera operación y reentradas.** Se cuentan sobre las **señales** (antes de la regla de una posición a la vez y del módulo de riesgo), como define las señales §13.2. Una señal que el riesgo rechaza igual cuenta como la primera de su secuencia. Así las señales son una función pura de las velas.
6. **Condición 8 (hay recorrido).** Se exige al decidir (el extremo de la estructura más allá del cierre de `t`) y otra vez al ejecutar (más allá de la apertura de `t + 1`); si no, no se opera. Pasó 0 veces en la ejecución.
7. **Detalles menores:** el mínimo de la estructura sale de la vela contraria válida **anterior** a la vela de la ruptura; la media de 20 del volumen pide las 21 velas consecutivas y del mismo contrato; las secuencias se reinician con cada sesión del CME; los perfiles se reinician en cada vencimiento y cuando la sesión anterior es de otro contrato.

## Límites

- Velas y volumen del futuro NQ, no del CFD. El volumen es el del mercado real, que es lo que mira Bruni.
- Spread de IG de 1,6 puntos constante (todas las ejecuciones caen en la sesión de contado).
- El filtro de noticias de alto impacto de la spec (§7.2) no tiene histórico y no se aplicó, como en las pruebas anteriores. Los días de FOMC, CPI y empleo sí se excluyeron.
- Las reglas son una lectura medible de lo que dice Bruni en una entrevista (perfil, área de valor, estructura, volumen "eficiente"); otra definición podría dar otro resultado, pero cambiarla ahora sería una prueba nueva.
- Muestra chica: 85 operaciones.

---

**Nota del 25-11-2026 (#690), agregada después del informe:** la regla de presupuesto global de #660 fue aprobada por el dueño el 25-11-2026 en #660 (cita: «1, 2 y 7 apruebo la recomendacion.»), adoptada en #690. **La reserva de 1,25 % del programa 4 deja de existir.** La candidata de §8 del pre-registro ("saltear tras una pérdida") solo se puede probar con un pre-registro nuevo que saque su nivel del libro único ("Presupuesto global" en `registro-de-resultados.md`) y pase el filtro de borde bruto. El pre-registro congelado (`2026-11-programa-4-preregistro.md` y su `.sha256`) no se tocó.
