# Estudio: walk-forward de US500, US100 y US30 con dos escenarios de financiación (€STR y SOFR)

Issue #405 · 24-11-2026 · **solo estudio de sensibilidad, no es un resultado oficial.** El tipo de referencia de la financiación de estos tres CFD sigue **pendiente** en `config/instruments.toml` ("Confirmar con IG") y este estudio **no lo cambia**. Cada escenario pone un tipo rotulado **solo en memoria**, dentro de la corrida. La fase 5 de EE. UU. sigue bloqueada hasta que IG confirme la tasa.

> **Advertencia.** Los dos escenarios son supuestos: ni la API de IG ni ig.com/es confirman qué tipo de referencia usan estos contratos en EUR. Los números de abajo sirven para saber **si la conclusión depende de esa tasa**, no para aprobar ni rechazar la fase 5. Las ventanas son pasado: no prometen ningún resultado futuro.

## Conclusión

- **El veredicto no cambia con la tasa.** En los tres índices, con €STR y con SOFR:
  - el **número de ventanas positivas** es el mismo (US500 2 de 16, US100 4 de 8, US30 4 de 16);
  - las **veces que la estrategia supera a comprar y mantener** son las mismas (2 de 16, 3 de 8 y 3 de 16);
  - el **retorno medio por ventana de prueba** casi no se mueve: US500 −8,09 % frente a −8,04 %, US100 +0,07 % frente a −0,00 %, US30 −7,30 % frente a −6,88 %.
- **US500 y US30 pierden con las dos tasas.** El retorno medio y la mediana son claramente negativos y el Sharpe medio queda en −1,5 o peor. **US100 queda en cero:** el retorno medio cruza el cero de +0,07 % a −0,00 %, pero es ruido alrededor de cero, con la mediana en +0,66 % y +0,61 %. Con ninguna tasa hay una ventaja clara.
- **La financiación pesa poco en la estrategia.** En la mayoría de las ventanas, la diferencia SOFR − €STR es de −0,3 a 0 puntos de retorno (mediana de −0,04 a −0,08 puntos), porque casi todas las operaciones se cierran en el día. SOFR es mucho más alta que €STR en el período (unos 1,7 puntos de media en las ventanas de prueba), pero eso apenas cambia el resultado: los largos pagan un poco más y los cortos reciben un poco más.
- **Comprar y mantener sí la nota.** El retorno de comprar y mantener baja entre 0,1 y 0,7 puntos por trimestre con SOFR, en todas las ventanas. Con un tamaño de 1 contrato mínimo es poco capital invertido (ver "Límites"), así que en una posición del tamaño de la cuenta el efecto sería varias veces mayor.
- **Dos ventanas cambian mucho por un efecto de camino, no por la tasa.** En US500 2023-11 → 2024-02 (50 frente a 47 operaciones, +2,12 puntos) y en US30 2025-05 → 2025-08 (54 frente a 65 operaciones, +7,51 puntos), una diferencia chica de capital cambia el recorrido. La causa probable, no verificada operación por operación: el kill switch, que en backtest se reactiva a los 20 días hábiles (#277), o la pérdida diaria máxima saltan en otro momento, y eso cambia qué operaciones se abren. Muestra que una ventana sola de 3 meses es frágil. No es un efecto de la financiación en sí.

**Para el dueño:** confirmar con IG el tipo de referencia de US500, US100 y US30 sigue siendo necesario para que los resultados sean oficiales, pero no va a dar vuelta la conclusión de la estrategia actual. El problema de US500 y US30 no es la financiación.

## Método

- **Herramienta:** ejemplo `walk_forward` (#310), con un flag nuevo, `--escenario-financiacion` (este PR):
  - `ESTR`: pone `ESTR` como tipo de referencia del instrumento **en memoria** y usa la serie diaria del BCE guardada en `reference_rates`, con su hora de publicación real (#79).
  - `SOFR=<archivo.csv>`: pone `SOFR` en memoria y lee la serie diaria oficial de la NY Fed desde un CSV. Cada valor cuenta recién desde las **13:00 UTC de dos días hábiles después** de su fecha. La NY Fed publica hacia las 08:00 ET del día hábil siguiente (12:00 o 13:00 UTC): el rezago extra de un día hábil cubre los feriados bancarios de EE. UU., así que ningún valor se usa antes de estar publicado (sin look-ahead, CLAUDE.md §5b).
  - En este modo **no se guarda ningún reporte** (se niega a correr con `report_id`) y **no se evalúa el fuera de muestra**.
  - La tasa administrativa (3 %, #379), la base de 360 días, el spread y el resto de los costos son los del catálogo, sin cambios.
- **Walk-forward:** el de `config/data_split.toml` (24 meses de entrenamiento, 3 de prueba, paso 3, 12 meses fuera de muestra) y `config/evaluation.toml` (30 días de calentamiento), con los parámetros de config. La sensibilidad se salteó (`--sin-sensibilidad`): no hace falta para comparar tasas. **El fuera de muestra (desde el 01-11-2025) no se tocó**: no se cargó en ninguna métrica y no se miró.
- **Datos:** velas de futuros de Databento (`--fuente futuros`, ES, NQ y YM continuos), con spread de config (el filtro de spread §7.3 no se aplica a futuros, #267) y cierre antes de cada vencimiento. No hay calendario de noticias guardado para el período, así que el filtro §7.2 no descuenta nada (igual en los dos escenarios).
- **Base real en solo lectura:** cada corrida usó una conexión con `default_transaction_read_only=on`, y no se escribió nada en `backtest_runs` ni en `walk_forward_reports`.
- **Comando** (uno por índice y escenario; tamaño de comprar y mantener = el mínimo del catálogo: 1 en US500, 0,5 en US100, 0,2 en US30):

  ```text
  cargo run --release -p cfds-backtest --example walk_forward -- \
      US500 2019-10-03T00:00:00Z 2026-11-23T00:00:00Z 10000 1 \
      --fuente futuros --sin-sensibilidad --escenario-financiacion ESTR

  cargo run --release -p cfds-backtest --example walk_forward -- \
      US500 2019-10-03T00:00:00Z 2026-11-23T00:00:00Z 10000 1 \
      --fuente futuros --sin-sensibilidad \
      --escenario-financiacion SOFR=../docs/analysis/2026-11-sofr-nyfed.csv
  ```

### Fuentes de las tasas

| Escenario | Serie | Fuente oficial | Período usado | Media anual |
|---|---|---|---|---|
| €STR | Euro short-term rate, diaria | BCE (Data Portal), cargada en `reference_rates` por `python -m cfds_intel.rates` (#79), con la hora de publicación real | 01-10-2019 → 20-11-2026 | 2021 −0,57 %, 2022 −0,01 %, 2023 3,21 %, 2024 3,65 %, 2025 2,18 % |
| SOFR | Secured Overnight Financing Rate, diaria | Federal Reserve Bank of New York, `https://markets.newyorkfed.org/api/rates/secured/sofr/search.json` (descargada el 24-11-2026), copiada tal cual en `docs/analysis/2026-11-sofr-nyfed.csv` | 03-09-2019 → 20-11-2026 | 2021 0,04 %, 2022 1,64 %, 2023 5,01 %, 2024 5,15 %, 2025 4,24 % |

En las ventanas de prueba (01-11-2021 → 01-11-2025), la media es de **2,14 % para €STR y 3,85 % para SOFR**.

### Período: desde octubre de 2019 y no desde 2011

Los futuros ES están completos desde 2011, pero **€STR existe recién desde el 01-10-2019** y no hay ningún valor anterior en la base. La alternativa era EONIA, que el BCE publica como serie oficial, pero no está cargada y cargarla significaría escribir en la base, cosa que este estudio no hace. SOFR, además, empieza en abril de 2018. Por eso **los dos escenarios corren sobre el mismo período, desde el 03-10-2019**: así la única diferencia entre ellos es la tasa.

### Estado de los datos de cada índice

| Índice | Futuro | Velas en la base (15m) | Usado en el estudio | Ventanas de prueba |
|---|---|---|---|---|
| US500 | ES | 02-01-2011 → 22-11-2026 (completo) | 03-10-2019 → 22-11-2026 | 16 (11-2021 → 11-2025) |
| US30 | YM | 01-06-2018 → 22-11-2026 (todavía cargando hacia atrás) | 03-10-2019 → 22-11-2026, completo para este período | 16 (11-2021 → 11-2025) |
| US100 | NQ | 01-11-2021 → 22-11-2026 (**todavía cargando hacia atrás**) | 01-11-2021 → 22-11-2026 | **8** (11-2023 → 11-2025) |

Para US100 se usó lo que había: la mitad de las ventanas. Cuando termine la carga de NQ, conviene repetir US100 con el mismo comando.

## Resumen por índice y escenario (ventanas de prueba)

| Índice | Escenario | Ventanas | Operaciones | Retorno medio | Mediana | Positivas | Peor drawdown | Sharpe medio | Supera a comprar y mantener | Comprar y mantener medio |
|---|---|---|---|---|---|---|---|---|---|---|
| US500 | €STR | 16 | 946 | −8,09 % | −10,19 % | 2 | 22,28 % | −1,64 | 2 de 16 | +0,81 % |
| US500 | SOFR | 16 | 943 | −8,04 % | −10,21 % | 2 | 22,30 % | −1,63 | 2 de 16 | +0,60 % |
| US100 | €STR | 8 | 471 | +0,07 % | +0,66 % | 4 | 14,34 % | −0,07 | 3 de 8 | +5,97 % |
| US100 | SOFR | 8 | 471 | −0,00 % | +0,61 % | 4 | 14,37 % | −0,08 | 3 de 8 | +5,54 % |
| US30 | €STR | 16 | 916 | −7,30 % | −9,03 % | 4 | 23,93 % | −1,62 | 3 de 16 | +0,53 % |
| US30 | SOFR | 16 | 927 | −6,88 % | −8,37 % | 4 | 23,97 % | −1,52 | 3 de 16 | +0,21 % |

Cada ventana de prueba arranca del capital inicial (10 000 €). Los retornos son de la cuenta. "Comprar y mantener" es el mismo período con el tamaño mínimo, con costos y financiación.

## Ventanas de prueba, una por una

Δ es SOFR menos €STR, en puntos porcentuales.

### US500 (futuros ES)

| Prueba | Op. €STR | Retorno €STR | CyM €STR | Op. SOFR | Retorno SOFR | CyM SOFR | Δ retorno | Δ CyM |
|---|---|---|---|---|---|---|---|---|
| 2021-11-01 → 2022-02-01 | 57 | −14,41 % | −1,46 % | 57 | −14,44 % | −1,54 % | −0,03 | −0,08 |
| 2022-02-01 → 2022-05-01 | 61 | −14,29 % | −3,83 % | 61 | −14,29 % | −3,92 % | +0,00 | −0,09 |
| 2022-05-01 → 2022-08-01 | 56 | +11,44 % | −0,53 % | 56 | +11,41 % | −0,70 % | −0,03 | −0,17 |
| 2022-08-01 → 2022-11-01 | 69 | +3,67 % | −2,64 % | 69 | +3,63 % | −2,88 % | −0,04 | −0,24 |
| 2022-11-01 → 2023-02-01 | 57 | −5,66 % | +1,45 % | 57 | −5,76 % | +1,21 % | −0,10 | −0,24 |
| 2023-02-01 → 2023-05-01 | 52 | −16,13 % | +0,44 % | 52 | −16,19 % | +0,23 % | −0,06 | −0,21 |
| 2023-05-01 → 2023-08-01 | 68 | −1,75 % | +3,65 % | 68 | −1,87 % | +3,46 % | −0,12 | −0,19 |
| 2023-08-01 → 2023-11-01 | 68 | −9,94 % | −4,94 % | 68 | −9,94 % | −5,12 % | +0,00 | −0,18 |
| 2023-11-01 → 2024-02-01 | 50 | −13,56 % | +5,99 % | 47 | −11,44 % | +5,83 % | **+2,12** | −0,16 |
| 2024-02-01 → 2024-05-01 | 66 | −5,76 % | +0,95 % | 66 | −5,78 % | +0,78 % | −0,02 | −0,17 |
| 2024-05-01 → 2024-08-01 | 65 | −3,02 % | +4,40 % | 65 | −3,30 % | +4,20 % | −0,28 | −0,20 |
| 2024-08-01 → 2024-11-01 | 69 | −12,98 % | +0,60 % | 69 | −13,07 % | +0,37 % | −0,09 | −0,23 |
| 2024-11-01 → 2025-02-01 | 54 | −10,44 % | +2,32 % | 54 | −10,47 % | +2,11 % | −0,03 | −0,21 |
| 2025-02-01 → 2025-05-01 | 57 | −9,60 % | −4,28 % | 57 | −9,60 % | −4,55 % | +0,00 | −0,27 |
| 2025-05-01 → 2025-08-01 | 45 | −11,84 % | +6,54 % | 45 | −12,06 % | +6,21 % | −0,22 | −0,33 |
| 2025-08-01 → 2025-11-01 | 52 | −15,22 % | +4,29 % | 52 | −15,53 % | +3,91 % | −0,31 | −0,38 |

### US100 (futuros NQ, carga incompleta)

| Prueba | Op. €STR | Retorno €STR | CyM €STR | Op. SOFR | Retorno SOFR | CyM SOFR | Δ retorno | Δ CyM |
|---|---|---|---|---|---|---|---|---|
| 2023-11-01 → 2024-02-01 | 69 | +2,39 % | +12,86 % | 69 | +2,28 % | +12,60 % | −0,11 | −0,26 |
| 2024-02-01 → 2024-05-01 | 47 | −6,03 % | −0,38 % | 47 | −6,09 % | −0,68 % | −0,06 | −0,30 |
| 2024-05-01 → 2024-08-01 | 65 | +19,96 % | +9,30 % | 65 | +19,77 % | +8,95 % | −0,19 | −0,35 |
| 2024-08-01 → 2024-11-01 | 69 | +3,68 % | +0,34 % | 69 | +3,59 % | −0,05 % | −0,09 | −0,39 |
| 2024-11-01 → 2025-02-01 | 59 | −1,07 % | +5,92 % | 59 | −1,06 % | +5,54 % | +0,01 | −0,38 |
| 2025-02-01 → 2025-05-01 | 53 | +6,14 % | −7,83 % | 53 | +6,12 % | −8,30 % | −0,02 | −0,47 |
| 2025-05-01 → 2025-08-01 | 64 | −13,80 % | +15,60 % | 64 | −13,86 % | +15,02 % | −0,06 | −0,58 |
| 2025-08-01 → 2025-11-01 | 45 | −10,70 % | +11,96 % | 45 | −10,82 % | +11,27 % | −0,12 | −0,69 |

### US30 (futuros YM)

| Prueba | Op. €STR | Retorno €STR | CyM €STR | Op. SOFR | Retorno SOFR | CyM SOFR | Δ retorno | Δ CyM |
|---|---|---|---|---|---|---|---|---|
| 2021-11-01 → 2022-02-01 | 63 | −15,24 % | −2,19 % | 63 | −15,28 % | −2,31 % | −0,04 | −0,12 |
| 2022-02-01 → 2022-05-01 | 71 | −3,65 % | −4,34 % | 71 | −3,66 % | −4,46 % | −0,01 | −0,12 |
| 2022-05-01 → 2022-08-01 | 69 | −12,66 % | −1,00 % | 69 | −12,69 % | −1,27 % | −0,03 | −0,27 |
| 2022-08-01 → 2022-11-01 | 69 | −7,21 % | −0,34 % | 69 | −7,28 % | −0,72 % | −0,07 | −0,38 |
| 2022-11-01 → 2023-02-01 | 60 | +4,03 % | +1,81 % | 60 | +4,02 % | +1,41 % | −0,01 | −0,40 |
| 2023-02-01 → 2023-05-01 | 45 | −22,25 % | −0,84 % | 45 | −22,26 % | −1,18 % | −0,01 | −0,34 |
| 2023-05-01 → 2023-08-01 | 67 | +0,80 % | +1,96 % | 67 | +0,72 % | +1,64 % | −0,08 | −0,32 |
| 2023-08-01 → 2023-11-01 | 57 | −15,08 % | −6,47 % | 57 | −15,07 % | −6,75 % | +0,01 | −0,28 |
| 2023-11-01 → 2024-02-01 | 55 | +7,92 % | +9,28 % | 55 | +7,69 % | +9,04 % | −0,23 | −0,24 |
| 2024-02-01 → 2024-05-01 | 38 | −15,26 % | −1,98 % | 38 | −15,28 % | −2,25 % | −0,02 | −0,27 |
| 2024-05-01 → 2024-08-01 | 67 | −9,51 % | +4,98 % | 67 | −9,59 % | +4,68 % | −0,08 | −0,30 |
| 2024-08-01 → 2024-11-01 | 64 | −8,56 % | +0,24 % | 64 | −8,69 % | −0,09 % | −0,13 | −0,33 |
| 2024-11-01 → 2025-02-01 | 54 | +14,62 % | +4,20 % | 54 | +14,57 % | +3,89 % | −0,05 | −0,31 |
| 2025-02-01 → 2025-05-01 | 31 | −12,75 % | −8,24 % | 31 | −12,74 % | −8,63 % | +0,01 | −0,39 |
| 2025-05-01 → 2025-08-01 | 54 | −14,02 % | +5,60 % | 65 | −6,51 % | +5,13 % | **+7,51** | −0,47 |
| 2025-08-01 → 2025-11-01 | 52 | −7,98 % | +5,83 % | 52 | −8,04 % | +5,31 % | −0,06 | −0,52 |

## Límites

1. **Ninguno de los dos escenarios está confirmado.** IG puede usar otro tipo para estos contratos (por ejemplo, otro índice en EUR o un margen distinto). Si confirma uno de estos dos, este estudio ya dice cuánto pesa. Si confirma otro, la sensibilidad sirve como referencia: entre dos tasas que difieren unos 1,7 puntos de media, el veredicto no cambió.
2. **Futuros, no el CFD.** Las velas son de ES, NQ y YM: la base y los horarios no son los del CFD de IG, y el spread es el de config, no uno medido.
3. **Comprar y mantener con el tamaño mínimo.** Con 1, 0,5 o 0,2 contratos sobre 10 000 € de cuenta, el nocional es una fracción de la cuenta. Por eso el efecto de la tasa sobre comprar y mantener se ve chico en %. En una posición del tamaño de la cuenta, la diferencia SOFR − €STR costaría del orden de 1,7 % anual.
4. **Sin calendario de noticias** en el período (§7.2) y sin interpretaciones de noticias. Las interpretaciones empiezan el 03-12-2025, dentro del fuera de muestra. Es igual en los dos escenarios, así que no afecta la comparación, pero sí el nivel absoluto.
5. **US100 con la mitad de las ventanas**, porque NQ sigue cargando.
6. **Antes de octubre de 2019 no se midió** (ver "Período").

## Reproducir

1. Compilar en la raíz `engine/` y exportar `DATABASE_URL`. Conviene agregarle `?options=-c%20default_transaction_read_only%3Don` para que la sesión sea de solo lectura.
2. Correr los dos comandos de "Método" con `US500 … 1`, `US100 … 0.5` y `US30 … 0.2`.
3. El CSV de SOFR está en `docs/analysis/2026-11-sofr-nyfed.csv`. Para actualizarlo, se vuelve a bajar de la URL de la NY Fed de la tabla de fuentes.
