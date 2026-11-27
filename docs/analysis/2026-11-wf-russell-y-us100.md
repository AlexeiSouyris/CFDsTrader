# Estudio: Russell 2000 (costos supuestos) y US100 con su historia completa, en walk-forward

Issue #421 · 24-11-2026 · sigue a #371 (opción A del dueño: el Russell entra al estudio con **costos supuestos, marcados como tales**) y a #405 (PR #411, donde US100 tenía solo 8 ventanas porque NQ todavía se estaba cargando).

> **Advertencia.** Es un **estudio, no un resultado oficial**. IG España no lista el Russell 2000, así que sus costos son **supuestos**. Para US100 y US2000, el tipo de referencia de la financiación sigue pendiente en `config/instruments.toml`. Nada de esto cambia `config/`: los supuestos y los escenarios se aplican **solo en memoria**, dentro de cada corrida. Las ventanas son pasado y no prometen ningún resultado futuro.

## Conclusión

- **US2000 pierde con todos los supuestos.** En el walk-forward de config (25 ventanas de prueba, de 08-2019 a 10-2025):
  - con costos «como US500»: −10,5 % de retorno medio por ventana, 4 ventanas positivas de 25 y Sharpe medio −2,4;
  - con costos «proporcionales» (el escenario optimista): −7,0 %, 5 positivas de 25 y Sharpe −1,5.
  Solo 4 o 5 ventanas de 25 superan a comprar y mantener. El costo supuesto cambia el nivel de la pérdida, no el signo: **no hay ventaja que defender** con la estrategia actual en el Russell.
- **US100, con la historia completa (51 ventanas, 02-2013 → 10-2025), también pierde:** −6,1 % de retorno medio por ventana, 10 positivas de 51, Sharpe medio −1,3 y supera a comprar y mantener en 11 de 51.
  - Esto **corrige la lectura de #411**, donde US100 parecía estar «en torno a cero» con 8 ventanas. Esas 8 ventanas (11-2023 → 10-2025) dan lo mismo acá (diferencias de ±0,01 puntos, solo por la serie de tasas), pero eran un tramo favorable: los años de prueba positivos son 2021, 2022 y 2024; el resto es negativo.
  - Parte de la pérdida de los primeros años la explica el spread fijo en puntos (ver "Límites", punto 3). Aun desde 2019, la mayoría de los años es negativa.
- **La tasa de financiación no cambia el veredicto, otra vez.**
  - Entre el escenario en EUR (EONIA→€STR) y el escenario en USD (EFFR→SOFR), las ventanas positivas y las que superan a comprar y mantener son **idénticas** en US100 y en US2000.
  - La diferencia mediana por ventana es de −0,01 a −0,04 puntos, y la mayor es de −0,55.
  - Comprar y mantener baja con la tasa en USD, como en #411: hasta 0,69 puntos por ventana en US100 y entre 0,02 y 0,13 en US2000.
- **Para el dueño:** ni US2000 ni US100 justifican hoy ir a la fase 5 con la estrategia actual. Los costos reales del Russell (si IG lo ofrece) y el tipo de referencia confirmado no van a dar vuelta este resultado: lo mueven unos puntos, pero no cambian el signo.

## Método

- **Herramienta:** ejemplo `walk_forward`, con dos flags de estudio en memoria (este PR amplía el de #411):
  - `--supuestos <archivo.toml>` (nuevo): completa los campos `pending` del catálogo con valores de un TOML de estudio, con su propia fuente rotulada (`cfds_backtest::apply_assumptions`). **Nunca pisa un dato con fuente**: si un campo ya tiene dato, la corrida se niega. También puede completar la regla de vencimiento del continuo si `[futures.roll]` no la tiene.
  - `--escenario-financiacion <ETIQUETA>=<archivo.csv>` (ahora acepta cualquier etiqueta): toma la serie diaria de un CSV de fuente oficial. Cada valor cuenta desde las 13:00 UTC de dos días hábiles después de su fecha: el BCE y la NY Fed publican el día hábil siguiente a la mañana, así que nunca se usa un valor antes de estar publicado.
  - En modo estudio, la herramienta **no guarda nada** (se niega a correr con `report_id`) y **no evalúa el fuera de muestra**.
- **Walk-forward:** el de `config/data_split.toml` (24 meses de entrenamiento, 3 de prueba, paso 3, 12 meses fuera de muestra) y `config/evaluation.toml` (30 días de calentamiento), con todos los parámetros de la estrategia de config. Se salteó la sensibilidad (`--sin-sensibilidad`).
  - Cada ventana corre un **backtest** completo de la estrategia sobre su entrenamiento (24 meses) y otro sobre su prueba (3 meses), cada uno desde 10 000 € y con comprar y mantener.
  - La tabla resumen muestra el retorno medio de los backtests de entrenamiento, y el informe se centra en las pruebas.
  - **El fuera de muestra (desde el 01-11-2025) no se tocó.**
- **Datos:** velas de 15m y 1h armadas desde los futuros de Databento (`--fuente futuros`), NQ desde 2011 y RTY desde el 09-07-2017, con la carga terminada (night.log: «FIN» a las 04:15 del 24-11-2026).
  - Se cierra antes de cada vencimiento.
  - El spread es el de config o el supuesto, porque el filtro de spread §7.3 no se aplica a futuros.
  - No hay calendario de noticias guardado para el período, así que el filtro §7.2 no descuenta nada. Es igual en todos los escenarios.
- **Base real en solo lectura:** cada corrida usó una conexión con `default_transaction_read_only=on`. No se escribió nada en `backtest_runs` ni en `walk_forward_reports`.

### Costos supuestos del US2000

Están en dos archivos de estudio (no en `config/`), cada uno con su fuente rotulada como «supuesto-estudio-421 … (no es dato de IG)»:

| Campo | «Como US500» (`2026-11-us2000-supuestos-us500.toml`) | «Proporcional» (`2026-11-us2000-supuestos-proporcional.toml`) | Por qué |
|---|---|---|---|
| Spread típico / mínimo | 0,9 / 0,8 puntos | 0,43 / 0,38 puntos | Los puntos del US500 de IG. El «proporcional» los escala por el cociente Russell / S&P 500 medido en los futuros antes del fuera de muestra (0,478 de media, 2017–2025) |
| Margen | 10 % | 10 % | ESMA no trata al Russell como índice principal: mínimo regulatorio 10 % |
| Valor por punto | 1 € | 1 € | Como los demás índices de EE. UU. (#67) |
| Tamaño mínimo | 1 | 1 | Como el US500 |
| Tick | 0,1 | 0,1 | Tick del futuro RTY (el slippage es de 1 tick) |
| Tasa administrativa | 3 % | 3 % | La publicada para los CFD de índices (#379) |
| Vencimiento | `after_expiry_day` | `after_expiry_day` | RTY es de CME. Ya está en `config/candle_sources.toml` por #449 (las corridas lo tomaron del supuesto, con el mismo valor) |

En términos relativos, «como US500» cuesta **unas dos veces más** que el US500 (es el escenario conservador). «Proporcional» cuesta lo mismo que el US500 (es el optimista).

### Escenarios de financiación (fuentes oficiales, series empalmadas)

| Escenario | Serie | Fuente | Archivo |
|---|---|---|---|
| EUR | EONIA hasta el 30-09-2019 y €STR desde el 01-10-2019 | BCE, Data Portal: `EON.D.EONIA_TO.RATE` y `EST.B.EU000A2X2A25.WT` | `docs/analysis/2026-11-eur-eonia-estr.csv` |
| USD | EFFR hasta el 29-03-2018 y SOFR desde el 02-04-2018 (su primera publicación) | Federal Reserve Bank of New York, API de tasas de referencia | `docs/analysis/2026-11-usd-effr-sofr.csv` |

- Las dos series se descargaron el 24-11-2026, con las URLs en el encabezado de cada CSV y sin modificar los valores. El tramo €STR del CSV coincide con lo guardado en `reference_rates` (verificado en el primer día, en uno intermedio y en el último).
- EONIA es la serie oficial que pedía #405 para antes de 2019. Con ella, US100 y US2000 corren desde el principio de sus datos.
- Medias en las ventanas de prueba de US100 (02-2013 → 10-2025): **EUR 0,48 % y USD 1,68 %**. De 2013 a 2015, las dos estuvieron cerca de cero.

## Resumen por índice y escenario (ventanas de prueba)

| Índice | Escenario | Ventanas | Operaciones | Retorno medio | Mediana | Positivas | Peor drawdown | Sharpe medio | Supera a CyM | CyM medio | Entrenamiento: retorno medio |
|---|---|---|---|---|---|---|---|---|---|---|---|
| US100 | EUR (EONIA→€STR) | 51 | 2690 | −6,12 % | −5,99 % | 10 | 28,20 % | −1,32 | 11 de 51 | +1,82 % | −34,43 % |
| US100 | USD (EFFR→SOFR) | 51 | 2690 | −6,18 % | −5,99 % | 10 | 28,20 % | −1,34 | 11 de 51 | +1,65 % | −35,35 % |
| US2000 | costos «como US500» · EUR | 25 | 1114 | −10,52 % | −12,55 % | 4 | 26,80 % | −2,41 | 4 de 25 | +0,17 % | −58,38 % |
| US2000 | costos «como US500» · USD | 25 | 1118 | −10,56 % | −12,60 % | 4 | 26,88 % | −2,41 | 4 de 25 | +0,10 % | −57,94 % |
| US2000 | costos «proporcionales» · EUR | 25 | 1239 | −6,95 % | −8,50 % | 5 | 20,10 % | −1,53 | 5 de 25 | +0,17 % | −46,25 % |
| US2000 | costos «proporcionales» · USD | 25 | 1239 | −6,97 % | −8,51 % | 5 | 20,11 % | −1,53 | 5 de 25 | +0,10 % | −46,37 % |

Cada ventana arranca desde 10 000 €. «CyM» es comprar y mantener del mismo tramo con el tamaño mínimo (0,5 contratos en US100 y 1 en US2000), con costos y financiación. «Entrenamiento» es el retorno medio de los backtests de 24 meses de cada ventana.

### US100 por año de la ventana de prueba

| Año | Ventanas | Retorno medio EUR | Retorno medio USD | Positivas EUR | CyM medio EUR | CyM medio USD |
|---|---|---|---|---|---|---|
| 2013 | 4 | −19,06 % | −19,06 % | 0 | +0,84 % | +0,84 % |
| 2014 | 4 | −9,56 % | −9,55 % | 0 | +0,62 % | +0,62 % |
| 2015 | 4 | −10,34 % | −10,34 % | 0 | −0,02 % | −0,04 % |
| 2016 | 4 | −9,77 % | −9,79 % | 1 | +0,91 % | +0,87 % |
| 2017 | 4 | −9,64 % | −9,78 % | 0 | +2,10 % | +1,99 % |
| 2018 | 4 | −9,00 % | −9,06 % | 0 | −0,34 % | −0,54 % |
| 2019 | 4 | −7,22 % | −7,42 % | 1 | +2,39 % | +2,14 % |
| 2020 | 4 | −3,81 % | −3,85 % | 0 | +4,49 % | +4,39 % |
| 2021 | 4 | +3,38 % | +3,35 % | 3 | +2,11 % | +2,00 % |
| 2022 | 4 | +3,27 % | +3,25 % | 1 | −3,98 % | −4,26 % |
| 2023 | 4 | −5,86 % | −5,94 % | 1 | +5,33 % | +5,03 % |
| 2024 | 4 | +4,13 % | +4,05 % | 2 | +3,80 % | +3,44 % |
| 2025 | 3 | −6,12 % | −6,19 % | 1 | +6,58 % | +6,00 % |

### US100, ventana por ventana

| Prueba | Op. EUR | Retorno EUR | CyM EUR | Op. USD | Retorno USD | CyM USD | Δ retorno | Δ CyM |
|---|---|---|---|---|---|---|---|---|
| 2013-02-01 → 2013-05-01 | 33 | −16,11 % | +0,62 % | 33 | −16,11 % | +0,62 % | +0,00 | +0,00 |
| 2013-05-01 → 2013-08-01 | 43 | −27,51 % | +0,91 % | 43 | −27,51 % | +0,91 % | +0,00 | +0,00 |
| 2013-08-01 → 2013-11-01 | 46 | −17,25 % | +1,28 % | 46 | −17,25 % | +1,28 % | +0,00 | +0,00 |
| 2013-11-01 → 2014-02-01 | 41 | −15,38 % | +0,55 % | 41 | −15,38 % | +0,55 % | +0,00 | +0,00 |
| 2014-02-01 → 2014-05-01 | 40 | −10,82 % | +0,17 % | 40 | −10,81 % | +0,18 % | +0,01 | +0,01 |
| 2014-05-01 → 2014-08-01 | 37 | −11,52 % | +1,43 % | 37 | −11,50 % | +1,43 % | +0,02 | +0,00 |
| 2014-08-01 → 2014-11-01 | 43 | −9,91 % | +1,07 % | 43 | −9,92 % | +1,06 % | −0,01 | −0,01 |
| 2014-11-01 → 2015-02-01 | 64 | −5,99 % | −0,19 % | 64 | −5,99 % | −0,20 % | +0,00 | −0,01 |
| 2015-02-01 → 2015-05-01 | 47 | −14,84 % | +1,17 % | 47 | −14,85 % | +1,16 % | −0,01 | −0,01 |
| 2015-05-01 → 2015-08-01 | 60 | −5,91 % | +0,69 % | 60 | −5,91 % | +0,67 % | +0,00 | −0,02 |
| 2015-08-01 → 2015-11-01 | 59 | −15,67 % | +0,13 % | 59 | −15,68 % | +0,12 % | −0,01 | −0,01 |
| 2015-11-01 → 2016-02-01 | 60 | −4,93 % | −2,08 % | 60 | −4,93 % | −2,11 % | +0,00 | −0,03 |
| 2016-02-01 → 2016-05-01 | 38 | −5,18 % | +0,21 % | 38 | −5,21 % | +0,17 % | −0,03 | −0,04 |
| 2016-05-01 → 2016-08-01 | 52 | −15,52 % | +1,80 % | 52 | −15,54 % | +1,77 % | −0,02 | −0,03 |
| 2016-08-01 → 2016-11-01 | 40 | −24,78 % | +0,18 % | 40 | −24,79 % | +0,14 % | −0,01 | −0,04 |
| 2016-11-01 → 2017-02-01 | 54 | +6,42 % | +1,45 % | 54 | +6,38 % | +1,40 % | −0,04 | −0,05 |
| 2017-02-01 → 2017-05-01 | 42 | −5,53 % | +2,08 % | 42 | −5,59 % | +2,00 % | −0,06 | −0,08 |
| 2017-05-01 → 2017-08-01 | 55 | −16,73 % | +1,31 % | 55 | −16,89 % | +1,21 % | −0,16 | −0,10 |
| 2017-08-01 → 2017-11-01 | 47 | −15,04 % | +1,64 % | 47 | −15,17 % | +1,53 % | −0,13 | −0,11 |
| 2017-11-01 → 2018-02-01 | 59 | −1,28 % | +3,36 % | 59 | −1,48 % | +3,23 % | −0,20 | −0,13 |
| 2018-02-01 → 2018-05-01 | 49 | −4,40 % | −1,99 % | 49 | −4,45 % | −2,15 % | −0,05 | −0,16 |
| 2018-05-01 → 2018-08-01 | 40 | −14,62 % | +3,03 % | 40 | −14,68 % | +2,85 % | −0,06 | −0,18 |
| 2018-08-01 → 2018-11-01 | 55 | −15,77 % | −1,74 % | 55 | −15,92 % | −1,97 % | −0,15 | −0,23 |
| 2018-11-01 → 2019-02-01 | 59 | −1,22 % | −0,66 % | 59 | −1,18 % | −0,90 % | +0,04 | −0,24 |
| 2019-02-01 → 2019-05-01 | 51 | −18,31 % | +4,53 % | 51 | −18,60 % | +4,29 % | −0,29 | −0,24 |
| 2019-05-01 → 2019-08-01 | 61 | +7,34 % | −0,31 % | 61 | +7,29 % | −0,59 % | −0,05 | −0,28 |
| 2019-08-01 → 2019-11-01 | 39 | −15,66 % | +0,98 % | 39 | −15,76 % | +0,73 % | −0,10 | −0,25 |
| 2019-11-01 → 2020-02-01 | 60 | −2,24 % | +4,35 % | 60 | −2,59 % | +4,13 % | −0,35 | −0,22 |
| 2020-02-01 → 2020-05-01 | 46 | −0,17 % | −1,00 % | 46 | −0,26 % | −1,14 % | −0,09 | −0,14 |
| 2020-05-01 → 2020-08-01 | 64 | −4,00 % | +10,08 % | 64 | −4,02 % | +10,01 % | −0,02 | −0,07 |
| 2020-08-01 → 2020-11-01 | 41 | −10,83 % | +0,44 % | 41 | −10,84 % | +0,35 % | −0,01 | −0,09 |
| 2020-11-01 → 2021-02-01 | 60 | −0,23 % | +8,44 % | 60 | −0,29 % | +8,35 % | −0,06 | −0,09 |
| 2021-02-01 → 2021-05-01 | 54 | +5,00 % | +4,96 % | 54 | +4,96 % | +4,86 % | −0,04 | −0,10 |
| 2021-05-01 → 2021-08-01 | 55 | +7,93 % | +5,00 % | 55 | +7,89 % | +4,90 % | −0,04 | −0,10 |
| 2021-08-01 → 2021-11-01 | 63 | −6,94 % | +4,06 % | 63 | −6,99 % | +3,94 % | −0,05 | −0,12 |
| 2021-11-01 → 2022-02-01 | 60 | +7,53 % | −5,57 % | 60 | +7,53 % | −5,69 % | +0,00 | −0,12 |
| 2022-02-01 → 2022-05-01 | 59 | −0,09 % | −10,34 % | 59 | −0,10 % | −10,48 % | −0,01 | −0,14 |
| 2022-05-01 → 2022-08-01 | 49 | −6,43 % | −0,26 % | 49 | −6,41 % | −0,53 % | +0,02 | −0,27 |
| 2022-08-01 → 2022-11-01 | 57 | +20,74 % | −7,80 % | 57 | +20,64 % | −8,18 % | −0,10 | −0,38 |
| 2022-11-01 → 2023-02-01 | 47 | −1,12 % | +2,48 % | 47 | −1,13 % | +2,13 % | −0,01 | −0,35 |
| 2023-02-01 → 2023-05-01 | 65 | −2,98 % | +5,17 % | 65 | −2,96 % | +4,86 % | +0,02 | −0,31 |
| 2023-05-01 → 2023-08-01 | 74 | −11,80 % | +11,69 % | 74 | −11,97 % | +11,38 % | −0,17 | −0,31 |
| 2023-08-01 → 2023-11-01 | 51 | −11,04 % | −8,41 % | 51 | −11,12 % | −8,72 % | −0,08 | −0,31 |
| 2023-11-01 → 2024-02-01 | 69 | +2,38 % | +12,86 % | 69 | +2,28 % | +12,60 % | −0,10 | −0,26 |
| 2024-02-01 → 2024-05-01 | 47 | −6,03 % | −0,38 % | 47 | −6,09 % | −0,68 % | −0,06 | −0,30 |
| 2024-05-01 → 2024-08-01 | 65 | +19,95 % | +9,30 % | 65 | +19,77 % | +8,95 % | −0,18 | −0,35 |
| 2024-08-01 → 2024-11-01 | 69 | +3,68 % | +0,34 % | 69 | +3,59 % | −0,05 % | −0,09 | −0,39 |
| 2024-11-01 → 2025-02-01 | 59 | −1,06 % | +5,92 % | 59 | −1,06 % | +5,54 % | +0,00 | −0,38 |
| 2025-02-01 → 2025-05-01 | 53 | +6,14 % | −7,83 % | 53 | +6,12 % | −8,30 % | −0,02 | −0,47 |
| 2025-05-01 → 2025-08-01 | 64 | −13,80 % | +15,60 % | 64 | −13,86 % | +15,02 % | −0,06 | −0,58 |
| 2025-08-01 → 2025-11-01 | 45 | −10,70 % | +11,96 % | 45 | −10,82 % | +11,27 % | −0,12 | −0,69 |

### US2000, ventana por ventana (retorno de la cuenta en la prueba)

| Prueba | «US500» EUR (op.) | «US500» USD (op.) | «Proporcional» EUR (op.) | «Proporcional» USD (op.) | CyM EUR | CyM USD |
|---|---|---|---|---|---|---|
| 2019-08-01 → 2019-11-01 | −26,80 % (35) | −26,88 % (35) | −15,50 % (40) | −15,52 % (40) | −0,22 % | −0,33 % |
| 2019-11-01 → 2020-02-01 | −16,88 % (38) | −16,87 % (38) | −16,92 % (44) | −16,89 % (44) | +0,42 % | +0,34 % |
| 2020-02-01 → 2020-05-01 | +7,80 % (40) | +7,74 % (40) | +9,96 % (40) | +9,90 % (40) | −3,32 % | −3,37 % |
| 2020-05-01 → 2020-08-01 | −9,13 % (49) | −9,14 % (49) | −7,50 % (49) | −7,51 % (49) | +1,77 % | +1,75 % |
| 2020-08-01 → 2020-11-01 | −19,95 % (42) | −19,96 % (42) | −16,56 % (56) | −16,57 % (56) | +0,55 % | +0,52 % |
| 2020-11-01 → 2021-02-01 | −12,23 % (49) | −12,26 % (49) | −6,61 % (61) | −6,64 % (61) | +4,97 % | +4,95 % |
| 2021-02-01 → 2021-05-01 | −9,05 % (47) | −9,06 % (47) | −0,75 % (57) | −0,76 % (57) | +2,07 % | +2,04 % |
| 2021-05-01 → 2021-08-01 | +7,95 % (53) | +7,93 % (53) | +10,42 % (53) | +10,40 % (53) | −0,57 % | −0,60 % |
| 2021-08-01 → 2021-11-01 | −13,66 % (49) | −13,65 % (49) | −12,73 % (65) | −12,74 % (65) | +0,62 % | +0,59 % |
| 2021-11-01 → 2022-02-01 | +1,03 % (54) | +1,01 % (54) | +3,44 % (54) | +3,42 % (54) | −3,02 % | −3,06 % |
| 2022-02-01 → 2022-05-01 | −10,54 % (41) | −10,53 % (41) | −9,91 % (42) | −9,91 % (42) | −1,66 % | −1,69 % |
| 2022-05-01 → 2022-08-01 | −12,55 % (41) | −12,60 % (41) | −13,31 % (43) | −13,38 % (43) | −0,07 % | −0,15 % |
| 2022-08-01 → 2022-11-01 | −14,85 % (59) | −14,87 % (59) | −8,24 % (65) | −8,28 % (65) | −0,36 % | −0,47 % |
| 2022-11-01 → 2023-02-01 | −19,41 % (56) | −19,44 % (56) | −15,58 % (53) | −15,62 % (53) | +0,59 % | +0,48 % |
| 2023-02-01 → 2023-05-01 | −18,72 % (42) | −18,79 % (42) | −17,64 % (43) | −17,68 % (43) | −1,94 % | −2,03 % |
| 2023-05-01 → 2023-08-01 | −16,22 % (40) | −16,27 % (40) | −1,55 % (58) | −1,69 % (58) | +2,14 % | +2,06 % |
| 2023-08-01 → 2023-11-01 | −13,74 % (44) | −14,29 % (48) | −15,01 % (53) | −14,93 % (53) | −3,88 % | −3,96 % |
| 2023-11-01 → 2024-02-01 | −2,55 % (41) | −2,56 % (41) | −2,43 % (41) | −2,44 % (41) | +2,67 % | +2,61 % |
| 2024-02-01 → 2024-05-01 | −14,03 % (31) | −14,05 % (31) | −8,50 % (45) | −8,51 % (45) | −0,13 % | −0,20 % |
| 2024-05-01 → 2024-08-01 | −18,01 % (35) | −18,00 % (35) | −13,58 % (31) | −13,57 % (31) | +2,62 % | +2,54 % |
| 2024-08-01 → 2024-11-01 | −8,38 % (48) | −8,43 % (48) | −6,59 % (48) | −6,65 % (48) | −1,14 % | −1,23 % |
| 2024-11-01 → 2025-02-01 | −2,63 % (55) | −2,61 % (55) | +2,46 % (56) | +2,49 % (56) | +0,53 % | +0,45 % |
| 2025-02-01 → 2025-05-01 | −13,14 % (36) | −13,14 % (36) | −9,88 % (53) | −9,89 % (53) | −3,07 % | −3,17 % |
| 2025-05-01 → 2025-08-01 | +2,85 % (52) | +2,81 % (52) | +7,72 % (52) | +7,71 % (52) | +2,21 % | +2,09 % |
| 2025-08-01 → 2025-11-01 | −10,13 % (37) | −10,17 % (37) | −8,96 % (37) | −9,00 % (37) | +2,41 % | +2,28 % |

Δ es USD menos EUR, en puntos porcentuales. La diferencia entre «como US500» y «proporcional» es mucho mayor que la que hay entre tasas: mediana de +2,5 puntos por ventana a favor del «proporcional», con entre −1,3 y +14,7 según la ventana. Con menos costo cambian el tamaño y los stops que el riesgo acepta, y por eso cambia también la cantidad de operaciones.

## Límites

1. **Costos del Russell supuestos.** IG España no lo lista (#371). Si IG lo ofrece, hay que medir su spread en la demo (como en #67) y repetir el estudio. Los dos supuestos acotan un rango razonable, y en los dos se pierde.
2. **Tipo de referencia sin confirmar** para US100 y US2000. Los dos escenarios dan el mismo veredicto.
3. **Spread fijo en puntos en toda la historia.** Para US100 se usaron los 1,6 puntos de hoy (config) en los 13 años. NQ cotizaba unos 3.000 puntos en 2013, unos 7.600 en 2019 y unos 22.600 en 2025, así que en 2013 ese spread pesaba unas 7 veces más en relación al precio. Una parte de la pérdida de 2013–2018 es ese anacronismo. Aun así, desde 2019 se pierde en 4 de los 7 años de prueba, y en el total.
4. **Futuros, no el CFD:** la base y los horarios no son los del CFD de IG.
5. **Sin noticias:** no hay calendario guardado ni interpretaciones antes del 03-12-2025, que ya cae dentro del fuera de muestra.
6. **US2000 todavía sin diarias** (#449). Este estudio no las usa: el walk-forward no aplica los tercios de volatilidad ni los indicadores diarios.

## Reproducir

En `engine/`, con `DATABASE_URL` de solo lectura (agregarle `?options=-c%20default_transaction_read_only%3Don`):

```text
cargo run --release -p cfds-backtest --example walk_forward -- \
    US100 2011-01-03T00:00:00Z 2026-11-23T00:00:00Z 10000 0.5 --fuente futuros --sin-sensibilidad \
    --escenario-financiacion EONIA_ESTR=../docs/analysis/2026-11-eur-eonia-estr.csv
cargo run --release -p cfds-backtest --example walk_forward -- \
    US2000 2017-07-10T00:00:00Z 2026-11-23T00:00:00Z 10000 1 --fuente futuros --sin-sensibilidad \
    --supuestos ../docs/analysis/2026-11-us2000-supuestos-us500.toml \
    --escenario-financiacion EFFR_SOFR=../docs/analysis/2026-11-usd-effr-sofr.csv
```

Para el resto de las corridas, cambiar el archivo de supuestos o el CSV de tasas.
